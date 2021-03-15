//! # epub
//! https://www.w3.org/publishing/epub32/epub-spec.html
//!
//! > Library for reading an epub format ebook

// ****************************************************************************
//
// Imports
//
// ****************************************************************************
#![no_std]

pub mod container;
pub mod io;
pub mod mbr;
pub mod navigation;
pub mod package;

// for testing we want to have std available
#[cfg(test)]
extern crate std;
#[cfg(test)]
#[allow(unused_imports)]
use std::prelude::*;

extern crate alloc;

use alloc::{string::FromUtf8Error, string::String};
use container::Container;
use core::str::Utf8Error;
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider};
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
    InvalidXml,
    InvalidLocalHeader,
    Unimplemented,
    FormatError(&'static str),
    NoSuchVolume,
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
    filepath: String,
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
    pub fn new(filepath: String) -> EPubFile {
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
            let res = con.get_container_rootfile(fs)?;
            if let Some(root_file) = &res {
                trace!("Found root_file: {:?}", root_file);
                let pkg = Package::read(&root_file.full_path, fs)?;
                info!("Package read: {:?}", pkg);
            }
        } else {
            panic!();
        };
        Ok(())
    }
}
