use crate::fs_utils::*;
use crate::device::block_device::BlockDevice;
use crate::file::{File, FileType};
use crate::fs_error::*;

use std::io::{self, SeekFrom};


pub struct DirectoryContentEntry {
	pub filename: String,
	pub file_type: FileType,
}

pub struct DirectoryContentResult {
	pub entries: Vec<DirectoryContentEntry>,
	// add maybe stuff as total size (including the directory used space)...
}


pub trait IntegrityError {
	// todo: i dont like the idea of "is_recoverable"
	//       everything is, sometimes it's trivial, other times it requires taking a decision
	//       maybe change this to "is_usable" for when the state is inconsistent but still works
	fn is_recoverable(&self) -> bool;
	fn to_string(&self) -> String;
}

pub struct IntegrityResult {
	pub errors: Vec<Box<dyn IntegrityError>>,
}

impl IntegrityResult {
	pub fn is_ok(&self) -> bool {
		self.errors.len() == 0
	}

	pub fn is_recoverable(&self) -> bool {
		for e in &self.errors {
			if !e.is_recoverable() {
				return false;
			}
		}

		return true;
	}
}


pub trait FsFormat<D: BlockDevice> {
	fn create_file(&mut self, device: &mut D, path: &str) -> FSResult<()>;
	fn create_directory(&mut self, device: &mut D, path: &str) -> FSResult<()>;
	fn create_symlink(&mut self, device: &mut D, path: &str, path_tgt: &str) -> FSResult<()>;
	fn create_hardlink(&mut self, device: &mut D, from: &str, to: &str) -> FSResult<()>;

	fn delete(&mut self, device: &mut D, path: &str) -> FSResult<()>;
	

	fn file_exists(&mut self, device: &mut D, path: &str) -> FSResult<(bool, Option<FileType>)>;
	fn open_file(&mut self, device: &mut D, path: &str) -> FSResult<File>;
	fn close_file(&mut self, device: &mut D, file: &File) -> FSResult<()>;

	fn read(&mut self, device: &mut D, file: &File, buf: &mut [u8]) -> FSResult<usize>;
	fn write(&mut self, device: &mut D, file: &File, buf: &[u8]) -> FSResult<usize>;
	fn seek(&mut self, device: &mut D, file: &File, pos: SeekFrom) -> FSResult<u64>;
	fn truncate(&mut self, device: &mut D, file: &File, size: u64) -> FSResult<()>;

	fn edit_symlink(&mut self, device: &mut D, path: &str, path_tgt: &str) -> FSResult<()>;

	fn get_directory_content(&mut self, device: &mut D, path: &str) -> FSResult<DirectoryContentResult>;
	
	fn free_space(&mut self, device: &mut D) -> FSResult<usize>;
	
	fn check_integrity(&self, device: &mut D) -> FSResult<IntegrityResult>;
}