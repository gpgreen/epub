use crate::io::BufReader;
use crate::EPubError;
use fatfs::ReadWriteSeek;
use heapless::{consts::*, String};

const LOCALHEADERFILESIG: u32 = 0x04034b50;

#[derive(Debug, Clone)]
pub struct LocalFileHeader {
    extract_version: u16,
    general_purpose_flag: u16,
    compression_method: u16,
    last_mod_file_time: u16,
    last_mod_file_date: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    file_name: String<U256>,
    extra_field: Option<String<U256>>,
}

impl LocalFileHeader {
    pub fn read<IO: ReadWriteSeek>(rdr: &mut BufReader) -> Result<LocalFileHeader, EPubError<IO>> {
        let sig = rdr.read4();
        if sig != LOCALHEADERFILESIG {
            return Err(EPubError::<IO>::InvalidLocalHeader);
        }
        let extract_version = rdr.read2();
        let general_purpose_flag = rdr.read2();
        let compression_method = rdr.read2();
        let last_mod_file_time = rdr.read2();
        let last_mod_file_date = rdr.read2();
        let crc32 = rdr.read4();
        let compressed_size = rdr.read4();
        let uncompressed_size = rdr.read4();
        let file_name_length = rdr.read2();
        let extra_field_length = rdr.read2();
        let file_name = rdr.read(file_name_length as usize);
        let extra_field = if extra_field_length > 0 {
            Some(rdr.read(extra_field_length as usize))
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
        Ok(lfh)
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
