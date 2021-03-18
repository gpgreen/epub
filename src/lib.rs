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
use fatfs::{FileSystem, OemCpConverter, ReadWriteSeek, TimeProvider, Write};
use io::BufReader;
use log::{info, trace};
use miniz_oxide::inflate::TINFLStatus;
use navigation::Toc;
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
    EPubFileNotExpanded,
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

/// An epub file
pub struct EPubFile {
    pub epub_filepath: String,
    expanded_filepath: String,
    container: Option<Container>,
    package: Option<Package>,
    toc: Option<Toc>,
}

impl EPubFile {
    pub const CUR_BOOK_DIR: &'static str = "CUR_BOOK";
    pub const EXPAND_DIR: &'static str = "/expanded";
    pub const EPUB_FILE_MEMO: &'static str = "/epub_file.txt";

    /// create EPubFile with a filename path
    pub fn new(epub_filepath: &str, expanded_filepath: &str) -> EPubFile {
        let container = None;
        let package = None;
        let toc = None;
        EPubFile {
            epub_filepath: String::from(epub_filepath),
            expanded_filepath: String::from(expanded_filepath),
            container,
            package,
            toc,
        }
    }

    pub fn get_package<'a, IO, TP, OCC>(
        &'a mut self,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<&'a Package, EPubError<IO>>
    where
        IO: ReadWriteSeek,
        TP: TimeProvider,
        OCC: OemCpConverter,
    {
        self.read_container(fs)?;
        Ok(self.package.as_ref().unwrap())
    }

    pub fn get_toc<'a, IO, TP, OCC>(
        &'a mut self,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<&'a Toc, EPubError<IO>>
    where
        IO: ReadWriteSeek,
        TP: TimeProvider,
        OCC: OemCpConverter,
    {
        self.read_container(fs)?;
        Ok(self.toc.as_ref().unwrap())
    }

    /// check if epub file has already been expanded
    pub fn has_expanded<'a, IO, TP, OCC>(
        &self,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<bool, EPubError<IO>>
    where
        IO: ReadWriteSeek,
        TP: TimeProvider,
        OCC: OemCpConverter,
    {
        let root_dir = fs.root_dir();
        let epub_file_memo_name = String::from(&self.expanded_filepath) + EPubFile::EPUB_FILE_MEMO;
        let mut retval = false;
        match root_dir.open_file(&epub_file_memo_name) {
            Ok(f) => {
                let mut rdr = BufReader::new(f)?;
                let lines = rdr.read_lines()?;
                for ln in lines {
                    if ln == self.epub_filepath {
                        retval = true;
                        break;
                    }
                    break;
                }
                Ok(retval)
            }
            Err(_) => Ok(false),
        }
    }

    /// expand the epub file into a directory
    pub fn expand<'a, IO, TP, OCC>(
        &mut self,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<(), EPubError<IO>>
    where
        IO: ReadWriteSeek,
        TP: TimeProvider,
        OCC: OemCpConverter,
    {
        // clear out cached stuff, in case this is called twice
        self.container = None;
        self.package = None;
        self.toc = None;
        let container_filepath = String::from(&self.expanded_filepath) + EPubFile::EXPAND_DIR;
        io::create_dirs(&container_filepath, fs)?;
        info!(
            "Expand epub file {} to {}",
            self.epub_filepath, container_filepath
        );
        self.container = Some(Container::new(&container_filepath));
        if let Some(con) = &mut self.container {
            con.expand(&self.epub_filepath, fs)?;
            // write a file with the epub filepath in it
            let root_dir = fs.root_dir();
            let file_marker_path = String::from(&self.expanded_filepath) + EPubFile::EPUB_FILE_MEMO;
            let mut epub_file_marker = root_dir.create_file(&file_marker_path)?;
            epub_file_marker.write(&self.epub_filepath.as_bytes())?;
            info!("created epub file memo");
        } else {
            panic!();
        }
        Ok(())
    }

    /// read the container metadata from the epub
    pub fn read_container<'a, IO, TP, OCC>(
        &mut self,
        fs: &mut FileSystem<IO, TP, OCC>,
    ) -> Result<(), EPubError<IO>>
    where
        IO: ReadWriteSeek,
        TP: TimeProvider,
        OCC: OemCpConverter,
    {
        if self.package.is_some() && self.toc.is_some() {
            Ok(())
        } else {
            match &self.container {
                Some(_) => (),
                None => {
                    if self.has_expanded(fs)? {
                        let container_filepath =
                            String::from(&self.expanded_filepath) + EPubFile::EXPAND_DIR;
                        self.container = Some(Container::new(&container_filepath));
                    } else {
                        return Err(EPubError::EPubFileNotExpanded);
                    }
                }
            }
            let con = self.container.as_ref().unwrap();
            let res = con.get_container_rootfile(fs)?;
            if let Some(root_file) = &res {
                trace!("Found root_file: {:?}", root_file);
                let pkg = Package::read(&root_file.full_path, fs)?;
                info!("Package read: {:?}", pkg);
                let tocfile = &pkg.spine.toc;
                for item in &pkg.manifest.items {
                    if &item.id == tocfile {
                        let tocitem = item;
                        let tocpath = String::from(&pkg.base_dir) + "/" + &tocitem.href;
                        let toc = Toc::read(&tocpath, fs)?;
                        info!("Toc read: {:?}", toc);
                        self.toc = Some(toc);
                        break;
                    }
                }
                self.package = Some(pkg);
            }
            Ok(())
        }
    }
}
