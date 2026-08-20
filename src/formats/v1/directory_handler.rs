use crate::fs_utils::*;
use crate::device::block_device::BlockDevice;
use crate::fs_error::*;
use crate::file::FileType;

use super::inode::INode;
use super::directory::{Directory, DirEntry};
use super::inode_handler::INodeTableHandler;

use std::io;
use std::path::{self, Path, Component, PathBuf};


pub struct ResolutionResult {
	pub target_inode: INode,
	pub target_inode_index: u32,
	pub parent_inode_index: u32,
	pub dir_entry: DirEntry,
	pub dir_entry_block_index: u32,
	pub dir_entry_offset: usize,
}


/// Handles the DATA blocks of a directory, since it has to deal with sub-block placement
pub struct DirectoryHandler {
	pub data_start: u32,
	pub root_inode_ind: u32,
}

impl DirectoryHandler {
	pub fn new(data_start: u32, root_inode_ind: u32) -> Self {
		Self { data_start, root_inode_ind }
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


	fn _resolve_symlink<D: BlockDevice>(&self, device: &mut D, inode: &INode, parent_inode_ind: u32, inode_handler: &INodeTableHandler, it: u32) -> FSResult<ResolutionResult> {
		let block_ind = self.data_start + inode.direct[0];
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(block_ind, &mut buf)?;
		let target = &buf[0..inode.size as usize];
		// TODO: as always, handle invalid strings
		let target_path = Path::new(std::str::from_utf8(target).unwrap());

		if target_path.is_absolute() {
			return self._resolve(device, &target_path, self.root_inode_ind, inode_handler, true, it + 1);
		} else {
			return self._resolve(device, &target_path, parent_inode_ind, inode_handler, true, it + 1);
		}
	}
	fn _resolve<D: BlockDevice>(&self, device: &mut D, path: &Path, src: u32, inode_handler: &INodeTableHandler, resolve_final_symlink: bool, it: u32) -> FSResult<ResolutionResult> {
		//! returns: inode of the target, inode index of the target, inode index of the parent directory, RELATIVE block index and offset of the entry
		//! if the target is a symlink and resolve_final_symlink is true, the values returned are those of the file pointed at by the symlink
		//! if path is empty ("", ".", "/") returns src 

		let mut cur_inode_ind = src;
		let mut cur_inode = inode_handler.read_inode(device, src)?;
		let mut parent_inode_ind = src;
		let mut entry_offset = 0;
		let mut entry_block = 0;
		let mut entry = DirEntry::free(0);


		if path == Path::new("") || path == Path::new(".") {
			return Ok(ResolutionResult { 
				target_inode: cur_inode, target_inode_index: cur_inode_ind, 
				parent_inode_index: parent_inode_ind, 
				dir_entry: entry, dir_entry_block_index: entry_block, dir_entry_offset: entry_offset
			});
		}
		if path == Path::new("/") {
			let cur_inode_ind = self.root_inode_ind;
			let cur_inode = inode_handler.read_inode(device, self.root_inode_ind)?;
			let parent_inode_ind = self.root_inode_ind;
			return Ok(ResolutionResult { 
				target_inode: cur_inode, target_inode_index: cur_inode_ind, 
				parent_inode_index: parent_inode_ind, 
				dir_entry: entry, dir_entry_block_index: entry_block, dir_entry_offset: entry_offset
			});
		}

		let mut progress = PathBuf::new();
		let components = path.components();
		let final_component = path.file_name().unwrap().to_str().unwrap();

		// for every component of the path
		for comp in components {
			// comp can be CurDir or RootDir only at the start of the path
			if comp == Component::CurDir || comp == Component::RootDir { 
				continue;
			}
			progress = progress.join(comp);

			let comp_name = Self::path_component_bytes(&comp);

			// check if the current directory contains the component
			let entry_opt = self.find_entry(device, &cur_inode, comp_name)?;
			if entry_opt.is_none() {
				return Err(FSError::DoesNotExist{ path: progress.to_string_lossy().into_owned() });
			}

			(entry, entry_block, entry_offset) = entry_opt.unwrap();
			if entry.inode == INVALID_ADDRESS {
				return Err(FSError::InvalidDirEntry(InvalidDirEntryKind::InvalidINode));
			} // TODO: check for address OOB

			let inode = inode_handler.read_inode(device, entry.inode)?;

			// if the inode is a symlink, we have to follow it, otherwise just prepare the next iteration
			let is_final_component = comp.as_os_str().to_str().unwrap() == final_component;
			if inode.file_type == FileType::Symlink && (!is_final_component || resolve_final_symlink) {
				if it >= 40 {
					return Err(FSError::MaximumSymlinkDepthReached);
				} if inode.size == 0 || inode.blocks == 0 {
					return Err(FSError::EmptySymlink{ path: progress.to_string_lossy().into_owned() });
				}
				
				let res = self._resolve_symlink(device, &inode, cur_inode_ind, inode_handler, it)?;
				cur_inode = res.target_inode;
				cur_inode_ind = res.target_inode_index;
				parent_inode_ind = res.parent_inode_index;
				entry_block = res.dir_entry_block_index;
				entry_offset = res.dir_entry_offset;

				if cur_inode.file_type != FileType::Directory && !is_final_component {
					return Err(FSError::NotADirectory{ path: progress.to_string_lossy().into_owned() });
				}

			} else if inode.file_type == FileType::Directory {
				parent_inode_ind = cur_inode_ind;
				cur_inode = inode;
				cur_inode_ind = entry.inode;
			} else if !is_final_component {
				return Err(FSError::NotADirectory{ path: progress.to_string_lossy().into_owned() });
			} else { // final File component (or Symlink if not to resolve)
				parent_inode_ind = cur_inode_ind;
				cur_inode = inode;
				cur_inode_ind = entry.inode;
			}
		}

		Ok(ResolutionResult { 
			target_inode: cur_inode, target_inode_index: cur_inode_ind, 
			parent_inode_index: parent_inode_ind, 
			dir_entry: entry, dir_entry_block_index: entry_block, dir_entry_offset: entry_offset
		})
	}
	pub fn resolve<D: BlockDevice>(&self, device: &mut D, path: &Path, src: u32, inode_handler: &INodeTableHandler, resolve_final_symlink: bool) -> FSResult<ResolutionResult> {
		//! returns: inode of the target, inode index of the target, inode index of the parent directory, block index and offset of the entry
		//! if the target is a symlink and resolve_final_symlink is true, the values returned are those of the file pointed at by the symlink

		self._resolve(device, path, src, inode_handler, resolve_final_symlink, 0)
	}


	//pub fn traverse<D: BlockDevice>(&self, device: &mut D, path: &Path, src: u32, inode_handler: &INodeTableHandler) -> FSResult<u32> {
	//	//! starting from the given src directory, returns the directory "src/path/"
	//	//! throws: DirectoryDoesNotExist or NotADirectory
		
	//	let mut cur_inode_ind = src;
	//	let mut cur_dir = inode_handler.read_inode(device, src)?;
		
	//	// for every component of the path
	//	let mut progress = PathBuf::new();
	//	for comp in path.components() {
	//		// comp can be CurDir or RootDir only at the start of the path
	//		if comp == Component::CurDir || comp == Component::RootDir { 
	//			continue;
	//		}
	//		progress = progress.join(comp);

	//		let comp_name = Self::path_component_bytes(&comp);

	//		// check if the current directory contains the component
	//		let entry = self.find_entry(device, &cur_dir, comp_name)?;
	//		if entry.is_none() {
	//			return Err(FSError::DirectoryDoesNotExist{ path: progress.to_string_lossy().into_owned() });
	//		}
	//		let inode_ind = entry.unwrap().0.inode;
	//		if inode_ind == INVALID_ADDRESS {
	//			return Err(FSError::InvalidDirEntry(InvalidDirEntryKind::InvalidINode));
	//		}

	//		let cur_inode = inode_handler.read_inode(device, inode_ind)?;
			
	//		if cur_inode.file_type == FileType::Symlink {
	//			if cur_inode.size == 0 || cur_inode.blocks == 0 {
	//				return Err(FSError::EmptySymlink{ path: progress.to_string_lossy().into_owned() });
	//			}

	//			let block_ind = self.data_start + cur_inode.direct[0];
	//			let mut buf = [0u8; BLOCK_SIZE];
	//			device.read_block(block_ind, &mut buf)?;
	//			let target = &buf[0..cur_inode.size as usize];
	//			// TODO: as always, handle invalid strings
	//			let target_path = Path::new(std::str::from_utf8(target).unwrap());

	//			if target[0] == b'/' {
	//				cur_inode_ind = self.traverse(device, &target_path, root, inode_handler)?;
	//			} else {
	//				cur_inode_ind = self.traverse(device, &target_path, cur_inode_ind, inode_handler)?;
	//			}
	//			cur_dir = inode_handler.read_inode(device, cur_inode_ind)?;

	//		} else if cur_dir.file_type == FileType::Directory {
	//			cur_dir = cur_inode;
	//			cur_inode_ind = inode_ind;
	//		} else {
	//			return Err(FSError::NotADirectory{ path: progress.to_string_lossy().into_owned() });
	//		}
	//	}

	//	Ok(cur_inode_ind)
	//}


	pub fn can_fit<D: BlockDevice>(&self, device: &mut D, dir: &INode, to_insert: &DirEntry) -> FSResult<Option<(u32, u16)>> {
		//! checks whether the entry can be inserted in the directory without adding another data block.
		//! if it can fit, returns the block index where it fits and the offset of the offset of the free region to use.

		let mut res: FSResult<Option<(u32, u16)>> = Ok(None);
		
		self.iterate(device, dir, |entry: DirEntry, block: u32, offset: usize| -> bool {
			// we return the first free region big enough.
			// deletion behavior guarantees that there will not be two contiguous free regions
			if entry.is_free() && entry.record_len >= to_insert.record_len + DirEntry::min_free_size() {
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
		if free_region.record_len < entry.record_len + DirEntry::min_free_size() {
			return Err(FSError::DirFreeRegionTooSmall);
		} if !free_region.is_free() {
			return Err(FSError::DirEntryNotFree);
		}

		// move it forward (if not fully utilized)
		free_region.record_len -= entry.record_len;
		if free_region.record_len > DirEntry::min_free_size() {
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


	pub fn free_region<D: BlockDevice>(&self, device: &mut D, _dir: &INode, block: u32, offset: u16) -> FSResult<u16> {
		//! marks the specified region as free, potentially merging with adjacent free regions
		//! return: record length of the freed entry
		//! throws: InvalidInput(OffsetNotAtDirEntryStart)
		
		// read the whole block (might contain other entries)
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(self.data_start + block, &mut buf)?;

		let mut free_region_size = 0;
		let mut free_region_start = offset;

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

		// parse the entry to free
		let to_free = DirEntry::deserialize(&buf, offset as usize);
		free_region_size += to_free.record_len;

		// parse the next entry
		let next_offset = offset as usize + to_free.record_len as usize;
		if next_offset < BLOCK_SIZE {
			// TODO: this can fail if the provided offset/size are invalid.
			let next_entry = DirEntry::deserialize(&buf, next_offset);
			if next_entry.is_free() {
				free_region_size += next_entry.record_len;
			}
		}
		
		// create the entry corresponding to the new free region
		let free_entry = DirEntry::free(free_region_size);
		free_entry.serialize(&mut buf, free_region_start as usize);
		
		device.write_block(self.data_start + block, &buf)?;

		Ok(to_free.record_len)
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

