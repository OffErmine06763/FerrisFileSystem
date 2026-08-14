use crate::fs_utils::*;
use crate::device::block_device::BlockDevice;
use super::file::FileIO;

use std::io;


pub struct DirectoryContentEntry {
	pub filename: String,
	pub file_type: FileType,
}

pub struct DirectoryContentResult {
	pub entries: Vec<DirectoryContentEntry>,
	// add maybe stuff as total size (including the directory used space)...
}


pub type File = Box<dyn FileIO>;


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
	fn create_file(&mut self, device: &mut D, path: &str, file_type: FileType) -> io::Result<File>;
	fn delete_file(&mut self, device: &mut D, path: &str, file_type: FileType) -> io::Result<()>;
	
	fn open_file(&mut self, device: &mut D, path: &str) -> io::Result<File>;
	fn get_directory_content(&mut self, device: &mut D, path: &str) -> io::Result<DirectoryContentResult>;

	fn file_exists(&mut self, device: &mut D, path: &str) -> io::Result<(bool, Option<FileType>)>;

	fn free_space(&mut self, device: &mut D) -> io::Result<usize>;
	fn check_integrity(&self, device: &mut D) -> io::Result<IntegrityResult>;
}