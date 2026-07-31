use crate::fs_utils::*;
use super::block_device::BlockDevice;

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;


/// Represents a storage system stored in memory
pub struct MemoryDevice {
	blocks: Vec<[u8; BLOCK_SIZE]>,
}


impl BlockDevice for MemoryDevice {
	fn read_block(&mut self, block: u32, buf: &mut [u8; BLOCK_SIZE]) -> io::Result<()> {
		if block >= self.block_count() {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "block out of range"));
		}
		buf.copy_from_slice(&self.blocks[block as usize]);
		Ok(())
	}
	fn write_block(&mut self, block: u32, buf: &[u8; BLOCK_SIZE]) -> io::Result<()> {
		if block >= self.block_count() {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "block out of range"));
		}
		self.blocks[block as usize] = *buf;
		Ok(())
	}
	fn block_count(&self) -> u32 {
		self.blocks.len() as u32
	}
	fn resize(&mut self, blocks: u32) -> io::Result<()> {
		self.blocks.resize(blocks as usize, [0u8; BLOCK_SIZE]);
		Ok(())
	}
}

impl MemoryDevice {
	pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
		let mut file = File::open(path)?;
		let mut blocks = Vec::new();

		loop {
			let mut block = [0u8; BLOCK_SIZE];

			match file.read_exact(&mut block) {
				Ok(()) => { blocks.push(block); }
				Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => { break; }
				Err(e) => { return Err(e); }
			}
		}

		Ok(MemoryDevice { blocks })
	}
	pub fn empty(blocks: u64) -> Self {
		//! same as FileDevice::empty(), but without the permantent effect of creating a file.
		MemoryDevice { blocks: vec![[0u8; BLOCK_SIZE]; blocks as usize] }
	}
	pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
		let mut file = OpenOptions::new().write(true).open(path)?;

		for block in &self.blocks {
			file.write_all(block)?;
		}
		file.set_len(self.block_count() as u64 * BLOCK_SIZE as u64)?;

		Ok(())
	}
}








#[test]
fn memory_device() -> io::Result<()> {
	let mut device = MemoryDevice::empty(2);
	assert_eq!(device.block_count(), 2);
	
	let mut buf: [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
	
	buf.fill(1u8);
	device.write_block(1, &buf)?;
	device.read_block(1, &mut buf)?;
	assert_eq!(buf, [1u8; BLOCK_SIZE]);

	buf.fill(2u8);
	device.write_block(0, &buf)?;
	device.read_block(0, &mut buf)?;
	assert_eq!(buf, [2u8; BLOCK_SIZE]);
	device.read_block(1, &mut buf)?;
	assert_eq!(buf, [1u8; BLOCK_SIZE]);
	
	buf.fill(2u8);
	device.write_block(1, &buf)?;
	device.read_block(1, &mut buf)?;
	assert_eq!(buf, [2u8; BLOCK_SIZE]);

	device.resize(10)?;
	assert_eq!(device.block_count(), 10);

	buf.fill(3u8);
	device.write_block(6, &buf)?;
	device.read_block(6, &mut buf)?;
	assert_eq!(buf, [3u8; BLOCK_SIZE]);

	device.resize(5)?;
	assert_eq!(device.block_count(), 5);

	let res = device.read_block(100, &mut buf).unwrap_err();
	assert_eq!(res.kind(), io::ErrorKind::InvalidInput);
	assert_eq!(res.to_string(), "block out of range");
	let res = device.write_block(100, &mut buf).unwrap_err();
	assert_eq!(res.kind(), io::ErrorKind::InvalidInput);
	assert_eq!(res.to_string(), "block out of range");


	File::create("test_memory_device.img")?;
	device.save("test_memory_device.img")?;

	device.resize(0)?;
	device = MemoryDevice::from_file("test_memory_device.img")?;
	assert_eq!(device.block_count(), 5);

	device.read_block(0, &mut buf)?;
	assert_eq!(buf, [2u8; BLOCK_SIZE]);
	device.read_block(1, &mut buf)?;
	assert_eq!(buf, [2u8; BLOCK_SIZE]);

	
	fs::remove_file("test_memory_device.img")?;
	Ok(())
}