use crate::fs_utils::*;
use crate::fs_error::*;
use crate::device::block_device::{self, BlockDevice};
use crate::device::memory_device::MemoryDevice;
use crate::formats::format::*;
use crate::file::{File, FileType};
use crate::formats::v1::format::FormatV1;

use std::io::{self, SeekFrom};
use std::path::Path;


pub struct FFS<D: BlockDevice> {
	device: D,
	format: Box<dyn FsFormat<D>>,
}


impl<D: BlockDevice> FFS<D> {
	pub fn mount(mut device: D) -> FSResult<Self> {
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(0, &mut buf)?;

		let version = read_version(&buf);
		let format: Box<dyn FsFormat<D>> = match version {
			Version::V1 => { Box::new(FormatV1::mount(&mut device)?) }
		};

		Ok(Self { device, format })
	}


	pub fn create_file(&mut self, path: &str) -> FSResult<()> {
		self.format.create_file(&mut self.device, path)
	}
	pub fn create_directory(&mut self, path: &str) -> FSResult<()> {
		self.format.create_directory(&mut self.device, path)
	}
	pub fn create_symlink(&mut self, path: &str, tgt: &str) -> FSResult<()> {
		self.format.create_symlink(&mut self.device, path, tgt)
	}

	pub fn delete(&mut self, path: &str) -> FSResult<()> {
		self.format.delete(&mut self.device, path)
	}

	pub fn link(&mut self, src: &str, dst: &str) -> FSResult<()> {
		self.format.link(&mut self.device, src, dst)
	}

	pub fn file_exists(&mut self, path: &str) -> FSResult<(bool, Option<FileType>)> {
		self.format.file_exists(&mut self.device, path)
	}
	pub fn open_file(&mut self, path: &str) -> FSResult<File> {
		self.format.open_file(&mut self.device, path)
	}
	pub fn close_file(&mut self, file: &File) -> FSResult<()> {
		self.format.close_file(&mut self.device, file)
	}
	
	pub fn read(&mut self, file: &File, buf: &mut [u8]) -> FSResult<usize> {
		self.format.read(&mut self.device, file, buf)
	}
	pub fn write(&mut self, file: &File, buf: &[u8]) -> FSResult<usize> {
		self.format.write(&mut self.device, file, buf)
	}
	pub fn seek(&mut self, file: &File, pos: SeekFrom) -> FSResult<u64> {
		self.format.seek(&mut self.device, file, pos)
	}
	
	pub fn get_directory_content(&mut self, path: &str) -> FSResult<DirectoryContentResult> {
		self.format.get_directory_content(&mut self.device, path)
	}

	
	pub fn free_space(&mut self) -> FSResult<usize> {
		self.format.free_space(&mut self.device)
	}
	

	pub fn check_integrity(&mut self) -> FSResult<IntegrityResult> {
		self.format.check_integrity(&mut self.device)
	}
}


impl FFS<MemoryDevice> {
	pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
		self.device.save(path)
	}
}