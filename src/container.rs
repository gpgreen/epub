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

/// describes a Local File Header from the zip specification
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
        let file_name_length = rdr.read2()?;
        let extra_field_length = rdr.read2()?;
        let file_name = extract_string_256(rdr, file_name_length as usize)?;
        let extra_field = if extra_field_length > 0 {
            let mut fieldvec = Vec::new();
            let mut data_left = extra_field_length;
            while data_left > 0 {
                let id = rdr.read2()?;
                let data_len = rdr.read2()?;
                let mut data = Vec::new();
                if data_len as usize > data.capacity() {
                    return Err(EPubError::ReadTruncated);
                }
                // TODO: read the whole thing at once
                for _i in 0..data_len {
                    data.push(rdr.read1()?).unwrap();
                }
                let ef = ExtraHeader { id, data };
                fieldvec.push(ef).map_err(|_| EPubError::ReadTruncated)?;
                data_left -= data_len + 4;
            }
            Some(fieldvec)
        } else {
            None
        };

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
            for i in 0..n {
                input[i] = rdr.read1()?;
            }
            let mut do_it = true;
            let mut start = 0;
            while do_it {
                // following should loop until all input consumed
                let (status, in_consumed, out_consumed) =
                    core::decompress(&mut decomp, &input[start..n], &mut output, 0, flags);
                match status {
                    TINFLStatus::NeedsMoreInput | TINFLStatus::Done => {
                        trace!("done with inflate input chunk");
                        do_it = false;
                    }
                    TINFLStatus::HasMoreOutput => {
                        trace!("inflate has more output");
                        start += in_consumed;
                    }
                    e => return Err(EPubError::Decompress(e)),
                }
                trace!(
                    "inflated incoming {} bytes created {} outgoing bytes",
                    in_consumed,
                    out_consumed
                );

                let outb = output_file
                    .write(&output[..out_consumed])
                    .map_err(|x| EPubError::<IO>::IO(x))?;
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

/// extract a maximum 256 byte string from a BufReader
fn extract_string_256<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
    rdr: &mut BufReader<IO, TP, OCC>,
    nbytes: usize,
) -> Result<String<U256>, EPubError<IO>> {
    if nbytes > 256 {
        return Err(EPubError::ReadTruncated);
    }
    let (n, v) = rdr.read(nbytes)?;
    if n < nbytes {
        return Err(EPubError::ReadTruncated);
    }
    Ok(String::from_utf8(v).map_err(|e| EPubError::UTF8(e))?)
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
