use crate::io::BufReader;
use crate::EPubError;
use alloc::{string::String, vec::Vec};
use fatfs::{File, FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider, Write};
use log::{info, trace};
use miniz_oxide::inflate::{core, TINFLStatus};
use xml::{Event, Parser, StartTag};

/// represents an extra section from the extra field portion of a LocalFileHeader
#[derive(Debug)]
pub struct ExtraHeader {
    pub id: u16,
    pub data: Vec<u8>,
}

/// represents a Local File Header from the zip specification
#[derive(Debug)]
pub struct LocalFileHeader {
    pub extract_version: u16,
    pub general_purpose_flag: u16,
    pub compression_method: u16,
    pub last_mod_file_time: u16,
    pub last_mod_file_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub file_name: String,
    pub extra_field: Option<Vec<ExtraHeader>>,
    pub data_descriptor: Option<DataDescriptor>,
}

/// represents a data descriptor
#[derive(Debug)]
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
        let mut v = Vec::new();
        v.resize(file_name_length, 0);
        rdr.read_to_array(&mut v)?;
        let file_name = String::from_utf8(v)?;
        let extra_field = if extra_field_length > 0 {
            let mut fieldvec = Vec::new();
            let mut data_left = extra_field_length;
            while data_left > 0 {
                let id = rdr.read2()?;
                let data_len = rdr.read2()? as usize;
                let mut data = Vec::new();
                data.resize(data_len, 0);
                rdr.read_to_array(&mut data)?;
                let ef = ExtraHeader { id, data };
                fieldvec.push(ef);
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
            "begin inflate {} bytes to {} bytes",
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
            rdr.read_to_array(&mut input[..n])?;
            let mut keep_looping = true;
            let mut in_start = 0;
            while keep_looping {
                // following should loop until all input consumed
                let (status, in_consumed, out_consumed) =
                    core::decompress(&mut decomp, &input[in_start..n], &mut output, 0, flags);
                trace!(
                    "inflate [status {:?} incoming {} bytes outgoing {} bytes]",
                    status,
                    in_consumed,
                    out_consumed
                );
                match status {
                    TINFLStatus::NeedsMoreInput | TINFLStatus::Done => {
                        keep_looping = false;
                    }
                    TINFLStatus::HasMoreOutput => {
                        in_start += in_consumed;
                    }
                    e => return Err(EPubError::Decompress(e)),
                }

                let mut out_start = 0;
                while out_start < out_consumed {
                    let n = output_file.write(&output[out_start..out_consumed])?;
                    trace!("wrote {} bytes to file", n,);
                    out_start += n;
                }
                output_file.flush()?;
                count += out_consumed;
            }
            bytes_to_go -= n;
        }
        trace!(
            "finished inflate {} bytes, expected {}",
            count,
            self.uncompressed_size
        );
        Ok(count)
    }
}

/// represents an epub file container
///
/// this type is responsible for expanding the epub file on disk and to pass file handles
/// to other parts of the library
#[derive(Clone)]
pub struct Container {
    expanded_dir_path: String,
}

impl Container {
    const CENTRAL_DIR_FILE_HEADER: u32 = 0x02014b50;
    const EPUB_CONTAINER_FILE: &'static str = "META-INF/container.xml";

    /// create new container rooted at given directory
    pub fn new(dir_path: &str) -> Container {
        Container {
            expanded_dir_path: String::from(dir_path),
        }
    }

    /// get the root file entry from container.xml
    pub fn get_container_rootfile<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        &self,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<Option<Rootfile>, EPubError<IO>> {
        let container_file_name: String = self.expanded_file_path(Container::EPUB_CONTAINER_FILE);
        let root_dir = fs.root_dir();
        let container_file = root_dir.open_file(&container_file_name)?;
        let mut rdr = BufReader::new(container_file)?;
        let mut p = Parser::new();
        let mut stack: Vec<Event> = Vec::new();
        let mut in_rootfiles = false;
        let mut root_file: Option<Rootfile> = None;
        let lines = rdr.read_lines()?;
        for ln in lines {
            p.feed_str(&ln);
            for event in &mut p {
                match event {
                    Ok(e) => match e {
                        Event::PI(s) => info!("PI({})", s),
                        Event::ElementStart(tag) => {
                            info!("Start({})", tag.name);
                            if tag.name == "rootfiles" {
                                in_rootfiles = true;
                            }
                            stack.push(Event::ElementStart(tag))
                        }
                        Event::ElementEnd(tag) => {
                            info!("End({})", tag.name);
                            if let Some(last) = stack.pop() {
                                match last {
                                    Event::ElementStart(start_tag) => {
                                        if tag.name == "rootfiles" {
                                            in_rootfiles = false;
                                        }
                                        if in_rootfiles {
                                            root_file = Some(Rootfile::new(
                                                &start_tag,
                                                &self.expanded_dir_path,
                                            ));
                                        }
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => (),
                    },
                    Err(e) => return Err(EPubError::XmlParseErr(e)),
                }
            }
        }
        Ok(root_file)
    }

    /// expand the epub file into the directory
    pub fn expand<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        &mut self,
        epub_filepath: &str,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<(), EPubError<IO>> {
        // open the epub file
        let root_dir = fs.root_dir();
        let epub_file = root_dir.open_file(epub_filepath)?;

        // create the disk entry file
        info!("creating epub file entry data file");
        let de_filename = self.expanded_file_path("fentry.txt");
        let mut disk_entry_file = root_dir.create_file(&de_filename.as_str())?;
        disk_entry_file.write(epub_filepath.as_bytes())?;
        disk_entry_file.write(b"\n")?;

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
                        let filename = self.expanded_file_path(&lfh.file_name);
                        let mut this_file = root_dir.create_file(&filename.as_str())?;
                        // write the file, either compressed or not
                        if lfh.compression_method == 8 {
                            lfh.inflate(&mut rdr, &mut this_file)?;
                        } else {
                            let mut bytes_to_go = lfh.uncompressed_size as usize;
                            while bytes_to_go > 0 {
                                let mut n = if bytes_to_go > 256 { 256 } else { bytes_to_go };
                                let mut v = Vec::new();
                                v.resize(n, 0);
                                n = rdr.read_to_array(&mut v[..n])?;
                                this_file.write(&v[..n])?;
                                bytes_to_go -= n;
                            }
                        }
                        // add the file entry
                        disk_entry_file.write(&lfh.file_name.as_bytes())?;
                        disk_entry_file.write(b"\n")?;
                    } else if lfh.is_dir() {
                        info!("Create directory {}", lfh.file_name);
                        let dirname = self.expanded_file_path(&lfh.file_name);
                        root_dir.create_dir(&dirname.as_str())?;
                    }
                }
                if lfh.have_data_descriptor() {
                    let dd = DataDescriptor::read(&mut rdr)?;
                    lfh.data_descriptor.replace(dd);
                }
            } else if signature == Container::CENTRAL_DIR_FILE_HEADER {
                info!("End of local file headers in the epub file");
                break;
            } else {
                return Err(EPubError::FormatError(
                    "unknown signature after local file header",
                ));
            }
        }

        Ok(())
    }
    /// create a file path under the epub directory, with the given filename
    fn expanded_file_path(&self, fname: &str) -> String {
        let mut s = String::from(self.expanded_dir_path.as_str());
        s.push_str("/");
        s.push_str(fname);
        s
    }
}

/*
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
        <rootfiles>
                <rootfile full-path="OEBPS/9781718500457.opf" media-type="application/oebps-package+xml" />
        </rootfiles>
</container>
*/
/// represents rootfile section from container.xml
#[derive(Debug)]
pub struct Rootfile {
    pub full_path: String,
    pub media_type: String,
}

impl Rootfile {
    pub fn new(tag: &StartTag, leading_dir: &str) -> Rootfile {
        if let Some(fp) = tag.attributes.get(&(String::from("full-path"), None)) {
            if let Some(mtype) = tag.attributes.get(&(String::from("media-type"), None)) {
                Rootfile {
                    full_path: String::from(leading_dir) + "/" + fp,
                    media_type: String::from(mtype),
                }
            } else {
                panic!();
            }
        } else {
            panic!();
        }
    }
}

#[cfg(test)]
use super::*;

mod tests {

    #[test]
    fn it_works() {
        //        assert_eq!(2, read4(rdr));
    }
}
