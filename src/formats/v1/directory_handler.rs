use crate::fs_utils::*;

use super::inode::{INode, FileType};
use super::directory::{Directory, DirEntry};
use crate::device::block_device::BlockDevice;

use std::io;
use std::path::{self, Path};


/// Handles the DATA blocks of a directory, since it has to deal with sub-block placement
pub struct DirectoryHandler {
	pub data_start: u32,
}

impl DirectoryHandler {
	pub fn new(data_start: u32) -> Self {
		Self { data_start }
	}

	pub fn traverse<D: BlockDevice>(&self, device: &mut D, path: &Path, root: u32) -> io::Result<u32> {
		//! starting from the given directory, returns the directory "root/path/"

		Ok(0)
	}

	pub fn can_fit<D: BlockDevice>(&self, device: &mut D, dir: &INode, entry: &mut DirEntry) -> io::Result<Option<(u32, u32)>> {
		//! checks whether the entry can be inserted in the directory without adding another data block.
		//! if it can fit, returns the block index where it fits and the offset of the entry that has to be shrunk.

		Ok(Some((0, 0)))
	}

	pub fn write_directory<D: BlockDevice>(&self, device: &mut D, directory: &Directory) -> io::Result<()> {
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(directory.inode, &mut buf)?;
		let inode = INode::deserialize(&buf);
		self.write_directory_with_inode(device, directory, &inode)
	}
	pub fn write_directory_with_inode<D: BlockDevice>(&self, device: &mut D, directory: &Directory, inode: &INode) -> io::Result<()> {
		let mut buf = [0u8; BLOCK_SIZE];
		let mut offset = 0;
		let mut data_block_dst = 0;

		for e in &directory.entries {
			if offset + e.record_len as usize >= BLOCK_SIZE {
				if data_block_dst >= 12 {
					todo!()
				}
			
				device.write_block(self.data_start + inode.direct[data_block_dst], &buf)?;
				buf = [0u8; BLOCK_SIZE];
				offset = 0;
				data_block_dst += 1;
			}

			e.serialize(&mut buf, offset);
			offset += e.record_len as usize;
		}

		device.write_block(self.data_start + inode.direct[data_block_dst], &buf)?;
		Ok(())
	}


	pub fn add_entry_here<D: BlockDevice>(&self, device: &mut D, dir: &INode, entry: &mut DirEntry, block: u32, offset_entry_to_shrink: u32) -> io::Result<()> {
		//! adds the file at the specified block, by shrinking the given entry.



		// shink the prev entry
		// expand the new entry
		// write the block.
	}
	pub fn add_entry_grow<D: BlockDevice>(&self, device: &mut D, dir: &INode, entry: &mut DirEntry, block: u32) -> io::Result<()> {
		//! adds the file at the specified block.
		//! NOTE: the inode itself isn't modified, so the caller should add the block to the direct/indirect and increase the inode.size

		entry.record_len = BLOCK_SIZE as u16;
		let mut buf = [0u8; BLOCK_SIZE];
		entry.serialize(&mut buf, 0);
		device.write_block(self.data_start + block, &buf)?;

		Ok(())
	}
	//pub fn add_file<D: BlockDevice>(&self, device: &mut D, dir: &mut INode, entry: &mut DirEntry, allocator: &BitmapAllocator) -> io::Result<()> {
	//	let res = self.can_fit(device, dir, entry)?;
	//	match res {
	//		None => {
	//			return self.add_file_grow(device, dir, entry);
	//		}
	//		Some(pos) => {
	//			let (block, offset) = pos;
	//			return self.add_file_here(device, dir, entry, block, offset);
	//		}
	//	}
	//}
}