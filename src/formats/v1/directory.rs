use crate::fs_utils::*;

use std::io;

// A directory is an inode.
// The data blocks associated to it contains the entries of the directory (other directories or files)
// An entry is just the inode of the file/sub-directory and its name
// Each entry can have variable size (depending on the name length).
// Record length is the size of an entry on the device:
// - for allocated entries it is equal to the area necessary to store it
// - for free regions (at the end of the block or internally after deletions) it is equal to the free area
// A free region has invalid inode index.

pub struct DirEntry {
	pub inode: u32,
	pub file_type: FileType,
	/// length of the entry on the device (or of the free region)
	pub record_len: u16,
	pub name_len: u16,
	pub name: [u8; Self::MAX_NAME],
}

impl DirEntry {
	pub const MAX_NAME: usize = 64;

	pub fn new(inode: u32, file_type: FileType, name: &str) -> Self {
		//! returns a new entry, with record_len equal to the minimum device memory necessary to store it (plus alignment)
		let mut name_arr = [0u8; 64];
		let bytes = name.as_bytes();

		let name_len = bytes.len().min(name_arr.len());
		name_arr[..name_len].copy_from_slice(&bytes[..name_len]);

		let record_len = Self::min_record_len(name_len as u16);
		Self { inode, file_type, record_len, name_len: name_len as u16, name: name_arr }
	}
	pub fn free(size: u16) -> Self {
		//! create an entry that represents a free region
		Self { inode: INVALID_ADDRESS, file_type: FileType::File, record_len: size, name_len: 0, name: [0u8; Self::MAX_NAME] }
	}

	pub fn is_free(&self) -> bool {
		return self.inode == INVALID_ADDRESS;
	}


	pub fn min_record_len(name_len: u16) -> u16 {
		// alignment!!
		// 1 2 3 4 -> 4
		// raw  : 0001 0010 0011 0100 -> 0100
		// add 3: 0100 0101 0110 0111
		// shift: 0100 0100 0100 0100
		let mut res = 4 + 1 + 2 + 2 + 1 * name_len;
		res += 3;
		(res >> 2) << 2
	}

	pub fn serialize(&self, buf: &mut [u8; BLOCK_SIZE], init_offset: usize) {
		let mut offset = init_offset;

		macro_rules! write_field {
			($value:expr) => {{
				let bytes = $value.to_le_bytes();
				buf[offset..offset + bytes.len()].copy_from_slice(&bytes);
				offset += bytes.len();
			}};
		}

		write_field!(self.inode);
		write_field!(self.record_len);
		write_field!(self.name_len);
		write_field!(self.file_type);
		buf[offset..offset + self.name_len as usize].copy_from_slice(&self.name[..self.name_len as usize]);
		offset += self.name_len as usize;
		assert!(offset <= init_offset + self.record_len as usize, "directory entry record length is less than the minimum space required");
		buf[offset..init_offset + self.record_len as usize].fill(0);
	}

	pub fn deserialize(buf: &[u8], init_offset: usize) -> Self {
		let mut offset = init_offset;

		macro_rules! read_field {
			($ty:ty) => {{
				let size = core::mem::size_of::<$ty>();
				let value = <$ty>::from_le_bytes(
					buf[offset..offset + size]
						.try_into()
						.expect("buffer too small"),
				);
				offset += size;
				value
			}};
		}

		let mut res = Self {
			inode: read_field!(u32),
			record_len: read_field!(u16),

			name_len: read_field!(u16),
			file_type: read_field!(FileType),

			name: [0u8; 64],
		};
		res.name[..res.name_len as usize].copy_from_slice(&buf[offset..offset + res.name_len as usize]);
		res
	}
}

pub struct Directory {
	pub inode: u32,
	pub entries: Vec<DirEntry>,
}

impl Directory {
	pub fn new(inode: u32, parent: u32) -> Self {
		let self_entry = DirEntry::new(inode, FileType::Directory, ".");
		let parent_entry = DirEntry::new(parent, FileType::Directory, "..");
		let free_entry = DirEntry::free(BLOCK_SIZE as u16 - self_entry.record_len - parent_entry.record_len);

		Self { inode, entries: Vec::<DirEntry>::from([self_entry, parent_entry, free_entry]) }
	}

	//pub fn lookup(&self, name: &str) -> Option<u32> {
	//	Option::Some(1)
	//}

	//pub fn insert(
	//	&mut self,
	//	name: &str,
	//	inode: u32,
	//) -> io::Result<()> {
	//	Ok(())
	//}

	//pub fn remove(
	//	&mut self,
	//	name: &str,
	//) -> io::Result<()> {
	//	Ok(())
	//}

	//pub fn rename(
	//	&mut self,
	//	old: &str,
	//	new: &str,
	//) -> io::Result<()> {
	//	Ok(())
	//}

	//pub fn list(&self) -> Vec<DirEntry> {
	//	Vec::<DirEntry>::new()
	//}
}