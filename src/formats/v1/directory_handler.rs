use crate::fs_utils::*;
use crate::device::block_device::BlockDevice;
use crate::fs_error::*;
use crate::file::FileType;

use super::inode::INode;
use super::directory::{Directory, DirEntry};
use super::inode_handler::INodeTableHandler;

use std::io;
use std::path::{self, Path, Component, PathBuf};


/// Handles the DATA blocks of a directory, since it has to deal with sub-block placement
pub struct DirectoryHandler {
	pub data_start: u32,
}

impl DirectoryHandler {
	pub fn new(data_start: u32) -> Self {
		Self { data_start }
	}

	// TODO: create functions to rearrange entries to optimize space usage



	pub fn find_entry<D: BlockDevice>(&self, device: &mut D, dir_inode: &INode, name: &[u8]) -> FSResult<Option<(DirEntry, u32, usize)>> {
		//! returns the DirEntry named 'name' in the specified directory, with RELATIVE data block index and offset in it where the entry is located.

		let mut res: FSResult<Option<(DirEntry, u32, usize)>> = Ok(None);
		
		self.iterate(device, dir_inode, |entry: DirEntry, block: u32, offset: usize| -> bool {
			if !entry.is_free() && entry.name[0..entry.name_len as usize] == *name {
				res = Ok(Some((entry, block, offset)));
				return true;
			}

			return false;
		})?;

		return res;
	}


	pub fn traverse<D: BlockDevice>(&self, device: &mut D, path: &Path, root: u32, inode_handler: &INodeTableHandler) -> FSResult<u32> {
		//! starting from the given root directory, returns the directory "root/path/"
		//! throws: DirectoryDoesNotExist or NotADirectory
		
		let mut cur_inode = root;
		let mut cur_dir = inode_handler.read_inode(device, root)?;
		
		// for every component of the path
		let mut progress = PathBuf::new();
		for comp in path.components() {
			// comp can be CurDir or RootDir only at the start of the path
			if comp == Component::CurDir || comp == Component::RootDir { 
				continue;
			}
			progress = progress.join(comp);

			let comp_name = Self::path_component_bytes(&comp);

			// check if the current directory contains the component
			let entry = self.find_entry(device, &cur_dir, comp_name)?;
			if entry.is_none() {
				return Err(FSError::DirectoryDoesNotExist{ path: progress.to_string_lossy().into_owned() });
			}
			cur_inode = entry.unwrap().0.inode;
			if cur_inode == INVALID_ADDRESS {
				return Err(FSError::InvalidDirEntry(InvalidDirEntryKind::InvalidINode));
			}

			// check if it is a directory
			cur_dir = inode_handler.read_inode(device, cur_inode)?;
			if cur_dir.file_type != FileType::Directory {
				return Err(FSError::NotADirectory{ path: progress.to_string_lossy().into_owned() });
			}
		}

		Ok(cur_inode)
	}


	pub fn can_fit<D: BlockDevice>(&self, device: &mut D, dir: &INode, to_insert: &DirEntry) -> FSResult<Option<(u32, u16)>> {
		//! checks whether the entry can be inserted in the directory without adding another data block.
		//! if it can fit, returns the block index where it fits and the offset of the offset of the free region to use.

		let mut res: FSResult<Option<(u32, u16)>> = Ok(None);
		
		self.iterate(device, dir, |entry: DirEntry, block: u32, offset: usize| -> bool {
			// we return the first free region big enough.
			// deletion behavior guarantees that there will not be two contiguous free regions
			if entry.is_free() && entry.record_len >= to_insert.record_len {
				res = Ok(Some((block, offset as u16)));
				return true;
			}

			return false;
		})?;

		return res;
	}


	pub fn write_directory_with_inode<D: BlockDevice>(&self, device: &mut D, directory: &Directory, inode: &INode) -> FSResult<()> {
		//! writes the directory entries in the order provided.
		//! expects the directory inode to have enough allocated blocks.
		//! the entries must be correctly formatted: each block utilized must be filled entirely
		//! throws: InvalidInput(DirEntriesOverfillBlock, DirEntriesOverfillBlock)  // TODO: for all important functions add a proper throws and return

		// First pass: validate the directory layout.
		let mut offset = 0usize;
		let mut data_block_dst = 0usize;

		for e in &directory.entries {
			let record_len = e.record_len as usize;

			if offset + record_len > BLOCK_SIZE {
				return Err(FSError::InvalidInput(InvalidInputKind::DirEntriesOverfillBlock));
			}

			offset += record_len;

			if offset == BLOCK_SIZE {
				data_block_dst += 1;
				offset = 0;
			}
		}

		// The directory must end exactly on a block boundary.
		if offset != 0 {
			return Err(FSError::InvalidInput(InvalidInputKind::DirEntriesUnderfillBlock));
		}

		if data_block_dst > 12 {
			todo!(); // support indirect
		}


		// Second pass: perform the writes.
		let mut buf = [0u8; BLOCK_SIZE];
		let mut offset = 0usize;
		let mut data_block_dst = 0usize;

		for e in &directory.entries {
			let record_len = e.record_len as usize;

			e.serialize(&mut buf, offset);
			offset += record_len;

			if offset == BLOCK_SIZE {
				device.write_block(self.data_start + inode.direct[data_block_dst], &buf)?;

				buf = [0u8; BLOCK_SIZE];
				offset = 0;
				data_block_dst += 1;
			}
		}

		Ok(())
	}


	pub fn add_entry_here<D: BlockDevice>(&self, device: &mut D, _dir: &INode, entry: &mut DirEntry, block: u32, free_region_offset: u16) -> FSResult<()> {
		//! adds the file at the specified block, by shrinking the given free region.
		//! throws: InvalidInput(DirFreeRegionTooSmall, DiEntryNotFree)

		// read the whole block (might contain other entries)
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(self.data_start + block, &mut buf)?;

		// deserialize the free region data
		let mut free_region = DirEntry::deserialize(&buf, free_region_offset as usize);
		if free_region.record_len < entry.record_len {
			return Err(FSError::DirFreeRegionTooSmall);
		} if !free_region.is_free() {
			return  Err(FSError::DirEntryNotFree);
		}

		// move it forward (if not fully utilized)
		free_region.record_len -= entry.record_len;
		if free_region.record_len > 0 {
			free_region.serialize(&mut buf, (free_region_offset + entry.record_len) as usize);
		}

		// serialize the new entry
		entry.serialize(&mut buf, free_region_offset as usize);
		
		device.write_block(self.data_start + block, &buf)?;

		Ok(())
	}
	pub fn add_entry_grow<D: BlockDevice>(&self, device: &mut D, _dir: &INode, entry: &DirEntry, block: u32) -> FSResult<()> {
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


	pub fn free_region<D: BlockDevice>(&self, device: &mut D, _dir: &INode, block: u32, offset: u16, size: u16) -> FSResult<()> {
		//! marks the specified region as free, potentially merging with adjacent free regions
		//! throws: InvalidInput(OffsetNotAtDirEntryStart)
		
		// read the whole block (might contain other entries)
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(self.data_start + block, &mut buf)?;

		let mut free_region_size = size;
		let mut free_region_start = offset;


		// parse the next entry
		let next_offset = offset as usize + size as usize;
		if next_offset < BLOCK_SIZE {
			// TODO: this can fail if the provided offset/size are invalid.
			let next_entry = DirEntry::deserialize(&buf, next_offset);
			if next_entry.is_free() {
				free_region_size += next_entry.record_len;
			}
		}
		

		// find the previous entry
		// TODO: this might be optimized by returning it when looking for the entry to remove.
		let mut prev_offset = 0;
		let mut prev_entry = None;
		while prev_offset < offset {
			let parsed = DirEntry::deserialize(&buf, prev_offset as usize);
			if prev_offset + parsed.record_len > offset {
				return Err(FSError::InvalidInput(InvalidInputKind::OffsetNotAtDirEntryStart));
			}
			if prev_offset + parsed.record_len == offset {
				prev_entry = Some(parsed);
				break;
			}
			prev_offset += parsed.record_len;
		}

		match prev_entry {
			Some(e) => if e.is_free() {
				free_region_size += e.record_len;
				free_region_start = prev_offset;
			}
			_ => {}
		}


		// create the entry corresponding to the new free region
		let free_entry = DirEntry::free(free_region_size);
		free_entry.serialize(&mut buf, free_region_start as usize);
		
		device.write_block(self.data_start + block, &buf)?;

		Ok(())
	}


	pub fn get_entries<D: BlockDevice>(&self, device: &mut D, dir: &INode, include_free: bool) -> FSResult<Vec<DirEntry>> {
		//! returns the contents of the directory

		let mut entries = Vec::<DirEntry>::new();
		
		// perform this check only once, even if it means duplicating some code.
		if include_free {
			self.iterate(device, dir, |entry: DirEntry, _block: u32, _offset: usize| -> bool {
				entries.push(entry);
				return false;
			})?;
		}
		else 
		{
			self.iterate(device, dir, |entry: DirEntry, _block: u32, _offset: usize| -> bool {
				if !entry.is_free() {
					entries.push(entry);
				}
				return false;
			})?;
		}

		Ok(entries)
	}


	pub fn iterate<D: BlockDevice, F: FnMut(DirEntry, u32, usize) -> bool>(&self, device: &mut D, dir_inode: &INode, mut callback: F) -> FSResult<()> {
		//! iterate over the entries of the given directory, calling the provided callback with also the RELATIVE data block index and offset in the block.
		//! the callback should return true to stop iterating.
		//! the function always throws AFTER passing the problematic entry to the callback, and throws regardless of the callback return value.
		//! throws: InvalidDir(EntriesOverfillBlock) InvalidDirEntry(ZeroLength)

		let mut buf = [0u8; BLOCK_SIZE];

		// for every block used by the given directory
		for block in 0..dir_inode.blocks {
			let absolute_block = dir_inode.direct[block as usize] + self.data_start;
			device.read_block(absolute_block, &mut buf)?;

			// parse all the entries
			let mut offset = 0;
			while offset < BLOCK_SIZE {
				let parsed = DirEntry::deserialize(&buf, offset);
				let len = parsed.record_len as usize;
				
				let res = callback(parsed, dir_inode.direct[block as usize], offset);

				if offset + len > BLOCK_SIZE {
					return Err(FSError::InvalidDir(InvalidDirKind::EntriesOverfillBlock));
				} if len == 0 {
					return Err(FSError::InvalidDirEntry(InvalidDirEntryKind::ZeroLength));
				}

				if res { return Ok(()); }

				offset += len;
			}
		}

		Ok(())
	}
	pub fn iterate_block_unchecked<D: BlockDevice, F: FnMut(DirEntry, u32, usize) -> bool>(&self, device: &mut D, block_ind: u32, mut callback: F) -> FSResult<()> {
		//! iterates over the entries in the provided block, without checking if the last entry overflows to the next block
		let mut buf = [0u8; BLOCK_SIZE];

		let absolute_block = block_ind + self.data_start;
		device.read_block(absolute_block, &mut buf)?;

		// parse all the entries
		let mut offset = 0;
		while offset < BLOCK_SIZE {
			let parsed = DirEntry::deserialize(&buf, offset);
			let len = parsed.record_len as usize;

			let res = callback(parsed, block_ind, offset);
			if res { return Ok(()); }

			offset += len;
		}

		Ok(())
	}



	fn path_component_bytes<'a>(component: &'a Component<'a>) -> &'a [u8] {
		match component {
			Component::Prefix(prefix) => prefix.as_os_str().as_encoded_bytes(),
			Component::RootDir        => b"/",
			Component::CurDir         => b".",
			Component::ParentDir      => b"..",
			Component::Normal(name)   => name.as_encoded_bytes(),
		}
	}
}







#[test]
fn directory_handler() -> io::Result<()> {
	return Ok(());

	// TODO: test adding and removing entries from a "fake" directory.
	//		 don't even have to format the device, just to test if it handles
	//       correctly the variable size used/free entries.
}

