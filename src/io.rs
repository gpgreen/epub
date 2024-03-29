//! reading EPub documents
//!
//! BufReader uses 2 buffers to read a file
//! the cursor will advance and automatically swap in the blocks as the cursor advances
//! the cursor cannot go backwards

use crate::{io, EPubError};
use alloc::{string::String, vec::Vec};
use byteorder::{ByteOrder, LittleEndian};
use fatfs::{File, FileSystem, OemCpConverter, Read, ReadWriteSeek, TimeProvider};
use log::{info, trace};

/// Read data from blocks serially
///
/// Once the end of a block is reached, another will be retrieved
//#[derive(Debug)]
pub struct BufReader<'a, IO, TP, OCC>
where
    IO: ReadWriteSeek,
    TP: TimeProvider,
    OCC: OemCpConverter,
{
    /// the file we are reading from
    file: File<'a, IO, TP, OCC>,
    /// the block buffers
    blocks: Vec<Vec<u8>>,
    /// which block buffer is the cursor in
    block_idx: usize,
    /// the cursor position in the block_idx buffer
    cursor: usize,
    /// peek has rolled over the boundary, so don't load a new block
    peek_rolled: bool,
}

#[cfg(feature = "std")]
impl<'a, IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter> std::fmt::Debug
    for BufReader<'a, IO, TP, OCC>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} block_idx: {} cursor: {} loaded: {}",
            self.blocks, self.block_idx, self.cursor, self.loaded
        )
    }
}

const BUFBLOCKSIZE: usize = 512;

impl<'a, IO, TP, OCC> BufReader<'a, IO, TP, OCC>
where
    IO: ReadWriteSeek,
    TP: TimeProvider,
    OCC: OemCpConverter,
{
    /// create a BufReader attached to the file
    pub fn new(file: File<IO, TP, OCC>) -> Result<BufReader<IO, TP, OCC>, EPubError<IO>> {
        info!("Creating BufReader");
        let mut blocks = Vec::new();
        blocks.push(Vec::new());
        blocks.push(Vec::new());
        // start out with this idx, so 0 position block is loaded below
        let block_idx = 1;
        let cursor = 0;
        let peek_rolled = false;
        let mut rdr = BufReader {
            file,
            blocks,
            block_idx,
            cursor,
            peek_rolled,
        };
        rdr.load_block()?;
        rdr.block_idx = 0;
        Ok(rdr)
    }

    /// load a block into a buffer slot
    fn load_block(&mut self) -> Result<usize, EPubError<IO>> {
        if self.peek_rolled {
            self.peek_rolled = false;
            return Ok(0);
        }
        trace!("Loading Block into position {}", self.block_idx ^ 1);
        let buf = if self.block_idx == 0 {
            self.blocks[1].resize(BUFBLOCKSIZE, 0);
            &mut self.blocks[1][0..BUFBLOCKSIZE]
        } else {
            self.blocks[0].resize(BUFBLOCKSIZE, 0);
            &mut self.blocks[0][0..BUFBLOCKSIZE]
        };
        // TODO: it may not read all bytes, so need to retry
        let n = self.file.read(buf)?;
        if n != BUFBLOCKSIZE {
            trace!("load_block: short load of {} bytes", n);
            if self.block_idx == 0 {
                self.blocks[1].resize(n, 0);
            } else {
                self.blocks[0].resize(n, 0);
            }
        }
        Ok(n)
    }

    /// read 1 byte from file
    pub fn read1(&mut self) -> Result<u8, EPubError<IO>> {
        let mut arr = [0u8; 1];
        self.read_to_array(&mut arr)?;
        Ok(arr[0])
    }

    /// read 2 bytes from file
    pub fn read2(&mut self) -> Result<u16, EPubError<IO>> {
        let mut arr = [0u8; 2];
        self.read_to_array(&mut arr)?;
        Ok(LittleEndian::read_u16(&arr))
    }

    /// read 4 bytes from file
    pub fn read4(&mut self) -> Result<u32, EPubError<IO>> {
        let mut arr = [0u8; 4];
        self.read_to_array(&mut arr)?;
        Ok(LittleEndian::read_u32(&arr))
    }

    /// peek at next 4 bytes from file
    pub fn peek4(&mut self) -> Result<u32, EPubError<IO>> {
        let cur = self.cursor;
        let idx = self.block_idx;
        let peekee = self.read4()?;
        // restore previous state
        self.cursor = cur;
        if idx != self.block_idx {
            self.block_idx = idx;
            self.peek_rolled = true;
        }
        Ok(peekee)
    }

    /// read from file into an array
    pub fn read_to_array(&mut self, arr: &mut [u8]) -> Result<usize, EPubError<IO>> {
        trace!("read {} bytes to array", arr.len());
        let mut arr_idx = 0;
        let nbytes = arr.len();
        while arr_idx < nbytes {
            let n = if arr_idx + self.blocks[self.block_idx].len() < nbytes {
                self.blocks[self.block_idx].len()
            } else {
                nbytes - arr_idx
            };
            if self.cursor + n < self.blocks[self.block_idx].len() {
                for i in 0..n {
                    arr[arr_idx + i] = self.blocks[self.block_idx][self.cursor + i];
                }
                trace!(
                    "read_to_array {} bytes at {}:{}",
                    n,
                    self.block_idx,
                    self.cursor
                );
                self.cursor += n;
            } else {
                trace!("read block rollover");
                self.load_block()?;
                let j = self.blocks[self.block_idx].len() - self.cursor;
                trace!(
                    "read_to_array {} bytes at {}:{}",
                    j,
                    self.block_idx,
                    self.cursor
                );
                for i in 0..j {
                    arr[arr_idx + i] = self.blocks[self.block_idx][self.cursor + i];
                }
                self.block_idx ^= 1;
                trace!("read_to_array {} bytes at {}:{}", n - j, self.block_idx, 0);
                for i in 0..n - j {
                    arr[arr_idx + i + j] = self.blocks[self.block_idx][i];
                }
                self.cursor = n - j;
            }
            arr_idx += n;
            trace!("read_to_array progress:{} bytes", arr_idx);
        }
        Ok(nbytes)
    }

    /// read lines from file
    pub fn read_lines(&mut self) -> Result<alloc::vec::Vec<alloc::string::String>, EPubError<IO>> {
        // TODO: make sure that file hasn't yet been read
        let mut lines = alloc::vec::Vec::new();
        let mut ln = alloc::vec::Vec::new();
        trace!("read_lines");
        loop {
            let n = self.blocks[self.block_idx].len();
            let mut start = 0;
            for i in 0..n {
                if self.blocks[self.block_idx][i] == b'\n' {
                    ln.extend_from_slice(&self.blocks[self.block_idx][start..i + 1]);
                    let mut newln = alloc::vec::Vec::new();
                    newln.extend(ln.iter().copied());
                    lines.push(alloc::string::String::from_utf8(ln)?);
                    trace!("read_lines line[{}:{}]", start, i + 1);
                    ln = alloc::vec::Vec::new();
                    start = i + 1;
                }
            }
            ln.extend_from_slice(&self.blocks[self.block_idx][start..n]);
            if self.load_block()? == 0 {
                break;
            }
            self.block_idx ^= 1;
        }
        if !ln.is_empty() {
            lines.push(alloc::string::String::from_utf8(ln)?);
        }
        trace!("read_lines count {}", lines.len());
        Ok(lines)
    }
}

/// function to take a path, return the basename and the extension
/// of the filename in the path. All leading directories are stripped
/// from the basename
pub fn basename_and_ext(path: &str) -> (String, String) {
    let base_and_ext = split_path(path).pop().unwrap();
    let mut base = Vec::new();
    let mut ext = Vec::new();
    let mut switch = false;
    for byte in base_and_ext.into_bytes().iter() {
        if *byte != b'.' && !switch {
            base.push(*byte);
        } else {
            switch = true;
            ext.push(*byte);
        }
    }
    (
        String::from_utf8(base).unwrap(),
        String::from_utf8(ext).unwrap(),
    )
}

/// function to split paths up into directory(s) and filename
pub fn split_path(path: &str) -> Vec<String> {
    let mut v = Vec::new();
    for chunk in path.split('/') {
        if !chunk.is_empty() {
            v.push(String::from(chunk));
        }
    }
    v
}

/// function to create all directories in dir_path, if they don't exist
pub fn create_dirs<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
    dir_path: &str,
    fs: &mut FileSystem<IO, TP, OCC>,
) -> Result<(), EPubError<IO>> {
    let dirs = io::split_path(dir_path);
    let mut curdir = Some(fs.root_dir());
    for dir in dirs {
        if let Some(cd) = curdir {
            match cd.open_dir(&dir) {
                Ok(cd) => {
                    curdir = Some(cd);
                }
                Err(_) => {
                    let cd = cd.create_dir(&dir)?;
                    curdir = Some(cd);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_path() {
        let s = String::from("/this/path/is/here.txt");
        let vec = split_path(&s);
        assert_eq!(vec.len(), 4);
        assert_eq!(vec[0], "this");
        assert_eq!(vec[1], "path");
        assert_eq!(vec[2], "is");
        assert_eq!(vec[3], "here.txt");
    }

    #[test]
    fn test_split_path_start() {
        let s = String::from("here.txt");
        let vec = split_path(&s);
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], "here.txt");
    }

    #[test]
    fn test_split_path_end() {
        let s = String::from("/start/end/");
        let vec = split_path(&s);
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[0], "start");
        assert_eq!(vec[1], "end");
    }

    #[test]
    fn test_extension() {
        let s = String::from("/a/start/end.txt");
        let (base_vec, ext_vec) = basename_and_ext(&s);
        assert_eq!(base_vec, "end");
        assert_eq!(ext_vec, ".txt");
    }

    #[test]
    fn test_no_extension() {
        let s = String::from("/start/end");
        let (base_vec, ext_vec) = basename_and_ext(&s);
        assert_eq!(base_vec, "end");
        assert_eq!(ext_vec.len(), 0);
    }
}
