use crate::fs_utils::*;

use super::inode::INode;
use super::directory::{Directory, DirEntry};
use crate::device::block_device::BlockDevice;
use super::inode_handler::INodeTableHandler;

use std::io;
use std::path::{self, Path, Component};


/// Handles the DATA blocks of a directory, since it has to deal with sub-block placement
pub struct DirectoryHandler {
	pub data_start: u32,
}

impl DirectoryHandler {
	pub fn new(data_start: u32) -> Self {
		Self { data_start }
	}


	pub fn traverse<D: BlockDevice>(&self, device: &mut D, path: &Path, root: u32, inode_handler: &INodeTableHandler) -> io::Result<u32> {
		//! starting from the given root directory, returns the directory "root/path/"
		
		let mut buf = [0u8; BLOCK_SIZE];

		let mut cur_inode = root;
		let mut cur_dir = inode_handler.read_inode(device, root)?;
		
		// for every component of the path
		for comp in path.components() {
			// comp can be CurDir or RootDir only at the start of the path
			if comp == Component::CurDir || comp == Component::RootDir { continue; }

			let comp_name = Self::path_component_bytes(&comp);
			let mut found = false;

			// for every block used by the current directory
			for block in 0..cur_dir.blocks {
				let absolute_block = cur_dir.direct[block as usize] + self.data_start;
				device.read_block(absolute_block, &mut buf)?;

				// parse all the entries
				let mut offset = 0;
				while offset < BLOCK_SIZE {
					let parsed = DirEntry::deserialize(&buf, offset);
					if parsed.is_free() { continue; }
					if parsed.record_len == 0 {
						return Err(io::Error::new(io::ErrorKind::InvalidData, "directory entry has zero record length"));
					}

					if parsed.name[0..parsed.name_len as usize] == *comp_name {
						cur_inode = parsed.inode;
						cur_dir = inode_handler.read_inode(device, parsed.inode)?;

						if cur_dir.file_type != FileType::Directory {
							return Err(io::Error::new(io::ErrorKind::NotADirectory, "path component is not a directory"));
						}

						found = true;
						break;
					}

					offset += parsed.record_len as usize;
				}

				if found { break; }
			}

			if !found {
				return Err(io::Error::new(io::ErrorKind::NotFound, "directory doesn't exist"));
			}
		}

		Ok(cur_inode)
	}


	pub fn can_fit<D: BlockDevice>(&self, device: &mut D, dir: &INode, entry: &mut DirEntry) -> io::Result<Option<(u32, u16)>> {
		//! checks whether the entry can be inserted in the directory without adding another data block.
		//! if it can fit, returns the block index where it fits and the offset of the offset of the free region to use.

		let mut buf = [0u8; BLOCK_SIZE];

		// for every block used by the directory
		for block in 0..dir.blocks {
			let absolute_block = dir.direct[block as usize] + self.data_start;
			device.read_block(absolute_block, &mut buf)?;

			// parse all the entries
			let mut offset = 0;
			while offset < BLOCK_SIZE {
				let parsed = DirEntry::deserialize(&buf, offset);

				// we return the first free region big enough.
				// deletion behavior guarantees that there will not be two contiguous free regions
				if parsed.is_free() && parsed.record_len >= entry.record_len {
					return Ok(Some((dir.direct[block as usize], offset as u16)));
				}

				offset += parsed.record_len as usize;
			}
		}

		Ok(None)
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
			if offset + e.record_len as usize > BLOCK_SIZE {
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


	pub fn add_entry_here<D: BlockDevice>(&self, device: &mut D, dir: &INode, entry: &mut DirEntry, block: u32, free_region_offset: u16) -> io::Result<()> {
		//! adds the file at the specified block, by shrinking the given free region.

		// read the whole block (might contain other entries)
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(self.data_start + block, &mut buf)?;

		// deserialize the free region data and move it forward (if not fully utilized)
		let mut free_region = DirEntry::deserialize(&buf, free_region_offset as usize);
		free_region.record_len -= entry.record_len;
		if free_region.record_len > 0 {
			free_region.serialize(&mut buf, (free_region_offset + entry.record_len) as usize);
		}

		// serialize the new entry
		entry.serialize(&mut buf, free_region_offset as usize);
		
		device.write_block(self.data_start + block, &buf)?;

		Ok(())
	}
	pub fn add_entry_grow<D: BlockDevice>(&self, device: &mut D, dir: &INode, entry: &DirEntry, block: u32) -> io::Result<()> {
		//! adds the file at the specified block.
		//! NOTE: the inode itself isn't modified, so the caller should add the block to the direct/indirect and increase the inode.size

		let mut buf = [0u8; BLOCK_SIZE];
		
		entry.serialize(&mut buf, 0);

		if BLOCK_SIZE as u16 - entry.record_len > 0 {
			let free = DirEntry::free(BLOCK_SIZE as u16 - entry.record_len);
			free.serialize(&mut buf, entry.record_len as usize);
		}
		
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




	fn path_component_bytes<'a>(component: &'a Component<'a>) -> &'a [u8] {
		match component {
			Component::Prefix(prefix) => prefix.as_os_str().as_encoded_bytes(),
			Component::RootDir => b"/",
			Component::CurDir => b".",
			Component::ParentDir => b"..",
			Component::Normal(name) => name.as_encoded_bytes(),
		}
	}
}