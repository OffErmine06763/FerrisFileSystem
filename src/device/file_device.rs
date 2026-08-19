use crate::fs_utils::*;
use crate::fs_error::*;
use super::block_device::BlockDevice;

use std::fs::{self, File, OpenOptions};
use std::io::{self, SeekFrom, Seek, Read, Write};
use std::path::Path;


/// Represents a storage system stored in a file
pub struct FileDevice {
	file: File,
	/// number of blocks
	size: u32,
}


impl BlockDevice for FileDevice {
	fn read_block(&mut self, block: u32, buf: &mut [u8; BLOCK_SIZE]) -> FSResult<()> {
		if block >= self.block_count() {
			return Err(FSError::InvalidInput(InvalidInputKind::BlockIndexOOB));
		}
		self.file.seek(SeekFrom::Start(block as u64 * BLOCK_SIZE as u64))?;
		self.file.read_exact(buf)?;
		Ok(())
	}
	fn write_block(&mut self, block: u32, buf: &[u8; BLOCK_SIZE]) -> FSResult<()> {
		if block >= self.block_count() {
			return Err(FSError::InvalidInput(InvalidInputKind::BlockIndexOOB));
		}
		self.file.seek(SeekFrom::Start(block as u64 * BLOCK_SIZE as u64))?;
		self.file.write_all(buf)?;
		Ok(())
	}
	fn block_count(&self) -> u32 {
		self.size
	}
	fn resize(&mut self, blocks: u32) -> FSResult<()> {
		self.size = blocks;
		self.file.set_len(blocks as u64 * BLOCK_SIZE as u64)?;
		Ok(())
	}
}

impl FileDevice {
	pub fn create_disk_file<P: AsRef<Path>>(path: P, blocks: u32) -> io::Result<File> {
		//! creates an empty disk image file of the requested size.
		//! same as MemoryDevice::empty(), but this has the permantent effect of creating a file.
		let file = File::create(path)?;
		file.set_len(blocks as u64 * BLOCK_SIZE as u64)?;
		Ok(file)
	}
	pub fn from_file(file: File) -> io::Result<Self> {
		//! file must be r/w
		let size = file.metadata()?.len();
		Ok(Self { file, size: (size / BLOCK_SIZE as u64) as u32 })
	}
	pub fn from_path<P: AsRef<Path>>(path: P) -> io::Result<Self> {
		let file = OpenOptions::new()
			.read(true).write(true)
			.open(path)?;

		Self::from_file(file)
	}
}






#[test]
fn file_device() -> FSResult<()> {
	FileDevice::create_disk_file("test_file_device.img", 2)?;
	let mut device = FileDevice::from_path("test_file_device.img")?;
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

	let res = device.read_block(100, &mut buf);
	assert_error_code(res, FSErrorCode::InputBlockIndexOOB);
	let res = device.write_block(100, &mut buf);
	assert_error_code(res, FSErrorCode::InputBlockIndexOOB);


	device = FileDevice::from_path("test_file_device.img")?;
	assert_eq!(device.block_count(), 5);

	device.read_block(0, &mut buf)?;
	assert_eq!(buf, [2u8; BLOCK_SIZE]);
	device.read_block(1, &mut buf)?;
	assert_eq!(buf, [2u8; BLOCK_SIZE]);

	
	fs::remove_file("test_file_device.img")?;
	Ok(())
}