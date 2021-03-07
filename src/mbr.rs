use crate::EPubError;
use byteorder::{ByteOrder, LittleEndian};
use fatfs::{File, OemCpConverter, Read, ReadWriteSeek, TimeProvider};

//use log::{info, trace};

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

/* Constants for type of partitions, not used here
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
 */

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

#[cfg(test)]
mod tests {
    use super::*;
}
