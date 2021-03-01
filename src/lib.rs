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
use core::str::Utf8Error;
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider, Write};
use heapless::{consts::*, String};
use miniz_oxide::inflate::TINFLStatus;

use log;
#[cfg(feature = "std")]
use std::fmt;

pub mod container;
pub mod io;
use container::LocalFileHeader;

/// an error
#[derive(Debug)]
pub enum EPubError<IO>
where
    IO: ReadWriteSeek,
{
    ReadTruncated,
    EmptyFile,
    InvalidLocalHeader,
    Unimplemented,
    FormatError(&'static str),
    NoSuchVolume,
    PathTooLong,
    Decompress(TINFLStatus),
    IO(fatfs::Error<IO::Error>),
    UTF8(Utf8Error),
}

/// An epub file
#[derive(Debug)]
pub struct EPubFile {
    filepath: String<U256>,
}

impl EPubFile {
    const EXPAND_DIR: &'static str = "CUR_BOOK";

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
        let epub_file = current_dir
            .open_file(filename.as_str())
            .map_err(|e| EPubError::IO(e))?;

        // now expand the file
        let root_dir = fs.root_dir();
        let mut rdr = io::BufReader::new(epub_file);
        if rdr.load_block()? == 0 {
            return Err(EPubError::EmptyFile);
        }
        loop {
            #[cfg(feature = "std")]
            log::trace!("{:?}", rdr);
            let signature = rdr.peek4()?;
            log::trace!("Signature: {}", signature);
            if LocalFileHeader::is_lfh(signature) {
                let lfh = LocalFileHeader::read(&mut rdr)?;
                if lfh.compression_method == 0 || lfh.compression_method == 8 {
                    if lfh.is_file() {
                        let filename = self.expanded_file_path(&lfh.file_name)?;
                        let mut this_file = root_dir
                            .create_file(&filename.as_str())
                            .map_err(|e| EPubError::IO(e))?;
                        this_file.truncate().map_err(|e| EPubError::IO(e))?;
                        if lfh.compression_method == 8 {
                            lfh.inflate(&mut rdr, &mut this_file)?;
                        } else {
                            // TODO: read all in at once
                            for _i in 0..lfh.uncompressed_size {
                                this_file
                                    .write(&[rdr.read1()?])
                                    .map_err(|e| EPubError::IO(e))?;
                            }
                        }
                    } else if lfh.is_dir() {
                        let dirname = self.expanded_file_path(&lfh.file_name)?;
                        root_dir
                            .create_dir(&dirname.as_str())
                            .map_err(|e| EPubError::IO(e))?;
                    }
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
        let mut s: String<U256> = String::new();
        s.push_str(EPubFile::EXPAND_DIR)
            .map_err(|_e| EPubError::PathTooLong)?;
        s.push_str("/").map_err(|_e| EPubError::PathTooLong)?;
        s.push_str(fname).map_err(|_e| EPubError::PathTooLong)?;
        Ok(s)
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
