//! # epub
//!
//! > Library for reading an epub format ebook
//!
//! this crate needs alloc, because the decompression library needs dynamic
//! vectors

// ****************************************************************************
//
// Imports
//
// ****************************************************************************
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

/// Vec requires alloc
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

//use embedded_sdmmc::{
//    BlockDevice, Controller, Error, File, Mode::ReadOnly, TimeSource, Volume, VolumeIdx,
//};
use fatfs::{File, FileSystem, OemCpConverter, Read, ReadWriteSeek, TimeProvider, Write};
use heapless::{consts::*, String};
use miniz_oxide::inflate::{decompress_to_vec_with_limit, TINFLStatus};

pub mod container;
pub mod io;

const EXPAND_DIR: &str = "CUR_BOOK";
const EXPAND_EXT: &str = ".EXP";

/// an error
#[derive(Debug)]
pub enum EPubError<IO>
where
    IO: ReadWriteSeek,
{
    InvalidLocalHeader,
    Unimplemented,
    FormatError(&'static str),
    NoSuchVolume,
    PathTooLong,
    Decompress(TINFLStatus),
    IO(fatfs::Error<IO::Error>),
}

/// An epub file
#[derive(Debug)]
pub struct EPubFile {
    filepath: String<U256>,
}

impl EPubFile {
    pub fn new(filepath: String<U256>) -> EPubFile {
        EPubFile { filepath }
    }

    /// open the epub file
    pub fn expand<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        &self,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<(), EPubError<IO>> {
        let mut path_elements = split_path(&self.filepath);
        let filename = path_elements.pop().expect("empty path");
        let root_dir = fs.root_dir();
        let mut current_dir = root_dir;
        for element in path_elements.iter() {
            let dir = current_dir.open_dir(element);
            current_dir = dir.map_err(|e| EPubError::IO(e))?;
        }
        let mut epub_file = current_dir
            .open_file(filename.as_str())
            .map_err(|e| EPubError::IO(e))?;

        // now expand the file
        let expanded_filepath = self.expanded_file_path()?;
        let root_dir = fs.root_dir();
        let mut expanded_file = root_dir
            .create_file(&expanded_filepath.as_str()[1..])
            .map_err(|e| EPubError::IO(e))?;
        expanded_file.truncate().map_err(|e| EPubError::IO(e))?;
        // expand the file
        loop {
            let mut rdr = io::BufReader::new();
            let ni = rdr.load_block(&mut epub_file)?;
            if ni == 0 {
                break;
            }
            let lfh = container::LocalFileHeader::read(&mut rdr)?;
        }
        Ok(())
    }

    fn expanded_file_path<IO: ReadWriteSeek>(&self) -> Result<String<U256>, EPubError<IO>> {
        let mut s: String<U256> = String::new();
        s.push_str("/").map_err(|_e| EPubError::PathTooLong)?;
        s.push_str(EXPAND_DIR)
            .map_err(|_e| EPubError::PathTooLong)?;
        s.push_str("/").map_err(|_e| EPubError::PathTooLong)?;
        let (basename, _ext) = basename_and_ext(&self.filepath);
        s.push_str(&basename).map_err(|_e| EPubError::PathTooLong)?;
        s.push_str(EXPAND_EXT)
            .map_err(|_e| EPubError::PathTooLong)?;
        Ok(s)
    }

    /// decompress a buffer
    fn decompress_chunk<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        &self,
        count: &mut usize,
        buf: &[u8],
        output: &mut File<IO, TP, OCC>,
    ) -> Result<usize, EPubError<IO>> {
        let mut buf_len = 0;
        loop {
            buf_len += 1024;
            match decompress_to_vec_with_limit(buf, buf_len) {
                Ok(vec) => {
                    *count += vec.len();
                    return output
                        .write(vec.as_slice())
                        .map_err(|x| EPubError::<IO>::IO(x));
                }
                Err(TINFLStatus::Done) => return Ok(0),
                Err(TINFLStatus::HasMoreOutput) => continue,
                Err(e) => return Err(EPubError::<IO>::Decompress(e)),
            };
        }
    }
}

/// function to take a path, return the basename and the extension
/// of the filename in the path. All leading directories are stripped
/// from the basename
fn basename_and_ext(path: &String<U256>) -> (String<U8>, String<U4>) {
    let base_and_ext = split_path(path).pop().unwrap();
    let mut base: heapless::Vec<u8, U8> = heapless::Vec::new();
    let mut ext: heapless::Vec<u8, U4> = heapless::Vec::new();
    let mut switch = false;
    for byte in base_and_ext.into_bytes().iter() {
        if *byte != b'.' && !switch {
            base.push(*byte).unwrap();
        } else {
            switch = true;
            ext.push(*byte).unwrap();
        }
    }
    (
        String::from_utf8(base).unwrap(),
        String::from_utf8(ext).unwrap(),
    )
}

/// function to split paths up into directory(s) and filename
/// the separator is the MSDOS separator '\'
fn split_path(path: &String<U256>) -> heapless::Vec<String<U12>, U8> {
    let bytes = path.clone().into_bytes();
    let mut path_elements: heapless::Vec<String<U12>, U8> = heapless::Vec::new();
    let mut element: String<U12> = String::new();
    for (i, &byte) in bytes.iter().enumerate() {
        if byte != b'/' {
            element.push(byte as char).unwrap();
        } else if i != 0 {
            path_elements.push(element).unwrap();
            element = String::new();
        }
    }
    if element.len() > 0 {
        path_elements.push(element).unwrap();
    }
    path_elements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_path() {
        let s: String<U256> = String::from("\\this\\path\\is\\here.txt");
        let vec = split_path(&s);
        assert_eq!(vec.len(), 4);
        assert_eq!(vec[0], "this");
        assert_eq!(vec[1], "path");
        assert_eq!(vec[2], "is");
        assert_eq!(vec[3], "here.txt");
    }

    #[test]
    fn test_split_path_start() {
        let s: String<U256> = String::from("here.txt");
        let vec = split_path(&s);
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], "here.txt");
    }

    #[test]
    fn test_split_path_end() {
        let s: String<U256> = String::from("\\start\\end\\");
        let vec = split_path(&s);
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[0], "start");
        assert_eq!(vec[1], "end");
    }

    #[test]
    fn test_extension() {
        let s: String<U256> = String::from("\\a\\start\\end.txt");
        let (base_vec, ext_vec) = basename_and_ext(&s);
        assert_eq!(base_vec, "end");
        assert_eq!(ext_vec, ".txt");
    }

    #[test]
    fn test_no_extension() {
        let s: String<U256> = String::from("\\start\\end");
        let (base_vec, ext_vec) = basename_and_ext(&s);
        assert_eq!(base_vec, "end");
        assert_eq!(ext_vec.len(), 0);
    }
}
