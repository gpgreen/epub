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
pub mod mbr;

use container::Container;
use core::str::Utf8Error;
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider};
use heapless::{consts::*, String};
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
    TooManyFileEntries,
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
    pub const EXPAND_DIR: &'static str = "CUR_BOOK";

    /// create EPubFile with a filename path
    pub fn new(filepath: String<U256>) -> EPubFile {
        EPubFile { filepath }
    }

    /// expand the epub file into a directory
    pub fn expand<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        &self,
        expand_dir: &str,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<(), EPubError<IO>> {
        info!(
            "Expand epub file {} to {}",
            self.filepath,
            EPubFile::EXPAND_DIR
        );
        let mut container = Container::new(expand_dir);
        container.expand(&self.filepath, fs)
    }
}
