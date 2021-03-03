use crate::EPubError;
use byteorder::{ByteOrder, LittleEndian};
use fatfs::{File, OemCpConverter, Read, ReadWriteSeek, TimeProvider};
use heapless::{consts::*, String, Vec};

use log::{info, trace};
#[cfg(feature = "std")]
use std::fmt;

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
    blocks: Vec<[u8; 512], U2>,
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
        blocks.push([0u8; BUFBLOCKSIZE]).unwrap();
        blocks.push([0u8; BUFBLOCKSIZE]).unwrap();
        // start out with this idx, so 0 position is loaded below
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
            &mut self.blocks[1]
        } else {
            &mut self.blocks[0]
        };
        self.file.read(buf).map_err(|e| EPubError::IO(e))
    }

    /// read 1 byte from file
    pub fn read1(&mut self) -> Result<u8, EPubError<IO>> {
        if self.cursor < BUFBLOCKSIZE {
            self.cursor += 1;
            trace!("read1 byte at {}:{}", self.block_idx, self.cursor - 1);
            Ok(self.blocks[self.block_idx][self.cursor - 1])
        } else {
            trace!("read1 block rollover");
            self.load_block()?;
            self.block_idx ^= 1;
            self.cursor = 1;
            trace!("read1 byte at {}:{}", self.block_idx, self.cursor - 1);
            Ok(self.blocks[self.block_idx][0])
        }
    }

    /// read 2 bytes from file
    pub fn read2(&mut self) -> Result<u16, EPubError<IO>> {
        if self.cursor + 2 <= BUFBLOCKSIZE {
            trace!("read2 bytes at {}:{}", self.block_idx, self.cursor);
            self.cursor += 2;
            Ok(LittleEndian::read_u16(
                &self.blocks[self.block_idx][self.cursor - 2..self.cursor],
            ))
        } else {
            trace!("read2 block rollover");
            self.load_block()?;
            let tmpbuf = if self.cursor == BUFBLOCKSIZE {
                let idx = self.block_idx ^ 1;
                trace!("read2 bytes at {}:{}", idx, 0);
                self.cursor = 2;
                [self.blocks[idx][0], self.blocks[idx][1]]
            } else {
                self.cursor = 1;
                trace!(
                    "read2 byte at {}:{} byte at {}:{}",
                    self.block_idx,
                    BUFBLOCKSIZE - 1,
                    self.block_idx ^ 1,
                    0
                );
                [
                    self.blocks[self.block_idx][BUFBLOCKSIZE - 1],
                    self.blocks[self.block_idx ^ 1][0],
                ]
            };
            self.block_idx ^= 1;
            Ok(LittleEndian::read_u16(&tmpbuf))
        }
    }

    /// read 4 bytes from file
    pub fn read4(&mut self) -> Result<u32, EPubError<IO>> {
        if self.cursor + 4 <= BUFBLOCKSIZE {
            self.cursor += 4;
            trace!("read4 bytes at {}:{}", self.block_idx, self.cursor - 4);
            Ok(LittleEndian::read_u32(
                &self.blocks[self.block_idx][self.cursor - 4..self.cursor],
            ))
        } else {
            trace!("read4 block rollover");
            self.roll_over_4(false)
        }
    }

    /// peek at next 4 bytes from file
    pub fn peek4(&mut self) -> Result<u32, EPubError<IO>> {
        if self.cursor + 4 <= BUFBLOCKSIZE {
            trace!("peek4 bytes at {}:{}", self.block_idx, self.cursor);
            Ok(LittleEndian::read_u32(
                &self.blocks[self.block_idx][self.cursor..self.cursor + 4],
            ))
        } else {
            trace!("peek4 block rollover");
            self.roll_over_4(true)
        }
    }

    /// crossing the block boundary with a 4 byte read
    fn roll_over_4(&mut self, peek: bool) -> Result<u32, EPubError<IO>> {
        self.load_block()?;
        let j = BUFBLOCKSIZE - self.cursor;
        let mut tmpbuf: [u8; 4] = [0u8; 4];
        trace!(
            "roll_over_4 {} bytes starting at {}:{}",
            j,
            self.block_idx,
            self.cursor
        );
        for i in 0..j {
            tmpbuf[i] = self.blocks[self.block_idx][self.cursor + i];
        }
        self.block_idx ^= 1;
        trace!(
            "roll_over_4 {} bytes starting at {}:{}",
            4 - j,
            self.block_idx,
            0
        );
        for i in 0..4 - j {
            tmpbuf[i + j] = self.blocks[self.block_idx][i];
        }
        if !peek {
            self.cursor = 4 - j;
        } else {
            self.peek_rolled = true;
            self.block_idx ^= 1;
        }
        Ok(LittleEndian::read_u32(&tmpbuf))
    }

    /// read 256 bytes from file, return as Vec
    pub fn read(&mut self, n: usize) -> Result<Vec<u8, U256>, EPubError<IO>> {
        if n > 256 {
            return Err(EPubError::ReadTruncated);
        };
        if self.cursor + n < BUFBLOCKSIZE {
            let v = Vec::from_slice(&self.blocks[self.block_idx][self.cursor..self.cursor + n])
                .unwrap();
            trace!("read {} bytes at {}:{}", n, self.block_idx, self.cursor);
            self.cursor += n;
            Ok(v)
        } else {
            let mut v = Vec::new();
            trace!("read block rollover");
            self.load_block()?;
            let j = BUFBLOCKSIZE - self.cursor;
            trace!("read {} bytes at {}:{}", j, self.block_idx, self.cursor);
            v.extend_from_slice(&self.blocks[self.block_idx][self.cursor..])
                .unwrap();
            self.block_idx ^= 1;
            trace!("read {} bytes at {}:{}", n - j, self.block_idx, 0);
            v.extend_from_slice(&self.blocks[self.block_idx][..n - j])
                .unwrap();
            self.cursor = n - j;
            Ok(v)
        }
    }

    /// read 512 bytes from file into an array
    pub fn read512_to_array(&mut self, arr: &mut [u8]) -> Result<(), EPubError<IO>> {
        let n = arr.len();
        if n > 512 {
            return Err(EPubError::ReadTruncated);
        };
        if self.cursor + n < BUFBLOCKSIZE {
            for i in 0..n {
                arr[i] = self.blocks[self.block_idx][self.cursor + i];
            }
            trace!(
                "read512_to_array {} bytes at {}:{}",
                n,
                self.block_idx,
                self.cursor
            );
            self.cursor += n;
            Ok(())
        } else {
            trace!("read block rollover");
            self.load_block()?;
            let j = BUFBLOCKSIZE - self.cursor;
            trace!(
                "read512_to_array {} bytes at {}:{}",
                j,
                self.block_idx,
                self.cursor
            );
            for i in 0..j {
                arr[i] = self.blocks[self.block_idx][self.cursor + i];
            }
            self.block_idx ^= 1;
            trace!(
                "read512_to_array {} bytes at {}:{}",
                n - j,
                self.block_idx,
                0
            );
            for i in 0..n - j {
                arr[i + j] = self.blocks[self.block_idx][i];
            }
            self.cursor = n - j;
            Ok(())
        }
    }

    /// get a copy of the current block
    pub fn get_block(&self) -> Vec<u8, U512> {
        Vec::<u8, U512>::from_slice(&self.blocks[self.block_idx]).unwrap()
    }
}

/// function to take a path, return the basename and the extension
/// of the filename in the path. All leading directories are stripped
/// from the basename
pub fn basename_and_ext(path: &String<U256>) -> (String<U8>, String<U4>) {
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
pub fn split_path(path: &String<U256>) -> heapless::Vec<String<U12>, U8> {
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
        let s: String<U256> = String::from("/this/path/is/here.txt");
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
        let s: String<U256> = String::from("/start/end/");
        let vec = split_path(&s);
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[0], "start");
        assert_eq!(vec[1], "end");
    }

    #[test]
    fn test_extension() {
        let s: String<U256> = String::from("/a/start/end.txt");
        let (base_vec, ext_vec) = basename_and_ext(&s);
        assert_eq!(base_vec, "end");
        assert_eq!(ext_vec, ".txt");
    }

    #[test]
    fn test_no_extension() {
        let s: String<U256> = String::from("/start/end");
        let (base_vec, ext_vec) = basename_and_ext(&s);
        assert_eq!(base_vec, "end");
        assert_eq!(ext_vec.len(), 0);
    }
}
