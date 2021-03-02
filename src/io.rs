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
    /// which buffer is the cursor in
    block_idx: usize,
    /// the cursor position in the block_idx buffer
    cursor: usize,
    /// has any data been loaded yet
    loaded: bool,
    /// peek has rolled over the boundary
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
    pub fn new(file: File<IO, TP, OCC>) -> BufReader<IO, TP, OCC> {
        info!("Creating BufReader");
        let mut blocks = Vec::new();
        blocks.push([0u8; BUFBLOCKSIZE]).unwrap();
        blocks.push([0u8; BUFBLOCKSIZE]).unwrap();
        let block_idx = 0;
        let cursor = 0;
        let loaded = false;
        let peek_rolled = false;
        BufReader {
            file,
            blocks,
            block_idx,
            cursor,
            loaded,
            peek_rolled,
        }
    }

    pub fn load_block(&mut self) -> Result<usize, EPubError<IO>> {
        if self.peek_rolled {
            self.peek_rolled = false;
            return Ok(0);
        }
        trace!("Loading Block into position {}", self.block_idx ^ 1);
        let buf = if self.loaded {
            if self.block_idx == 0 {
                &mut self.blocks[1]
            } else {
                &mut self.blocks[0]
            }
        } else {
            self.loaded = true;
            &mut self.blocks[0]
        };
        self.file.read(buf).map_err(|e| EPubError::IO(e))
    }

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

    pub fn read2(&mut self) -> Result<u16, EPubError<IO>> {
        trace!("read2 bytes at {}:{}", self.block_idx, self.cursor);
        if self.cursor + 2 <= BUFBLOCKSIZE {
            self.cursor += 2;
            Ok(LittleEndian::read_u16(
                &self.blocks[self.block_idx][self.cursor - 2..self.cursor],
            ))
        } else {
            trace!("read2 block rollover");
            self.load_block()?;
            let tmpbuf = if self.cursor == BUFBLOCKSIZE {
                let idx = self.block_idx ^ 1;
                self.cursor = 2;
                [self.blocks[idx][0], self.blocks[idx][1]]
            } else {
                self.cursor = 1;
                [
                    self.blocks[self.block_idx][BUFBLOCKSIZE - 1],
                    self.blocks[self.block_idx ^ 1][0],
                ]
            };
            self.block_idx ^= 1;
            Ok(LittleEndian::read_u16(&tmpbuf))
        }
    }

    pub fn read4(&mut self) -> Result<u32, EPubError<IO>> {
        trace!("read4 bytes at {}:{}", self.block_idx, self.cursor);
        if self.cursor + 4 <= BUFBLOCKSIZE {
            self.cursor += 4;
            Ok(LittleEndian::read_u32(
                &self.blocks[self.block_idx][self.cursor - 4..self.cursor],
            ))
        } else {
            trace!("read4 block rollover");
            self.roll_over_4(false)
        }
    }

    pub fn peek4(&mut self) -> Result<u32, EPubError<IO>> {
        trace!("peek4 bytes at {}:{}", self.block_idx, self.cursor);
        if self.cursor + 4 <= BUFBLOCKSIZE {
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
            "read4 {} bytes starting at {}:{}",
            j,
            self.block_idx,
            self.cursor
        );
        for i in 0..j {
            tmpbuf[i] = self.blocks[self.block_idx][self.cursor + i];
        }
        self.block_idx ^= 1;
        trace!("read4 {} bytes starting at {}:{}", 4 - j, self.block_idx, 0);
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

    /// read 512 bytes into an array
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

    pub fn get_block(&self) -> Vec<u8, U512> {
        Vec::<u8, U512>::from_slice(&self.blocks[self.block_idx]).unwrap()
    }
}

#[derive(Clone)]
pub struct Block {
    /// the bytes in the block
    pub contents: [u8; Block::LEN],
}

impl Block {
    pub const LEN: usize = 512;

    pub const LEN_U32: u32 = 521;

    pub fn new() -> Block {
        Block {
            contents: [0u8; Self::LEN],
        }
    }
}

/// A `VolumeIdx` is a number which identifies a volume (or partition) on a
/// disk. `VolumeIdx(0)` is the first primary partition on an MBR partitioned
/// disk.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct VolumeIdx(pub usize);

/// Represents the linear numeric address of a block (or sector). The first
/// block on a disk gets `BlockIdx(0)` (which usually contains the Master Boot
/// Record).
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockIdx(pub u32);

impl BlockIdx {
    pub fn into_bytes(self) -> u64 {
        (u64::from(self.0)) * (Block::LEN as u64)
    }
}

impl core::ops::Add<BlockCount> for BlockIdx {
    type Output = BlockIdx;
    fn add(self, rhs: BlockCount) -> BlockIdx {
        BlockIdx(self.0 + rhs.0)
    }
}

impl core::ops::AddAssign<BlockCount> for BlockIdx {
    fn add_assign(&mut self, rhs: BlockCount) {
        self.0 += rhs.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Partition {
    pub part_type: u8,
    pub lba_start: BlockIdx,
    pub num_blocks: BlockCount,
}

/// Represents the a number of blocks (or sectors). Add this to a `BlockIdx`
/// to get an actual address on disk.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockCount(pub u32);

/// Marker for a FAT32 partition. Sometimes also use for FAT16 formatted
/// partitions.
const PARTITION_ID_FAT32_LBA: u8 = 0x0C;
/// Marker for a FAT16 partition with LBA. Seen on a Raspberry Pi SD card.
const PARTITION_ID_FAT16_LBA: u8 = 0x0E;
/// Marker for a FAT16 partition. Seen on a card formatted with the official
/// SD-Card formatter.
const PARTITION_ID_FAT16: u8 = 0x06;
/// Marker for a FAT32 partition. What Macosx disk utility (and also SD-Card formatter?)
/// use.
const PARTITION_ID_FAT32_CHS_LBA: u8 = 0x0B;

/// Get a volume (or partition) based on entries in the Master Boot
/// Record. We do not support GUID Partition Table disks. Nor do we
/// support any concept of drive letters - that is for a higher layer to
/// handle.
pub fn get_partition<IO: ReadWriteSeek, TP: TimeProvider, OCC: OemCpConverter>(
    file: &mut File<IO, TP, OCC>,
    volume_idx: VolumeIdx,
) -> Result<Partition, EPubError<IO>> {
    const PARTITION1_START: usize = 446;
    const PARTITION2_START: usize = PARTITION1_START + PARTITION_INFO_LENGTH;
    const PARTITION3_START: usize = PARTITION2_START + PARTITION_INFO_LENGTH;
    const PARTITION4_START: usize = PARTITION3_START + PARTITION_INFO_LENGTH;
    const FOOTER_START: usize = 510;
    const FOOTER_VALUE: u16 = 0xAA55;
    const PARTITION_INFO_LENGTH: usize = 16;
    const PARTITION_INFO_STATUS_INDEX: usize = 0;
    const PARTITION_INFO_TYPE_INDEX: usize = 4;
    const PARTITION_INFO_LBA_START_INDEX: usize = 8;
    const PARTITION_INFO_NUM_BLOCKS_INDEX: usize = 12;

    let mut block: [u8; 512] = [0u8; 512];
    file.read(&mut block).map_err(|e| EPubError::IO(e))?;
    let (part_type, lba_start, num_blocks) = {
        // We only support Master Boot Record (MBR) partitioned cards, not
        // GUID Partition Table (GPT)
        if LittleEndian::read_u16(&block[FOOTER_START..FOOTER_START + 2]) != FOOTER_VALUE {
            return Err(EPubError::<IO>::FormatError("Invalid MBR signature"));
        }
        let partition = match volume_idx {
            VolumeIdx(0) => &block[PARTITION1_START..(PARTITION1_START + PARTITION_INFO_LENGTH)],
            VolumeIdx(1) => &block[PARTITION2_START..(PARTITION2_START + PARTITION_INFO_LENGTH)],
            VolumeIdx(2) => &block[PARTITION3_START..(PARTITION3_START + PARTITION_INFO_LENGTH)],
            VolumeIdx(3) => &block[PARTITION4_START..(PARTITION4_START + PARTITION_INFO_LENGTH)],
            _ => {
                return Err(EPubError::<IO>::NoSuchVolume);
            }
        };
        // Only 0x80 and 0x00 are valid (bootable, and non-bootable)
        if (partition[PARTITION_INFO_STATUS_INDEX] & 0x7F) != 0x00 {
            return Err(EPubError::<IO>::FormatError("Invalid partition status"));
        }
        let lba_start = LittleEndian::read_u32(
            &partition[PARTITION_INFO_LBA_START_INDEX..(PARTITION_INFO_LBA_START_INDEX + 4)],
        );
        let num_blocks = LittleEndian::read_u32(
            &partition[PARTITION_INFO_NUM_BLOCKS_INDEX..(PARTITION_INFO_NUM_BLOCKS_INDEX + 4)],
        );
        (
            partition[PARTITION_INFO_TYPE_INDEX],
            BlockIdx(lba_start),
            BlockCount(num_blocks),
        )
    };
    Ok(Partition {
        part_type,
        lba_start,
        num_blocks,
    })
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
