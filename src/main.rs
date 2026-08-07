#![allow(dead_code, unused_imports)]

mod ffs;
mod formats;
mod fs_utils;
mod device;

use ffs::FFS;
use device::block_device::{self, BlockDevice};
use device::file_device::FileDevice;
use device::memory_device::MemoryDevice;
use formats::v1::format::FormatV1;

use std::io::{self};
use std::fs;


fn main() -> io::Result<()> {
	let path = "../../disks/disk1.img";
	let size = 10;

	let mut device = MemoryDevice::empty(size);
	FormatV1::format(&mut device)?;
	let mut ffs = FFS::mount(device)?;
	ffs.create_file("path")?;
	ffs.save(path)?;


	//if !fs::exists(path)? {
	//	FileDevice::create_disk(path, size)?;
	//}
	//let mut device = FileDevice::from_path(path)?;

	// let mut device = MemoryDevice::empty(size);
	// let mut device = MemoryDevice::from_file(path)?;
	
	// let mut buf: [u8; block_device::BLOCK_SIZE] = [0; block_device::BLOCK_SIZE];
	
	// buf.fill('a' as u8);
	// device.write_block(2, &buf)?;

	// device.read_block(2, &mut buf)?;
	// println!("{}", String::from_utf8_lossy(&buf));

	// buf.fill('b' as u8);
	// device.write_block(1, &buf)?;

	// device.resize(10)?;

	// device.save(path)?;

	println!("TS WORKS!");
	Ok(())
}
