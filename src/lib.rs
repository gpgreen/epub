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
pub mod package;

#[macro_use]
extern crate alloc;

use alloc::string::FromUtf8Error;
use container::Container;
use core::{borrow::BorrowMut, fmt::Debug, str::Utf8Error};
use fatfs::{File, FileSystem, IoBase, IoError, OemCpConverter, ReadWriteSeek, TimeProvider};
use heapless::{consts::*, String};
use miniz_oxide::inflate::TINFLStatus;

use log::info;

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
    FromUTF8(FromUtf8Error),
}

impl<IO> IoError for EPubError<IO>
where
    IO: ReadWriteSeek + Debug + IoBase<Error = IO>,
{
    fn is_interrupted(&self) -> bool {
        false
    }
    fn new_unexpected_eof_error() -> Self {
        EPubError::<IO>::IO(fatfs::Error::<IO>::UnexpectedEof)
    }
    fn new_write_zero_error() -> Self {
        EPubError::<IO>::IO(fatfs::Error::<IO>::WriteZero)
    }
}

/// An epub file
pub struct EPubFile {
    filepath: String<U256>,
    container: Option<Container>,
}

impl EPubFile {
    pub const EXPAND_DIR: &'static str = "CUR_BOOK";

    /// create EPubFile with a filename path
    pub fn new(filepath: String<U256>) -> EPubFile {
        let container = None;
        EPubFile {
            filepath,
            container,
        }
    }

    /// expand the epub file into a directory
    pub fn expand<
        IO: ReadWriteSeek + Debug + IoBase<Error = IO>,
        TP: TimeProvider,
        OCC: OemCpConverter,
    >(
        &mut self,
        expand_dir: &str,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<(), EPubError<IO>> {
        info!(
            "Expand epub file {} to {}",
            self.filepath,
            EPubFile::EXPAND_DIR
        );
        self.container = Some(Container::new(expand_dir));
        if let Some(con) = &mut self.container {
            con.expand(&self.filepath, fs)
        } else {
            panic!();
        }
    }

    /// open a file from the epub
    pub fn open_file<
        'a,
        IO: ReadWriteSeek + Debug + IoBase<Error = IO>,
        TP: TimeProvider,
        OCC: OemCpConverter,
    >(
        &self,
        file_name: &str,
        fs: &'a mut FileSystem<IO, TP, OCC>,
    ) -> Result<File<'a, IO, TP, OCC>, EPubError<IO>> {
        if let Some(con) = &self.container {
            con.open_file(file_name, fs)
        } else {
            panic!();
        }
    }
}
