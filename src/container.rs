use crate::io::BufReader;
use crate::EPubError;
use fatfs::{File, FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider, Write};
use heapless::{consts::*, String, Vec};
use miniz_oxide::inflate::{core, TINFLStatus};

use log::{info, trace};
#[cfg(feature = "std")]
use std::fmt;

/// represents an extra section from the extra field portion of a LocalFileHeader
#[derive(Debug, Clone)]
pub struct ExtraHeader {
    pub id: u16,
    pub data: Vec<u8, U256>,
}

/// represents a Local File Header from the zip specification
#[derive(Debug, Clone)]
pub struct LocalFileHeader {
    pub extract_version: u16,
    pub general_purpose_flag: u16,
    pub compression_method: u16,
    pub last_mod_file_time: u16,
    pub last_mod_file_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub file_name: String<U256>,
    pub extra_field: Option<Vec<ExtraHeader, U8>>,
    pub data_descriptor: Option<DataDescriptor>,
}

/// represents a data descriptor
#[derive(Debug, Clone)]
pub struct DataDescriptor {
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
}

impl DataDescriptor {
    pub fn read<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        rdr: &mut BufReader<IO, TP, OCC>,
    ) -> Result<DataDescriptor, EPubError<IO>> {
        trace!("read data descriptor");
        let crc32 = rdr.read4()?;
        let compressed_size = rdr.read4()?;
        let uncompressed_size = rdr.read4()?;
        Ok(DataDescriptor {
            crc32,
            compressed_size,
            uncompressed_size,
        })
    }
}

/// debug format for LocalFileHeader
#[cfg(feature = "std")]
impl std::fmt::Debug for LocalFileHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalFileHeader")?;
        if Some(self.extra_field) {
            for eh in Some(self.extra_field).iter() {
                write!(f, "id: {:?} data {:?}", eh.id, eh.data)?;
            }
        }
        write!(f, "")
    }
}

impl LocalFileHeader {
    const LOCALHEADERFILESIG: u32 = 0x04034b50;

    /// is the signature a LocalFileHeader
    pub fn is_lfh(sig_byte: u32) -> bool {
        sig_byte == LocalFileHeader::LOCALHEADERFILESIG
    }

    /// is there data descriptor for this header
    pub fn have_data_descriptor(&self) -> bool {
        self.general_purpose_flag & (1 << 4) == (1 << 4)
    }

    /// does this header describe a file
    pub fn is_file(&self) -> bool {
        self.compressed_size > 0 && self.uncompressed_size > 0
    }

    /// does this header describe a directory
    pub fn is_dir(&self) -> bool {
        self.compressed_size == 0 && self.uncompressed_size == 0 && self.file_name.ends_with("/")
    }

    /// read a LocalFileHeader from BufReader
    pub fn read<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        rdr: &mut BufReader<IO, TP, OCC>,
    ) -> Result<LocalFileHeader, EPubError<IO>> {
        let sig = rdr.read4()?;
        if sig != LocalFileHeader::LOCALHEADERFILESIG {
            return Err(EPubError::<IO>::InvalidLocalHeader);
        }
        let extract_version = rdr.read2()?;
        let general_purpose_flag = rdr.read2()?;
        let compression_method = rdr.read2()?;
        let last_mod_file_time = rdr.read2()?;
        let last_mod_file_date = rdr.read2()?;
        let crc32 = rdr.read4()?;
        let compressed_size = rdr.read4()?;
        let uncompressed_size = rdr.read4()?;
        let file_name_length = rdr.read2()? as usize;
        let extra_field_length = rdr.read2()? as usize;
        if file_name_length > 256 {
            return Err(EPubError::ReadTruncated);
        }
        let file_name =
            String::from_utf8(rdr.read(file_name_length)?).map_err(|e| EPubError::UTF8(e))?;
        let extra_field = if extra_field_length > 0 {
            let mut fieldvec = Vec::new();
            let mut data_left = extra_field_length;
            while data_left > 0 {
                let id = rdr.read2()?;
                let data_len = rdr.read2()? as usize;
                if data_len > 256 {
                    return Err(EPubError::ReadTruncated);
                }
                let data = Vec::from_slice(&rdr.read(data_len)?).unwrap();
                let ef = ExtraHeader { id, data };
                fieldvec.push(ef).map_err(|_| EPubError::ReadTruncated)?;
                data_left -= data_len + 4;
            }
            Some(fieldvec)
        } else {
            None
        };
        let data_descriptor = None;

        let lfh = LocalFileHeader {
            extract_version,
            general_purpose_flag,
            compression_method,
            last_mod_file_time,
            last_mod_file_date,
            crc32,
            compressed_size,
            uncompressed_size,
            file_name,
            extra_field,
            data_descriptor,
        };
        info!("LocalFileHeader for {:?}", lfh.file_name);
        trace!("{:?}", lfh);
        Ok(lfh)
    }

    /// inflate compressed data from a BufReader into a file
    pub fn inflate<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        &self,
        rdr: &mut BufReader<IO, TP, OCC>,
        output_file: &mut File<IO, TP, OCC>,
    ) -> Result<usize, EPubError<IO>> {
        let mut input: [u8; 32768] = [0; 32768];
        let mut output: [u8; 32768] = [0; 32768];
        let mut decomp = core::DecompressorOxide::new();
        decomp.init();
        info!(
            "inflate {} bytes to {} bytes",
            self.compressed_size, self.uncompressed_size
        );
        let mut count = 0;
        let mut bytes_to_go = self.compressed_size as usize;
        while bytes_to_go > 0 {
            let (n, flags) = if bytes_to_go > 32768 {
                (32768, core::inflate_flags::TINFL_FLAG_HAS_MORE_INPUT)
            } else {
                (bytes_to_go, 0)
            };
            trace!("inflate {} byte chunk", n);
            let mut i = 0;
            while i < n {
                let end = if i + 512 < n { i + 512 } else { n };
                rdr.read512_to_array(&mut input[i..end])?;
                i += 512;
            }
            let mut do_it = true;
            let mut start = 0;
            while do_it {
                // following should loop until all input consumed
                let (status, in_consumed, out_consumed) =
                    core::decompress(&mut decomp, &input[start..n], &mut output, 0, flags);
                trace!(
                    "inflate status {:?} incoming {} bytes outgoing {} bytes",
                    status,
                    in_consumed,
                    out_consumed
                );
                match status {
                    TINFLStatus::NeedsMoreInput | TINFLStatus::Done => {
                        do_it = false;
                    }
                    TINFLStatus::HasMoreOutput => {
                        start += in_consumed;
                    }
                    e => return Err(EPubError::Decompress(e)),
                }

                let mut write_start = 0;
                while write_start < out_consumed {
                    let n = output_file
                        .write(&output[write_start..out_consumed])
                        .map_err(|x| EPubError::<IO>::IO(x))?;
                    trace!("wrote {} bytes to file", n,);
                    write_start += n;
                }
                output_file.flush().map_err(|e| EPubError::<IO>::IO(e))?;
                count += out_consumed;
            }
            bytes_to_go -= n;
        }
        trace!(
            "total inflated {} bytes, expected {}",
            count,
            self.uncompressed_size
        );
        Ok(count)
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    disk_name: String<U256>,
    name: String<U256>,
}

#[derive(Debug, Clone)]
pub struct Container {
    file_entries: Vec<FileEntry, U128>,
    expanded_dir_path: String<U256>,
}

impl Container {
    pub fn new(dir_path: &str) -> Container {
        Container {
            file_entries: Vec::new(),
            expanded_dir_path: String::from(dir_path),
        }
    }

    pub fn expand<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        &mut self,
        epub_filepath: &str,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<(), EPubError<IO>> {
        // open the epub file
        let root_dir = fs.root_dir();
        let epub_file = root_dir
            .open_file(epub_filepath)
            .map_err(|e| EPubError::IO(e))?;

        self.file_entries
            .push(FileEntry {
                disk_name: String::from(epub_filepath),
                name: String::from(epub_filepath),
            })
            .map_err(|_| EPubError::TooManyFileEntries)?;

        // now expand the file
        let mut rdr = BufReader::new(epub_file)?;
        loop {
            #[cfg(feature = "std")]
            log::trace!("{:?}", rdr);
            let signature = rdr.peek4()?;
            log::trace!("Signature: {:x}", signature);
            if LocalFileHeader::is_lfh(signature) {
                let mut lfh = LocalFileHeader::read(&mut rdr)?;
                if lfh.general_purpose_flag != 0 && !lfh.have_data_descriptor() {
                    return Err(EPubError::Unimplemented);
                }
                if lfh.compression_method == 0 || lfh.compression_method == 8 {
                    if lfh.is_file() {
                        info!("Create file {}", lfh.file_name);
                        let filename = self.expanded_file_path(&lfh.file_name)?;
                        let mut this_file = root_dir
                            .create_file(&filename.as_str())
                            .map_err(|e| EPubError::IO(e))?;
                        this_file.truncate().map_err(|e| EPubError::IO(e))?;
                        // write the file, either compressed or not
                        if lfh.compression_method == 8 {
                            lfh.inflate(&mut rdr, &mut this_file)?;
                        } else {
                            let mut bytes_to_go = lfh.uncompressed_size as usize;
                            while bytes_to_go > 0 {
                                let n = if bytes_to_go > 256 { 256 } else { bytes_to_go };
                                let v = rdr.read(n)?;
                                this_file.write(&v).map_err(|e| EPubError::IO(e))?;
                                bytes_to_go -= v.len();
                            }
                        }
                    } else if lfh.is_dir() {
                        info!("Create directory {}", lfh.file_name);
                        let dirname = self.expanded_file_path(&lfh.file_name)?;
                        root_dir
                            .create_dir(&dirname.as_str())
                            .map_err(|e| EPubError::IO(e))?;
                    }
                }
                if lfh.have_data_descriptor() {
                    let dd = DataDescriptor::read(&mut rdr)?;
                    lfh.data_descriptor.replace(dd);
                }
            } else {
                break;
            }
        }
        Ok(())
    }
    /// create a file path, with the given filename
    fn expanded_file_path<IO: ReadWriteSeek>(
        &self,
        fname: &String<U256>,
    ) -> Result<String<U256>, EPubError<IO>> {
        let mut s = String::from(self.expanded_dir_path.as_str());
        s.push_str("/").map_err(|_e| EPubError::PathTooLong)?;
        s.push_str(fname).map_err(|_e| EPubError::PathTooLong)?;
        Ok(s)
    }
}

#[cfg(test)]
use super::*;

mod tests {

    // read trait requires
    // fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    #[test]
    fn it_works() {
        //        let mut rdr: Vec<U256> = Vec::new();
        //        rdr.push(0);
        //        rdr.push(0);
        //        rdr.push(0);
        //        rdr.push(2);

        //        assert_eq!(2, read4(rdr));
    }
}
