
#![allow(dead_code, unused_imports)]

mod ffs;
mod formats;
mod fs_utils;
mod device;

use fs_utils::*;
use ffs::FFS;
use device::block_device::{self, BlockDevice};
use device::file_device::FileDevice;
use device::memory_device::MemoryDevice;
use formats::v1::format::FormatV1;

use std::io::{self};
use std::fs;


fn main() -> io::Result<()> {
	let path = "../../disks/disk1.img";
	let size = 100;

	let mut device = MemoryDevice::empty(size);
	FormatV1::format(&mut device)?;
	let mut ffs = FFS::mount(device)?;
	ffs.create_file("name.txt", FileType::File)?;
	ffs.create_file("fold", FileType::Directory)?;
	ffs.create_file("fold/inner.txt", FileType::File)?;
	ffs.delete_file("name.txt", FileType::File)?;
	ffs.delete_file("fold/inner.txt", FileType::File)?;
	ffs.delete_file("fold", FileType::Directory)?;
	ffs.save(path)?;

	println!("TS WORKS!");
	Ok(())
}
