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

//#[macro_use]
extern crate alloc;

use alloc::string::FromUtf8Error;
use container::Container;
use core::str::Utf8Error;
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider};
use heapless::{consts::*, String};
use log::{info, trace};
use miniz_oxide::inflate::TINFLStatus;
use package::Package;
use xml;

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
    XmlParseErr(xml::ParserError),
}

impl<IO> From<fatfs::Error<IO::Error>> for EPubError<IO>
where
    IO: ReadWriteSeek,
{
    fn from(error: fatfs::Error<IO::Error>) -> Self {
        EPubError::IO(error)
    }
}

impl<IO> From<Utf8Error> for EPubError<IO>
where
    IO: ReadWriteSeek,
{
    fn from(error: Utf8Error) -> Self {
        EPubError::UTF8(error)
    }
}

impl<IO> From<FromUtf8Error> for EPubError<IO>
where
    IO: ReadWriteSeek,
{
    fn from(error: FromUtf8Error) -> Self {
        EPubError::FromUTF8(error)
    }
}

/// An epub file
pub struct EPubFile {
    filepath: String<U256>,
    container: Option<Container>,
}

#[cfg(feature = "std")]
impl<IO> std::fmt::Debug for EPubError<IO>
where
    IO: ReadWriteSeek,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_'>) -> std::fmt::Result {
        f.debug_enum("EPubError")?;
    }
}

#[cfg(feature = "std")]
impl<T: Error<std::io::Error>> std::fmt::Debug for EPubError<T>
where
    T: ReadWriteSeek,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_'>) -> std::fmt::Result {
        f.debug_enum("EPubError")?;
    }
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
    pub fn expand<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
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
    pub fn read_container<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
        &self,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<(), EPubError<IO>> {
        if let Some(con) = &self.container {
            let res = con.get_metadata_filenames(fs)?;
            if let Some((opf_filename, container_filename)) = &res {
                trace!("Found opf: {}", opf_filename);
                trace!("Found container: {}", container_filename);
                let pkg = Package::read(opf_filename, container_filename, fs)?;
                info!("Package read: {:?}", pkg);
            }
        } else {
            panic!();
        };
        Ok(())
    }
}
