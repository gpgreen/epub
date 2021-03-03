//! # Tests the EpubFile with the fatfs Library
//!
//! This example should be given a file or block device as the first and only
//! argument. It will attempt to mount all four possible primary MBR
//! partitions, one at a time, prints the root directory and will print a file
//! called "README.TXT". It will then list the contents of the "TEST"
//! sub-directory.
//!
//! ```bash
//! $ cargo run --example test_expand --features example -- /dev/mmcblk0
//! $ cargo run --example test_expand --features example -- /dev/sda
//! ```
//!
//! ```bash
//! zcat ./disk.img.gz > ./disk.img
//! $ cargo run --example test_expand -- ./disk.img
//! ```

const FILE_TO_PRINT: &'static str = "README.TXT";

use epub::EPubFile;
use fscommon::{BufStream, StreamSlice};
use heapless::{consts::*, String};
use mbr;
//use std::env;
use std::io::prelude::*;
use std::path::PathBuf;

use env_logger::{Builder, Target};

fn main() {
    let mut builder = Builder::from_default_env();
    builder.target(Target::Stdout);

    builder.init();

    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or("/dev/sdd1".into());
    println!("Using filename: {:?}", filename);

    //////////////////////////////////////////////////////////////

    let partitions = mbr::partition::read_partitions(PathBuf::from(filename.clone())).unwrap();
    // get the first partition of type '6'
    let mut fatpart: mbr::partition::Partition = partitions[0].clone();
    for (i, p) in partitions.iter().enumerate() {
        if p.p_type == 0x6 {
            fatpart = p.clone();
        }
        println!("Partition {}: {:?}", i, p);
    }

    let img_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(filename)
        .unwrap();

    // make the buf stream
    let first_lba: u64 = (fatpart.p_lba * 512).into();
    let last_lba: u64 = (fatpart.p_size * 512).into();
    let stream_partition = StreamSlice::new(img_file, first_lba, last_lba + 1).unwrap();
    let buf_stream = BufStream::new(stream_partition);

    // make the filesystem
    let mut fs = fatfs::FileSystem::new(buf_stream, fatfs::FsOptions::new()).unwrap();
    println!("fs created");

    //////////////////////////////////////////////////////////////
    {
        let root_dir = fs.root_dir();
        println!("\nListing root directory:");
        for r in root_dir.iter() {
            let entry = r.unwrap();
            println!("\t{}", entry.file_name());
        }
        println!("\nFinding {}...", FILE_TO_PRINT);
        let mut f = root_dir.open_file(FILE_TO_PRINT).unwrap();
        println!("Found {:?}", FILE_TO_PRINT);
        println!("\nFILE STARTS:");
        loop {
            let mut buffer = [0u8; 32];
            let num_read = f.read(&mut buffer).unwrap();
            if num_read == 0 {
                break;
            }
            for b in &buffer[0..num_read] {
                if *b == 10 {
                    print!("\\n");
                }
                print!("{}", *b as char);
            }
        }
        println!("EOF");
    }
    //////////////////////////////////////////////////////////////

    let epubname: String<U256> = String::from("RUSTPR~1.EPU");
    let epubfile = EPubFile::new(epubname);

    match epubfile.expand(EPubFile::EXPAND_DIR, &mut fs) {
        Ok(()) => println!("Inflated successfully!"),
        Err(_e) => println!("Inflation unsuccessful!"),
    }
}
