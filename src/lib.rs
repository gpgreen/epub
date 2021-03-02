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

pub mod container;
pub mod io;

use container::LocalFileHeader;
use core::str::Utf8Error;
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider, Write};
use heapless::{consts::*, String};
use io::split_path;
use miniz_oxide::inflate::TINFLStatus;

use log::info;
#[cfg(feature = "std")]
use std::fmt;

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

    /// create EPubFile with a filename path
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
            log::trace!("Signature: {:x}", signature);
            if LocalFileHeader::is_lfh(signature) {
                let lfh = LocalFileHeader::read(&mut rdr)?;
                if lfh.general_purpose_flag != 0 {
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
