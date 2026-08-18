use crate::fs_utils::*;
use crate::formats::format::{self, FsFormat, IntegrityResult, IntegrityError};
use crate::formats::file::File;
use crate::device::block_device::BlockDevice;

use super::file::FileMetadata;
use super::inode::INode;
use super::superblock::Superblock;
use super::inode_handler::INodeTableHandler;
use super::bitmap_allocator::BitmapAllocator;
use super::directory::{Directory, DirEntry};
use super::directory_handler::DirectoryHandler;
use super::integrity_checker_errors::*;

use std::io::{self, SeekFrom, Seek, Read, Write};
use std::path::{self, Path};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;


pub struct FormatV1 {
	superblock: Superblock,
	inode_allocator: BitmapAllocator,
	block_allocator: BitmapAllocator,
	inode_handler: INodeTableHandler,
	directory_handler: DirectoryHandler,
	next_file_id: u32,
	open_files: HashMap<u32, FileMetadata>,
}


impl<D: BlockDevice> FsFormat<D> for FormatV1 {
	fn create_file(&mut self, device: &mut D, path_str: &str, file_type: FileType) -> io::Result<File> {
		// TODO: if any operation fails after the allocation, we should dealloc those blocks.

		// TODO: handle name collisions

		if file_type == FileType::Unknown {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid file type provided"))
		}

		// FIND PARENT DIRECTORY AND CREATE NEW ENTRY
		let path = Path::new(path_str);
		let parent = path.parent().unwrap(); // TODO: handle ill formatted paths
		let filename = path.file_name().unwrap().to_str().unwrap();

		let parent_inode_ind: u32 = self.directory_handler.traverse(device, parent, self.superblock.root_inode, &self.inode_handler)?;
		let mut parent_inode = self.inode_handler.read_inode(device, parent_inode_ind)?;

		let mut entry = DirEntry::new(INVALID_ADDRESS, file_type.clone(), filename);
		let fit_result = self.directory_handler.can_fit(device, &parent_inode, &entry)?;
		

		// ALLOCATE BLOCKS
		let inodes_to_alloc = 1;
		let file_blocks = match file_type {
			FileType::Directory => 1,
			FileType::File      => 0,
			FileType::Symlink   => todo!(),
			FileType::Unknown   => panic!(),
		};
		let data_to_alloc = file_blocks + if fit_result != None { 0 } else { 1 };
		let (allocated_inodes, allocated_data) = self.allocate(device, inodes_to_alloc, data_to_alloc)?;

		let inode_index = allocated_inodes[0];
		
		// CREATE INODE
		let now: u64 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let mut inode = INode::empty(file_type, u16::MAX, now);
		entry.inode = inode_index;

		if file_type == FileType::Directory {
			inode.add_block(allocated_data[0]);
		}
		inode.size = match file_type {
			FileType::Directory => BLOCK_SIZE as u64 * inode.blocks as u64,
			FileType::File      => 0,
			FileType::Symlink   => todo!(),
			FileType::Unknown   => panic!(),
		};


		// WRITE INODE AND FILE CONTENTS
		self.inode_handler.write_inode(device, inode_index, &inode)?;
		if file_type == FileType::Directory {
			let new_dir = Directory::new(inode_index, parent_inode_ind);
			self.directory_handler.write_directory_with_inode(device, &new_dir, &inode)?;
		}


		// WRITE DIRECTORY ENTRY
		let entry_block;
		let entry_offset;

		if fit_result == None {
			entry_block = allocated_data[1];
			entry_offset = 0;

			parent_inode.size += BLOCK_SIZE as u64;
			parent_inode.add_block(entry_block);
			self.directory_handler.add_entry_grow(device, &parent_inode, &mut entry, entry_block)?;
		}
		else {
			(entry_block, entry_offset) = fit_result.unwrap();
			self.directory_handler.add_entry_here(device, &parent_inode, &mut entry, entry_block, entry_offset)?;
		}


		// UPDATE PARENT INODE
		parent_inode.modified = now;
		self.inode_handler.write_inode(device, parent_inode_ind, &parent_inode)?;


		#[cfg(debug_assertions)]
		{
			match file_type {
				FileType::Directory => println!("Created directory '{}'", path_str),
				FileType::File      => println!("Created file '{}'", path_str),
				FileType::Symlink   => todo!(),
				FileType::Unknown   => panic!(),
			}
			println!();

			println!("{:<20} {:>12} {:>10} {:>12}", "", "Index/Offset", "Block", "Address");
			println!("{}", "-".repeat(58));

			if file_type == FileType::Directory {
				let block_index = allocated_data[0];
				let data_block = self.superblock.data_start + block_index;
				let data_addr = data_block as u64 * BLOCK_SIZE as u64;
				println!("{:<20} {:>12} {:>10}   0x{:08X}",
					"Directory Data Block", block_index, data_block, data_addr,
				);
			}

			let inode_block = self.superblock.inode_table_start + inode_index / INode::inodes_per_block();
			let inode_addr = inode_block as u64 * BLOCK_SIZE as u64 + inode_index as u64 % INode::inodes_per_block() as u64 * INode::on_disk_size() as u64;
			println!("{:<20} {:>12} {:>10}   0x{:08X}",
				"INode", inode_index, inode_block, inode_addr,
			);

			let dir_inode_block = self.superblock.inode_table_start + parent_inode_ind / INode::inodes_per_block();
			let dir_inode_addr = dir_inode_block as u64 * BLOCK_SIZE as u64 + parent_inode_ind as u64 % INode::inodes_per_block() as u64 * INode::on_disk_size() as u64;
			println!("{:<20} {:>12} {:>10}   0x{:08X}",
				"Parent INode", parent_inode_ind, dir_inode_block, dir_inode_addr,
			);

			let entry_block = self.superblock.data_start + entry_block;
			print!("{:<20} {:>12} {:>10}   0x{:08X}",
				"Parent Entry", entry_offset, entry_block, entry_block as u64 * BLOCK_SIZE as u64 + entry_offset as u64,
			);
			if fit_result == None { println!("New Block"); }
			else                  { println!(); }

			println!();
			inode.print();

			println!();
			entry.print();

			println!();
		}


		// CREATE HANDLE AND METADATA
		let id = self.next_file_id;
		self.next_file_id += 1;

		let metadata = FileMetadata::new(entry.inode);
		self.open_files.insert(id, metadata);

		Ok(File{ id })
	}
	fn delete_file(&mut self, device: &mut D, path_str: &str, file_type: FileType) -> io::Result<()> {
		// TODO: handle failures of intermediate operations.

		if file_type == FileType::Unknown {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "invilid file type provided"))
		}

		// FIND PARENT DIRECTORY
		let path = Path::new(path_str);
		let parent = path.parent().unwrap(); // TODO: handle ill formatted paths
		let filename = path.file_name().unwrap().to_str().unwrap();

		let parent_inode_ind: u32 = self.directory_handler.traverse(device, parent, self.superblock.root_inode, &self.inode_handler)?;
		let mut parent_inode = self.inode_handler.read_inode(device, parent_inode_ind)?;


		// FIND DIRECTORY ENTRY OF THE FILE TO DELETE
		let opt_entry = self.directory_handler.find(device, &parent_inode, filename.as_bytes())?;
		if opt_entry == None {
			return Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
		}

		let (entry, entry_block_ind, entry_offset) = unsafe { opt_entry.unwrap_unchecked() };
		if entry.record_len == 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "directory entry has zero record length"));
		}
		if entry.is_free() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "directory entry to delete is already marked as free"));
		}
		if entry.file_type != file_type {
			match file_type {
				FileType::Directory => return Err(io::Error::new(io::ErrorKind::NotADirectory, "file is not a directory")),
				FileType::File      => return Err(io::Error::new(io::ErrorKind::IsADirectory,  "file is a directory")),
				FileType::Symlink   => todo!(),
				FileType::Unknown   => panic!(),
			}
		}


		// READ THE ENTRY INODE
		let mut file_inode = self.inode_handler.read_inode(device, entry.inode)?;

		if file_type == FileType::Directory {
			// the directory must be empty
			let mut empty = true;

			self.directory_handler.iterate(device, &file_inode, |entry: DirEntry, _block: u32, _offset: usize| -> bool {
				if entry.is_free() { return false; }
				
				if (entry.name_len > 2) || (entry.name[0] != b'.') || (entry.name_len == 2 && entry.name[1] != b'.')
				{
					empty = false;
					return true;
				}

				return false;
			})?;

			if !empty {
				return Err(io::Error::new(io::ErrorKind::DirectoryNotEmpty, "attempted to delete a non-empty directory"));
			}
		}


		// DELETE
		let deleted: bool;
		if file_inode.links > 1 {
			// if the file has multiple links, just decrement the counter
			file_inode.links -= 1;
			self.inode_handler.write_inode(device, entry.inode, &file_inode)?;
			deleted = false;
		}
		else {
			// collect the blocks to dealloc
			let mut data_to_dealloc = Vec::<u32>::new();
			let mut inodes_to_dealloc = Vec::<u32>::new();

			// TODO: support large files
			data_to_dealloc.reserve(file_inode.blocks as usize);
			for block in 0..file_inode.blocks {
				data_to_dealloc.push(file_inode.direct[block as usize]);
			}

			inodes_to_dealloc.push(entry.inode);

			// do dealloc
			self.block_allocator.deallocate(device, &data_to_dealloc)?;
			self.inode_allocator.deallocate(device, &inodes_to_dealloc)?;

			// update superblock metadata
			self.superblock.free_data += data_to_dealloc.len() as u32;
			self.superblock.free_inodes += inodes_to_dealloc.len() as u32;
			let mut buf = [0u8; BLOCK_SIZE];
			self.superblock.serialize(&mut buf);
			device.write_block(0, &buf)?;

			deleted = true;
		}


		// FREE THE DIRECTORY ENTRY
		// do not bother deallocating the block if all free
		self.directory_handler.free_region(device, &parent_inode, entry_block_ind, entry_offset as u16, entry.record_len)?;


		// MODIFY THE PARENT TIMESTAMP
		let now: u64 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		parent_inode.modified = now;
		self.inode_handler.write_inode(device, parent_inode_ind, &parent_inode)?;


		#[cfg(debug_assertions)]
		{
			if deleted {
				match file_inode.file_type {
					FileType::Directory => println!("Deleted directory '{}'", path_str),
					FileType::File      => println!("Deleted file '{}'", path_str),
					FileType::Symlink   => todo!(),
					FileType::Unknown   => panic!(),
				}
			}
			else {
				match file_inode.file_type {
					FileType::Directory => println!("Unlinked directory '{}', links remaining {}", path_str, file_inode.links),
					FileType::File      => println!("Unlinked file '{}', links remaining {}", path_str, file_inode.links),
					FileType::Symlink   => todo!(),
					FileType::Unknown   => panic!(),
				}
			}
			println!();

			if deleted {
				println!("{:<20} {:>12} {:>10} {:>12}", "", "Index/Offset", "Block", "Address");
				println!("{}", "-".repeat(58));
				println!("Data Blocks Deleted:");
				for block in 0..file_inode.blocks {
					let index = file_inode.direct[block as usize];
					let block = self.superblock.data_start + index;
					let addr = block as u64 * BLOCK_SIZE as u64;
					println!("                     {:>12} {:>10}   0x{:08X}", index, block, addr);
				}

				let block = self.superblock.inode_table_start + entry.inode / INode::inodes_per_block();
				let addr = block as u64 * BLOCK_SIZE as u64 + entry.inode as u64 % INode::inodes_per_block() as u64 * INode::on_disk_size() as u64;
				println!("{:<20} {:>12} {:>10}   0x{:08X}",
					"INode Block Deleted:", entry.inode, block, addr,
				);
			}

			{
				println!("Directory Entry Freed:");
				let block = self.superblock.data_start + entry_block_ind;
				let addr = block as u64 * BLOCK_SIZE as u64;
				println!(
					"{:<20} {:>12} {:>10}   0x{:08X}",
					"  Block:", entry_block_ind, block, addr
				);
				println!(
					"{:<20} {:>12} {:>10}   0x{:08X}",
					"  Offset:", entry_offset as u16, '-', addr + entry_offset as u64
				);
				println!(
					"{:<20} {:>12} {:>10}   0x{:08X}",
					"  Size:", entry.record_len, '-', addr + entry_offset as u64 + entry.record_len as u64
				);
			}


			println!();
		}


		Ok(())
	}


	fn file_exists(&mut self, device: &mut D, path_str: &str) -> io::Result<(bool, Option<FileType>)> {
		//! if the file exists, returns true and it's type, otherwise false and None

		// FIND PARENT DIRECTORY
		let path = Path::new(path_str);
		let parent = path.parent().unwrap(); // TODO: handle ill formatted paths
		let filename = path.file_name().unwrap().to_str().unwrap();

		let parent_inode_ind: u32 = self.directory_handler.traverse(device, parent, self.superblock.root_inode, &self.inode_handler)?;
		let parent_inode = self.inode_handler.read_inode(device, parent_inode_ind)?;


		// FIND DIRECTORY ENTRY
		let opt_entry = self.directory_handler.find(device, &parent_inode, filename.as_bytes())?;
		if opt_entry == None {
			return Ok((false, None));
		}

		let (entry, _entry_block_ind, _entry_offset) = unsafe { opt_entry.unwrap_unchecked() };
		if entry.record_len == 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "directory entry has zero record length"));
		} if entry.is_free() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "file exists but directory entry marked as free"));
		} if entry.inode == INVALID_ADDRESS {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "file exists but it has invalid address"));
		} if entry.inode >= self.inode_allocator.max_index() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "file exists but it is not in the addressable area"));
		}

		Ok((true, Some(entry.file_type)))
	}
	fn open_file(&mut self, device: &mut D, path_str: &str) -> io::Result<File> {
		// FIND PARENT DIRECTORY
		let path = Path::new(path_str);
		let parent = path.parent().unwrap(); // TODO: handle ill formatted paths
		let filename = path.file_name().unwrap().to_str().unwrap();

		let parent_inode_ind: u32 = self.directory_handler.traverse(device, parent, self.superblock.root_inode, &self.inode_handler)?;
		let parent_inode = self.inode_handler.read_inode(device, parent_inode_ind)?;


		// FIND DIRECTORY ENTRY
		let opt_entry = self.directory_handler.find(device, &parent_inode, filename.as_bytes())?;
		if opt_entry == None {
			return Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
		}

		let (entry, _entry_block_ind, _entry_offset) = unsafe { opt_entry.unwrap_unchecked() };
		if entry.record_len == 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "directory entry has zero record length"));
		}
		if entry.is_free() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "directory entry to delete is already marked as free"));
		}
		if entry.file_type != FileType::File {
			return Err(io::Error::new(io::ErrorKind::IsADirectory, "cannot open a directory"));
		}

		// CREATE HANDLE AND METADATA
		let id = self.next_file_id;
		self.next_file_id += 1;

		let metadata = FileMetadata::new(entry.inode);
		self.open_files.insert(id, metadata);

		Ok(File{ id })
	}
	fn close_file(&mut self, _device: &mut D, file: &File) -> io::Result<()> {
		self.open_files.remove(&file.id);

		Ok(())
	}


	fn read(&mut self, device: &mut D, file: &File, buf: &mut [u8]) -> io::Result<usize> {
		//! fills the buf with the file contents at the current offset, then increments the offset.
		//! returns the number of bytes actually read.

		if !self.open_files.contains_key(&file.id) {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "file isn't open"));
		}
		if buf.is_empty() {
			return Ok(0);
		}

		let metadata = self.open_files.get_mut(&file.id).unwrap();
		let inode = self.inode_handler.read_inode(device, metadata.inode)?;

		// check that the inode is well formed: the size falls inside the allocated blocks
		if inode.blocks as usize * BLOCK_SIZE < inode.size as usize {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "file size greater than the region allocated for it"));
		}

		let mut read = 0usize;
		let mut block = (metadata.offset / BLOCK_SIZE as u64) as u32;
		let mut in_block_offset = metadata.offset as usize % BLOCK_SIZE;

		let mut device_buf = [0u8; BLOCK_SIZE];

		while read < buf.len() && metadata.offset < inode.size {
			if block >= 12 {
				todo!() // support indirect
			}

			// check that the address is acceptable
			let block_ind = inode.direct[block as usize];
			if block_ind == INVALID_ADDRESS {
				return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid inode direct address"));
			} else if block_ind > self.block_allocator.max_index() {
				return Err(io::Error::new(io::ErrorKind::InvalidData, "inode direct address outside of addressable area"));
			}

			let block_address = self.superblock.data_start + block_ind;
			device.read_block(block_address, &mut device_buf)?;

			// read the minimum between
			//  - the amount to read the remaining part of the block
			//  - the amount to fill the buffer
			//  - the amount to reach EOF
			let to_read = BLOCK_SIZE - in_block_offset;
			let to_read = to_read.min(buf.len() - read);
			let to_read = to_read.min((inode.size - metadata.offset) as usize);

			buf[read..(read + to_read)].copy_from_slice(&device_buf[in_block_offset..(in_block_offset + to_read)]);

			block += 1;
			in_block_offset = 0;
			metadata.offset += to_read as u64;
			read += to_read;
		}

		Ok(read)
	}
	fn write(&mut self, device: &mut D, file: &File, buf: &[u8]) -> io::Result<usize> {
		//! writes the contents of the buf in the file at the current offset, then increments the offset.
		//! returns the number of bytes actually written: usually equal to the buf length, except if the file
		//!         is full and cannot allocate new blocks.

		if !self.open_files.contains_key(&file.id) {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "file isn't open"));
		}
		if buf.is_empty() {
			return Ok(0);
		}

		let metadata = self.open_files.get_mut(&file.id).unwrap();
		let mut inode = self.inode_handler.read_inode(device, metadata.inode)?;

		let mut wrote = 0usize;
		let mut block = (metadata.offset as usize / BLOCK_SIZE) as u32;
		let final_block = ((metadata.offset as usize + buf.len() - 1) / BLOCK_SIZE) as u32;
		let mut in_block_offset = metadata.offset as usize % BLOCK_SIZE;

		let mut device_buf = [0u8; BLOCK_SIZE];


		// if attempting to write past EOF allocate more blocks
		let mut allocated = false;
		if final_block >= inode.blocks {
			allocated = true;

			if final_block >= 12 {
				todo!() // support indirect
			}

			let to_alloc = final_block - inode.blocks + 1;
			
			let data_inds: Vec<u32> = self.block_allocator.find_free(device, to_alloc as u32)?;
			self.block_allocator.allocate(device, &data_inds)?;

			inode.direct[inode.blocks as usize..(inode.blocks as usize + data_inds.len())].copy_from_slice(data_inds.as_slice());
			
			// zero all new blocks, since any write could fail
			for i in &data_inds {
				let block_address = self.superblock.data_start + i;
				device.write_block(block_address, &device_buf)?;
			}

			// this might have allocated less than to_alloc, we still want to write all we can
			inode.blocks += data_inds.len() as u32;
			self.superblock.free_data -= data_inds.len() as u32;
		}


		while wrote < buf.len() && block < inode.blocks {
			if block >= 12 {
				todo!() // support indirect
			}

			// check that the address is acceptable
			let block_ind = inode.direct[block as usize];
			if block_ind == INVALID_ADDRESS {
				return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid inode direct address"));
			} else if block_ind > self.block_allocator.max_index() {
				return Err(io::Error::new(io::ErrorKind::InvalidData, "inode direct address outside of addressable area"));
			}

			let block_address = self.superblock.data_start + block_ind;

			// write the minimum between
			//  - the amount to read the remaining part of the block
			//  - the amount to fill the buffer
			let to_write = BLOCK_SIZE - in_block_offset;
			let to_write = to_write.min(buf.len() - wrote);

			// first and last writes might start/end inside a block, me must read the remaining portion
			if to_write < BLOCK_SIZE {
				device.read_block(block_address, &mut device_buf)?;
			}

			device_buf[in_block_offset..(in_block_offset + to_write)].copy_from_slice(&buf[wrote..(wrote + to_write)]);
			device.write_block(block_address, &device_buf)?;

			block += 1;
			in_block_offset = 0;
			metadata.offset += to_write as u64;
			wrote += to_write;
		}

		// update the inode
		inode.size = inode.size.max(metadata.offset);
		let now: u64 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		inode.modified = now;

		self.inode_handler.write_inode(device, metadata.inode, &inode)?;

		// update the superblock
		if allocated {
			let mut buf = [0u8; BLOCK_SIZE];
			self.superblock.serialize(&mut buf);
			device.write_block(0, &buf)?;
		}

		// update superblock
		

		Ok(wrote)
	}
	fn seek(&mut self, device: &mut D, file: &File, pos: SeekFrom) -> io::Result<u64> {
		//! changes the offset.
		//! note: it can go past the end, in which case the file will grow at the next write or fail a read.

		if !self.open_files.contains_key(&file.id) {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "file isn't open"));
		}

		let metadata = self.open_files.get_mut(&file.id).unwrap();
		match pos {
			SeekFrom::Start(offset) => { metadata.offset = offset; },
			SeekFrom::End(offset) => {
				let inode = self.inode_handler.read_inode(device, metadata.inode)?;
				metadata.offset = (inode.size as i64 + offset).max(0) as u64;
			},
			SeekFrom::Current(offset) => { metadata.offset = (metadata.offset as i64 + offset).max(0) as u64; },
		}

		Ok(metadata.offset)
	}


	fn get_directory_content(&mut self, device: &mut D, path: &str) -> io::Result<format::DirectoryContentResult> {
		let dir = self.get_directory(device, path, false)?;
		let mut entries = Vec::<format::DirectoryContentEntry>::new();
		
		for e in &dir.entries {
			let conversion = String::from_utf8(e.name[0..e.name_len as usize].to_vec());
			match conversion {
				Ok(filename) => entries.push(format::DirectoryContentEntry { filename, file_type: e.file_type }),
				Err(_) => todo!(),
			}
		}

		Ok(format::DirectoryContentResult { entries })
	}


	fn free_space(&mut self, _device: &mut D) -> io::Result<usize> {
		Ok(self.free_data_blocks_count() as usize * BLOCK_SIZE)
	}


	fn check_integrity(&self, device: &mut D) -> io::Result<IntegrityResult> {
		//! checks that:
		//! - all the directory entries are well formatted:
		//! - - starts with . and ..
		//! - - filled entries have minimum size
		//! - - free entries are merged
		//! - - fill the whole block
		//! - - file type matching the inode
		//! - all allocated blocks are actually reachable
		//! - all files correspond to allocated bits
		//! - no two inodes point to the same block
		//! - no invalid addresses

		let mut result = IntegrityResult{ errors: Vec::<Box<dyn IntegrityError>>::new() };


		// count the set bits in the bitmap
		let allocated_inodes = self.inode_allocator.count_allocated(device)?;
		let allocated_blocks = self.block_allocator.count_allocated(device)?;
		let free_inodes = self.superblock.inode_table_blocks * INode::inodes_per_block() - allocated_inodes;
		let free_blocks = self.superblock.total_blocks - self.superblock.data_start - allocated_blocks;


		// validate superblock metadata
		if self.superblock.free_inodes != free_inodes {
			let reason = InconsistentMetadataData::FreeInodes{ actual: self.superblock.free_inodes, expected: free_inodes };
			result.errors.push(Box::new(V1IntegrityError::InconsistentMetadata(reason)));
		}
		if self.superblock.free_data != free_blocks {
			let reason = InconsistentMetadataData::FreeData{ actual: self.superblock.free_data, expected: free_blocks };
			result.errors.push(Box::new(V1IntegrityError::InconsistentMetadata(reason)));
		}


		// get the root inode and prepare to keep track of the reachable blocks
		let mut visited_inodes = IntegrityCheckerBitmap::new(self.inode_allocator.max_index());
		let mut visited_blocks = IntegrityCheckerBitmap::new(self.block_allocator.max_index());	


		// validate root entries recursively, iterating over every file and directory
		let res = self.check_integrity_recursive(device, self.superblock.root_inode, &mut visited_inodes, &mut visited_blocks)?;
		if !res.is_empty() {
			for error in res {
				result.errors.push(Box::new(error));
			}
		}


		// validate bitmaps
		// only allocated bits are marked as visited, so it cannot hold that visited.count > allocated
		// we have to find the allocated bits that are not visited
		if allocated_inodes > visited_inodes.count {
			for i in 0..(self.superblock.inode_table_blocks * INode::inodes_per_block()) {
				// TODO: this is terrible for performance but i'm tired
				if self.inode_allocator.is_allocated(device, i)? && !visited_inodes.visited(i) {
					let reason = UnreachableDataData::INode{ ind: i };
					result.errors.push(Box::new(V1IntegrityError::UnreachableData(reason)));
				}
			}
		}
		if allocated_blocks != visited_blocks.count {
			for i in 0..(self.superblock.total_blocks - self.superblock.data_start) {
				// TODO: this is terrible for performance but i'm tired
				if self.block_allocator.is_allocated(device, i)? && !visited_blocks.visited(i) {
					let reason = UnreachableDataData::Data{ ind: i };
					result.errors.push(Box::new(V1IntegrityError::UnreachableData(reason)));
				}
			}
		}

		return Ok(result);
	}
}



impl FormatV1 {
	pub const VERSION: Version = Version::V1;
	// TODO recommended: 1 inode per 16 KB
	pub const INODE_DENSITY: u32 = 1 * 1024;



	fn allocate<D: BlockDevice>(&mut self, device: &mut D, inodes: u32, data: u32) -> io::Result<(Vec<u32>, Vec<u32>)> {
		//! returns two vector containing the indices of the allocated blocks, first the inodes, then the data blocks.
		//! succeeds only if all the requested allocations succeed.

		if self.superblock.free_inodes < inodes {
			return Err(io::Error::new(io::ErrorKind::StorageFull, "not enough free inodes"));
		}
		if self.superblock.free_data < data {
			return Err(io::Error::new(io::ErrorKind::StorageFull, "not enough free data blocks"));
		}

		let inode_inds: Vec<u32> = self.inode_allocator.find_free(device, inodes)?;
		let data_inds: Vec<u32> = self.block_allocator.find_free(device, data)?;

		if inode_inds.len() < inodes as usize {
			return Err(io::Error::new(io::ErrorKind::StorageFull, "not enough free inodes"));
		}
		if data_inds.len() < data as usize {
			return Err(io::Error::new(io::ErrorKind::StorageFull, "not enough free data blocks"));
		}

		self.inode_allocator.allocate(device, &inode_inds)?;
		self.block_allocator.allocate(device, &data_inds)?; // TODO: if this fails deallocate the inodes.

		// update superblock metadata
		self.superblock.free_data -= data;
		self.superblock.free_inodes -= inodes;
		let mut buf = [0u8; BLOCK_SIZE];
		self.superblock.serialize(&mut buf);
		device.write_block(0, &buf)?;

		Ok((inode_inds, data_inds))
	}



	pub fn free_inodes_count(&self) -> u32 {
		self.superblock.free_inodes
	}
	pub fn free_data_blocks_count(&self) -> u32 {
		self.superblock.free_data
	}
	pub fn used_inodes_count(&self) -> u32 {
		self.superblock.inode_table_blocks * INode::inodes_per_block() - self.superblock.free_inodes
	}
	pub fn used_data_blocks_count(&self) -> u32 {
		self.superblock.total_blocks - self.superblock.data_start - self.superblock.free_data
	}



	pub fn get_directory<D: BlockDevice>(&self, device: &mut D, path_str: &str, include_free: bool) -> io::Result<Directory> {
		let path = Path::new(path_str);

		let dir_inode_ind: u32 = self.directory_handler.traverse(device, path, self.superblock.root_inode, &self.inode_handler)?;
		let dir_inode = self.inode_handler.read_inode(device, dir_inode_ind)?;
		if dir_inode.file_type != FileType::Directory {
			return Err(io::Error::new(io::ErrorKind::NotADirectory, "file is not a directory"));
		}

		let entries = self.directory_handler.get_entries(device, &dir_inode, include_free)?;

		Ok(Directory { inode: dir_inode_ind, entries })
	}



	pub fn mount<D: BlockDevice>(device: &mut D) -> io::Result<Self> {
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(0, &mut buf)?;
		let superblock = Superblock::deserialize(&buf);
		if superblock.magic != MAGIC {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic number, there is no valid FS in the device!"));
		}

		let data_blocks = superblock.total_blocks - superblock.inode_bitmap_blocks - superblock.inode_table_blocks - superblock.block_bitmap_blocks - 1;

		// NOTE: the bitmaps might contain more bits than the blocks available
		let inode_allocator = BitmapAllocator::new(superblock.inode_bitmap_start, superblock.inode_bitmap_blocks, superblock.inode_table_blocks * INode::inodes_per_block());
		let block_allocator = BitmapAllocator::new(superblock.block_bitmap_start, superblock.block_bitmap_blocks, data_blocks);
		let inode_handler   = INodeTableHandler::new(superblock.inode_table_start, superblock.inode_table_blocks);
		let directory_handler = DirectoryHandler::new(superblock.data_start);

		#[cfg(debug_assertions)]
		{
			println!("Mounted device with format V1:");
			println!();
			superblock.print();
			println!();
		}

		Ok(Self {
			superblock,
			inode_allocator,
			block_allocator,
			inode_handler,
			directory_handler,
			next_file_id: 0,
			open_files: HashMap::<u32, FileMetadata>::new(),
		})
	}


	pub fn format<D: BlockDevice>(device: &mut D) -> io::Result<()> {
		// COMPUTE NUMBER OF BLOCKS FOR EACH SECTION

		let blocks = device.block_count();
		let bits_per_block = 8 * BLOCK_SIZE as u32;

		let inode_count = (BLOCK_SIZE as u32 * blocks).div_ceil(FormatV1::INODE_DENSITY); // number of inodes
		let inode_table_blocks = inode_count.div_ceil(INode::inodes_per_block());		  // number of blocks to store the inodes
		
		let inode_bitmap_blocks = inode_table_blocks.div_ceil(bits_per_block);

		let block_bitmap_blocks = blocks.div_ceil(bits_per_block);
		// the above still wastes some bits, those corresponding to the block bitmap blocks itself, who cares.

		let mut superblock = Superblock::new(MAGIC, 1, BLOCK_SIZE as u32, blocks, inode_bitmap_blocks, block_bitmap_blocks, inode_table_blocks, 0);


		// CREATE ROOT NODE

		let inode_handler = INodeTableHandler::new(superblock.inode_table_start, inode_table_blocks);
		let now: u64 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
		let mut root_inode = INode::empty(FileType::Directory, u16::MAX, now);
		let root_dir = Directory::new(0, 0);
		root_inode.add_block(0);
		root_inode.size = BLOCK_SIZE as u64 * root_inode.blocks as u64; // directory entries will expand to fill all the available blocks
		let directory_handler = DirectoryHandler::new(superblock.data_start);
		

		// INITIALIZE DEVICE MEMORY

		let mut buf = [0u8; BLOCK_SIZE];

		// zero inode bitmap
		let start = superblock.inode_bitmap_start + 1;
		let count = superblock.inode_bitmap_blocks;
		for i in start..(start+count) {
			device.write_block(i, &buf)?;
		}

		// zero data bitmap
		let start = superblock.block_bitmap_start + 1;
		let count = superblock.block_bitmap_blocks;
		for i in start..(start+count) {
			device.write_block(i, &buf)?;
		}

		// root directory
		buf[0] = 1;
		device.write_block(superblock.inode_bitmap_start, &buf)?;
		device.write_block(superblock.block_bitmap_start, &buf)?;
		superblock.free_data -= 1;
		superblock.free_inodes -= 1;

		inode_handler.write_inode(device, 0, &root_inode)?;
		directory_handler.write_directory_with_inode(device, &root_dir, &root_inode)?;
		


		// superblock
		superblock.serialize(&mut buf);
		device.write_block(0, &buf)?;


		#[cfg(debug_assertions)]
		{
			println!("Formatted device with format V1:");
			println!();
			superblock.print();
			println!();
		}

		Ok(())
	}



	fn check_integrity_recursive<D: BlockDevice>
		(&self, device: &mut D, dir_inode_ind: u32, visited_inodes: &mut IntegrityCheckerBitmap, visited_blocks: &mut IntegrityCheckerBitmap)
		-> io::Result<Vec<V1IntegrityError>>
	{
		//! checks that the directory in valid, including recursive validity of all the childred
		//! checks inodes/entries/data...
		//! possible errors are: InvalidInode(*), UnallocatedData(*), DoubleReference, InvalidDirectoryEntry(*), MismatchedFileType, InvalidDirectoryStructure(*)


		let mut result = Vec::<V1IntegrityError>::new();

		// check allocation of the inode
		let mut res = self.check_integrity_inode_allocation(device, dir_inode_ind, visited_inodes)?;
		if !res.is_empty() {
			// we can still inspect the contents of this inode, no need to stop, just report the error
			result.append(&mut res);
		}


		let dir_inode = self.inode_handler.read_inode(device, dir_inode_ind)?;
		if dir_inode.file_type == FileType::Unknown {
			let reason = InvalidInodeData::FileTypeUnknown{ inode: dir_inode_ind };
			result.push(V1IntegrityError::InvalidInode(reason));
		}
		else if dir_inode.file_type != FileType::Directory {
			let reason = MismatchedFileTypeData{ actual: dir_inode.file_type, expected: FileType::Directory, inode: dir_inode_ind };
			result.push(V1IntegrityError::MismatchedFileType(reason));
		}

		
		let mut child_dirs = Vec::new();
		let mut child_others = Vec::new();

		
		for i in 0..dir_inode.blocks {
			// TODO: handle indirect

			let block_ind = dir_inode.direct[i as usize];

			if block_ind == INVALID_ADDRESS {
				let reason = InvalidInodeData::InvalidAddress{ inode: dir_inode_ind, direct_ind: i as u16 };
				result.push(V1IntegrityError::InvalidInode(reason));
			} else if block_ind > self.block_allocator.max_index() {
				let reason = InvalidInodeData::OutOfBoundsAddress{ inode: dir_inode_ind, direct_ind: i as u16 };
				result.push(V1IntegrityError::InvalidInode(reason));
			}
			
			// do NOT parse unallocated directory data blocks, they might be garbage
			if !self.block_allocator.is_allocated(device, block_ind)? {
				let reason = UnallocatedDataData::Data{ ind: block_ind, inode: dir_inode_ind };
				result.push(V1IntegrityError::UnallocatedData(reason));
				continue;
			}

			// cannot have two inodes that point to the same data block
			if visited_blocks.visit(block_ind) {
				let reason = DoubleReferenceData{ block: block_ind };
				result.push(V1IntegrityError::DoubleReference(reason));
			}

			let mut res = self.check_integrity_entries_in_block(device, block_ind, i == 0, dir_inode_ind, &mut child_dirs, &mut child_others)?;
			if !res.is_empty() {
				result.append(&mut res);
			}
		}



		for inode_ind in child_others {
			let mut res = self.check_integrity_inode_allocation(device, inode_ind, visited_inodes)?;
			if !res.is_empty() {
				result.append(&mut res);
			}
			let mut res = self.check_integrity_inode(device, inode_ind, visited_blocks, FileType::File)?;
			if !res.is_empty() {
				result.append(&mut res);
			}
		}

		for inode_ind in child_dirs {
			let mut res = self.check_integrity_recursive(device, inode_ind, visited_inodes, visited_blocks)?;
			if !res.is_empty() {
				result.append(&mut res);
			}
		}

		Ok(result)
	}
	fn check_integrity_entries_in_block<D: BlockDevice>
		(&self, device: &mut D, block_ind: u32, first: bool, dir_inode: u32, child_dirs: &mut Vec<u32>, child_others: &mut Vec<u32>)
		-> io::Result<Vec<V1IntegrityError>>
	{
		//! checks that the entries in the given block are well formatted
		//! possible errors are: InvalidDirectoryEntry(*), InvalidDirectoryStructure(*)

		let mut result = Vec::<V1IntegrityError>::new();

		let mut last_entry_size = BLOCK_SIZE;
		let mut last_entry_offset = 0;
		let mut last_entry_free = false;
		let mut count = 0;

		
		// TODO: replace this with the checked version, once it will use custom errors and not string, so that we can 
		//       replace entry.record_len as usize + offset > BLOCK_SIZE with checking the parsing error.
		self.directory_handler.iterate_block_unchecked(device, block_ind, |entry: DirEntry, block: u32, offset: usize| -> bool {
			if entry.file_type == FileType::Unknown {
				let reason = InvalidDirectoryEntryData::FileTypeUnknown{ dir_inode, entry_inode: entry.inode, block, offset: offset as u16 };
				result.push(V1IntegrityError::InvalidDirectoryEntry(reason));
			}

			// validate directory structure (self and parent as first two entries)
			if first {
				if count == 0 {
					// first entry must be self
					if entry.name_len != 1 || entry.name[0] != b'.' {
						let reason = InvalidDirectoryStructureData::MissingSelf;
						result.push(V1IntegrityError::InvalidDirectoryStructure(reason));
					}
				}
				else if count == 1 {
					// second entry must be parent
					if entry.name_len != 2 || entry.name[0] != b'.' || entry.name[1] != b'.' {
						let reason = InvalidDirectoryStructureData::MissingParent;
						result.push(V1IntegrityError::InvalidDirectoryStructure(reason));
					}
				}
			}

			let mut should_stop = false;
			let mut valid_enough = true;

			// check for invalid addresses of the inode pointer
			if !entry.is_free() {
				if entry.inode == INVALID_ADDRESS {
					let reason = InvalidDirectoryEntryData::InvalidAddress;
					result.push(V1IntegrityError::InvalidDirectoryEntry(reason));
					valid_enough = false;
				} else if entry.inode > self.inode_allocator.max_index() {
					let reason = InvalidDirectoryEntryData::OutOfBoundsAddress;
					result.push(V1IntegrityError::InvalidDirectoryEntry(reason));
					valid_enough = false;
				}
			}


			// record overflows to next block
			if entry.record_len as usize + offset > BLOCK_SIZE {
				let reason = InvalidDirectoryEntryData::Overflow{ dir_inode, entry_inode: entry.inode, block, offset: offset as u16 };
				result.push(V1IntegrityError::InvalidDirectoryEntry(reason));
				should_stop = true;
			}


			if !entry.is_free() {
				// record is taking up more or less than the necessary space
				if entry.record_len != DirEntry::min_record_len(entry.name_len) {
					let reason = InvalidDirectoryEntryData::NameOverflow{
						dir_inode, entry_inode: entry.inode, block, offset: offset as u16, name_len: entry.name_len
					};
					result.push(V1IntegrityError::InvalidDirectoryEntry(reason));

					// stop because we cannot know where the next entry actually starts
					should_stop = true;
				}
			}
			else {
				// two consecutive free regions
				if last_entry_free {
					let reason = InvalidDirectoryEntryData::AdjacentFree{ 
						dir_inode, block, first_offset: last_entry_offset as u16, first_size: last_entry_size as u16, second_size: entry.record_len
					};
					result.push(V1IntegrityError::InvalidDirectoryEntry(reason));
				}
			}

			// validate name length
			if entry.name_len as usize > DirEntry::MAX_NAME {
				let reason = InvalidDirectoryEntryData::NameOverflow{
					dir_inode, entry_inode: entry.inode, block, offset: offset as u16, name_len: entry.name_len
				};
				result.push(V1IntegrityError::InvalidDirectoryEntry(reason));
				// no need to stop, since the record_len is correct for this name_len
			}


			// add valid-enough children
			if !entry.is_free() && valid_enough {
				if entry.file_type == FileType::Directory {
					if !(first && (count == 0 || count == 1)) {
						child_dirs.push(entry.inode);
					}
				}
				else {
					child_others.push(entry.inode);
				}
			}

			count += 1;
			last_entry_free = entry.is_free();
			last_entry_offset = offset;
			last_entry_size = entry.record_len as usize;

			should_stop
		})?;

		// we cannot check for under-filled block since the parser will keep iterating until reaching the end of the block

		Ok(result)
	}
	fn check_integrity_inode_allocation<D: BlockDevice>
		(&self, device: &mut D, inode_ind: u32, visited_inodes: &mut IntegrityCheckerBitmap)
		-> io::Result<Vec<V1IntegrityError>> 
	{
		//! checks that the inode is allocated and marks it as visited
		//! possible errors are: UnallocatedData(INode)

		let mut result = Vec::<V1IntegrityError>::new();

		if !self.inode_allocator.is_allocated(device, inode_ind)? {
			let reason = UnallocatedDataData::INode{ ind: inode_ind };
			result.push(V1IntegrityError::UnallocatedData(reason));
		}
		else {
			// mark as visited only valid indices, this makes checking for unreachable blocks much easier
			visited_inodes.visit(inode_ind);
		}

		Ok(result)
	}
	fn check_integrity_inode<D: BlockDevice>
		(&self, device: &mut D, inode_ind: u32, visited_blocks: &mut IntegrityCheckerBitmap, expected_type: FileType)
		-> io::Result<Vec<V1IntegrityError>> 
	{
		//! checks that the content of the inode is valid
		//! possible errors are: MismatchedFileType(*), DoubleReference, InvalidInode(*), UnallocatedData(Data)

		let mut result = Vec::<V1IntegrityError>::new();

		let inode = self.inode_handler.read_inode(device, inode_ind)?;

		if inode.file_type == FileType::Unknown {
			let reason = InvalidInodeData::FileTypeUnknown{ inode: inode_ind };
			result.push(V1IntegrityError::InvalidInode(reason));
		}
		if inode.file_type != expected_type {
			let reason = MismatchedFileTypeData{ actual: inode.file_type, expected: FileType::Directory, inode: inode_ind };
			result.push(V1IntegrityError::MismatchedFileType(reason));
		}

		for i in 0..inode.blocks {
			// TODO: support indirect
			let block_ind = inode.direct[i as usize];

			if block_ind == INVALID_ADDRESS {
				let reason = InvalidInodeData::InvalidAddress{ inode: inode_ind, direct_ind: i as u16 };
				result.push(V1IntegrityError::InvalidInode(reason));
			} else if block_ind > self.block_allocator.max_index() {
				let reason = InvalidInodeData::OutOfBoundsAddress{ inode: inode_ind, direct_ind: i as u16 };
				result.push(V1IntegrityError::InvalidInode(reason));
			}
				
			if !self.block_allocator.is_allocated(device, block_ind)? {
				let reason = UnallocatedDataData::Data{ ind: block_ind, inode: inode_ind };
				result.push(V1IntegrityError::UnallocatedData(reason));
			}
				
			if visited_blocks.visit(block_ind) {
				let reason = DoubleReferenceData{ block: block_ind };
				result.push(V1IntegrityError::DoubleReference(reason));
			}
		}

		Ok(result)
	}
}



fn handle_inode_allocation_error(res: io::Result<u32>) -> io::Result<u32> {
	//! convert the generic allocation error into a specific inode allocation error.
	match res {
		Ok(n) => return Ok(n),
		Err(e) => match e.kind() {
			io::ErrorKind::StorageFull => {
				let msg = e.to_string();
				if msg == BitmapAllocator::ERR_MAPPED_REGION_FULL {
					return Err(io::Error::new(io::ErrorKind::StorageFull, "out of free inodes"));
				} else {
					return Err(io::Error::new(io::ErrorKind::StorageFull, "inodes bitmap full"));
				}
			}
			_ => { return Err(e); }
		}
	}
}
fn handle_data_allocation_error(res: io::Result<u32>) -> io::Result<u32> {
	//! convert the generic allocation error into a specific inode allocation error.
	match res {
		Ok(n) => return Ok(n),
		Err(e) => match e.kind() {
			io::ErrorKind::StorageFull => {
				let msg = e.to_string();
				if msg == BitmapAllocator::ERR_MAPPED_REGION_FULL {
					return Err(io::Error::new(io::ErrorKind::StorageFull, "out of free data block"));
				} else {
					return Err(io::Error::new(io::ErrorKind::StorageFull, "data bitmap full"));
				}
			}
			_ => { return Err(e); }
		}
	}
}




#[cfg(test)]
mod tests {
	use std::{assert_matches, error::Error};
	use super::*;
	use crate::device::memory_device::MemoryDevice;


	

	
	// FS SETUP
	#[test]
	fn formatting_mounting() -> io::Result<()> {
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let ffs = FormatV1::mount(&mut device)?;
		
		// check metadata
		assert_eq!(ffs.superblock.total_blocks, 20);
		assert_eq!(ffs.used_data_blocks_count(), 1);
		assert_eq!(ffs.used_inodes_count(), 1);
		assert_ne!(ffs.superblock.root_inode, INVALID_ADDRESS);
		assert!(ffs.superblock.root_inode < ffs.inode_allocator.max_index());

		// check root inode
		assert!(ffs.inode_allocator.is_allocated(&mut device, ffs.superblock.root_inode)?);
		let root = ffs.inode_handler.read_inode(&mut device, ffs.superblock.root_inode)?;
		assert_eq!(root.file_type, FileType::Directory);
		assert_eq!(root.links, 1);
		assert_eq!(root.size, BLOCK_SIZE as u64);
		assert_eq!(root.blocks, 1);
		assert_ne!(root.direct[0], INVALID_ADDRESS);
		assert!(root.direct[0] < ffs.inode_allocator.max_index());

		// check root entries
		let mut count = 0;
		ffs.directory_handler.iterate(&mut device, &root, |entry, _block, offset| -> bool {
			if count == 0 {
				assert_eq!(entry.name_len, 1);
				assert_eq!(entry.name[0], b'.');
				assert_eq!(entry.record_len, DirEntry::min_record_len(entry.name_len));
				assert_eq!(entry.inode, ffs.superblock.root_inode);
				assert_eq!(entry.file_type, FileType::Directory);
				assert!(!entry.is_free());
			} else if count == 1 {
				assert_eq!(entry.name_len, 2);
				assert_eq!(entry.name[0], b'.');
				assert_eq!(entry.name[1], b'.');
				assert_eq!(entry.record_len, DirEntry::min_record_len(entry.name_len));
				assert_eq!(entry.inode, ffs.superblock.root_inode);
				assert_eq!(entry.file_type, FileType::Directory);
				assert!(!entry.is_free());
			} else if count == 2 {
				assert!(entry.is_free());
				assert_eq!(entry.record_len, (BLOCK_SIZE - offset) as u16);
			} else {
				assert!(false);
			}

			count += 1;
			false
		})?;

		

		let res = ffs.check_integrity(&mut device)?;
		assert!(res.is_ok());

		Ok(())
	}

	#[test]
	fn mounting() -> io::Result<()> {
		// create a state
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let mut ffs = FormatV1::mount(&mut device)?;

		// create a directory and nested file
		let dirname = "dir";
		let filename = "dir/file";
		create_directory(&mut device, &mut ffs, dirname)?;
		create_file(&mut device, &mut ffs, filename)?;


		// re-mount the device (as of now there is no "un-mounting" logic)
		let mut ffs = FormatV1::mount(&mut device)?;

		// check directory and file existence
		let tmp = ffs.file_exists(&mut device, dirname)?;
		assert!(tmp.0);
		assert!(tmp.1.is_some());
		assert_eq!(tmp.1.unwrap(), FileType::Directory);

		let tmp = ffs.file_exists(&mut device, filename)?;
		assert!(tmp.0);
		assert!(tmp.1.is_some());
		assert_eq!(tmp.1.unwrap(), FileType::File);

		// delete the file and directory
		delete_file(&mut device, &mut ffs, filename)?;
		delete_directory(&mut device, &mut ffs, dirname)?;


		assert!(ffs.check_integrity(&mut device)?.is_ok());

		Ok(())
	}

	// BASIC WORKFLOWS
	#[test]
	fn file_creation_deletion() -> io::Result<()> {
		// initialization
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let mut ffs = FormatV1::mount(&mut device)?;

		// create and delete file
		let filename = "file";
		create_file(&mut device, &mut ffs, filename)?;
		delete_file(&mut device, &mut ffs, filename)?;

		assert!(ffs.check_integrity(&mut device)?.is_ok());

		Ok(())
	}

	#[test]
	fn directory_creation_deletion() -> io::Result<()> {
		// initialization
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let mut ffs = FormatV1::mount(&mut device)?;

		// create and delete directory and nested file
		let dirname = "dir";
		let filename = "dir/file";
		create_directory(&mut device, &mut ffs, dirname)?;
		create_file(&mut device, &mut ffs, filename)?;
		delete_file(&mut device, &mut ffs, filename)?;
		delete_directory(&mut device, &mut ffs, dirname)?;


		assert!(ffs.check_integrity(&mut device)?.is_ok());

		Ok(())
	}


	// ERRORS
	#[test]
	fn non_empty_directory_deletion() -> io::Result<()> {
		// initialization
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let mut ffs = FormatV1::mount(&mut device)?;

		// create dir and nested file
		let dirname = "dir";
		let filename = "dir/file";
		create_directory(&mut device, &mut ffs, dirname)?;
		create_file(&mut device, &mut ffs, filename)?;
		
		// delete dir
		let res = ffs.delete_file(&mut device, dirname, FileType::Directory);
		match res {
			Ok(_) => { assert!(false) }
			Err(e) => {
				match e.kind() {
					io::ErrorKind::DirectoryNotEmpty => {},
					_ => assert!(false),
				}
			}
		}

		assert!(ffs.check_integrity(&mut device)?.is_ok());

		Ok(())
	}

	#[test]
	fn inexistent_file_deletion() -> io::Result<()> {
		// initialization
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let mut ffs = FormatV1::mount(&mut device)?;

		// delete file
		let filename = "file";
		let res = ffs.delete_file(&mut device, filename, FileType::File);
		assert_error(res, io::ErrorKind::NotFound);

		// create it
		create_file(&mut device, &mut ffs, filename)?;

		// try to delete it as a directory
		let res = ffs.delete_file(&mut device, filename, FileType::Directory);
		assert_error(res, io::ErrorKind::NotADirectory);

		assert!(ffs.check_integrity(&mut device)?.is_ok());

		Ok(())
	}

	#[test]
	fn inexistent_directory_deletion() -> io::Result<()> {
		// initialization
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let mut ffs = FormatV1::mount(&mut device)?;

		// delete file
		let dirname = "dir";
		let res = ffs.delete_file(&mut device, dirname, FileType::Directory);
		assert_error(res, io::ErrorKind::NotFound);

		// create it
		create_directory(&mut device, &mut ffs, dirname)?;

		// try to delete it as a file
		let res = ffs.delete_file(&mut device, dirname, FileType::File);
		assert_error(res, io::ErrorKind::IsADirectory);

		assert!(ffs.check_integrity(&mut device)?.is_ok());

		Ok(())
	}


	// RANDOM / STRESS
	#[test]
	fn create_delete_sequence() -> io::Result<()> {
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let mut ffs = FormatV1::mount(&mut device)?;

		let initial_inodes = ffs.free_inodes_count();
		let initial_blocks = ffs.free_data_blocks_count();

		// create and delete many files
		for i in 0..1000 {
			create_file(&mut device, &mut ffs, &format!("/file{i}"))?;
			delete_file(&mut device, &mut ffs, &format!("/file{i}"))?;

			assert_eq!(ffs.free_inodes_count(), initial_inodes);
			assert_eq!(ffs.free_data_blocks_count(), initial_blocks);
		}

		// create as many file as possible before deleting them
		for i in 0..initial_inodes {
			create_file(&mut device, &mut ffs, &format!("/file{i}"))?;
		}
		for i in 0..initial_inodes {
			let res = ffs.file_exists(&mut device, &format!("/file{i}"))?;
			assert!(res.0);
			assert!(res.1.is_some());
			assert_eq!(res.1.unwrap(), FileType::File);
		}
		for i in 0..initial_inodes {
			delete_file(&mut device, &mut ffs, &format!("/file{i}"))?;
		}

		assert_eq!(ffs.free_inodes_count(), initial_inodes);
		assert_eq!(ffs.free_data_blocks_count(), initial_blocks);
		
		assert!(ffs.check_integrity(&mut device)?.is_ok());

		Ok(())
	}

	#[test]
	fn fill_inodes() -> io::Result<()> {
		let size = 20;
		let mut device = MemoryDevice::empty(size);
		FormatV1::format(&mut device)?;

		let mut ffs = FormatV1::mount(&mut device)?;

		let initial_inodes = ffs.free_inodes_count();

		for i in 0..initial_inodes {
			create_file(&mut device, &mut ffs, &format!("/file{i}"))?;
		}
		
		let res = ffs.create_file(&mut device, "FULL", FileType::File);
		assert_error_message(res, io::ErrorKind::StorageFull, "not enough free inodes");
		
		assert!(ffs.check_integrity(&mut device)?.is_ok());

		Ok(())
	}



	// test utils



	fn assert_error<T>(actual: io::Result<T>, expected: io::ErrorKind) {
		match actual {
			Ok(_) => { assert!(false) }
			Err(e) => {
				assert_eq!(e.kind(), expected);
			}
		}
	}
	fn assert_error_message<T>(actual: io::Result<T>, expected: io::ErrorKind, message: &str) {
		match actual {
			Ok(_) => { assert!(false) }
			Err(e) => {
				assert_eq!(e.kind(), expected);
				assert_eq!(e.to_string(), message.to_string());
			}
		}
	}

	fn create_file<D: BlockDevice>(device: &mut D, ffs: &mut FormatV1, path_str: &str) -> io::Result<INode> {
		//! creates a valid file

		let file_type = FileType::File;
		let initial_free_data = ffs.superblock.free_data;
		let initial_free_inodes = ffs.superblock.free_inodes;

		let path = Path::new(path_str);
		let parent = path.parent().unwrap();
		let filename = path.file_name().unwrap().to_str().unwrap();

		ffs.create_file(device, path_str, file_type)?;

		// check file existance
		let tmp = ffs.file_exists(device, path_str)?;
		assert!(tmp.0);
		assert!(tmp.1.is_some());
		assert_eq!(tmp.1.unwrap(), file_type);

		// check update of the metadata
		assert_eq!(ffs.superblock.free_data, initial_free_data);
		assert_eq!(ffs.superblock.free_inodes, initial_free_inodes - 1);

		// check the validity of the directory entry
		let parent_inode_ind = ffs.directory_handler.traverse(device, parent, ffs.superblock.root_inode, &ffs.inode_handler)?;
		let parent_inode = ffs.inode_handler.read_inode(device, parent_inode_ind)?;
		let tmp = ffs.directory_handler.find(device, &parent_inode, filename.as_bytes())?;
		assert!(tmp.is_some());

		let (entry, block, offset) = tmp.unwrap();
		assert_eq!(entry.file_type, file_type);
		assert_eq!(entry.name_len, filename.len() as u16);
		assert_eq!(entry.record_len, DirEntry::min_record_len(entry.name_len));
		assert!(entry.inode != INVALID_ADDRESS);
		assert!(entry.inode < ffs.inode_allocator.max_index());
		assert!(ffs.inode_allocator.is_allocated(device, entry.inode)?);

		// check file inode validity
		let inode = ffs.inode_handler.read_inode(device, entry.inode)?;
		assert_eq!(inode.file_type, file_type);
		assert_eq!(inode.links, 1);
		assert_eq!(inode.size, 0);
		assert_eq!(inode.blocks, 0);

		Ok(inode)
	}

	fn create_directory<D: BlockDevice>(device: &mut D, ffs: &mut FormatV1, path_str: &str) -> io::Result<INode> {
		//! creates a valid directory
		
		let file_type = FileType::Directory;
		let initial_free_data = ffs.superblock.free_data;
		let initial_free_inodes = ffs.superblock.free_inodes;

		let path = Path::new(path_str);
		let parent = path.parent().unwrap();
		let filename = path.file_name().unwrap().to_str().unwrap();

		ffs.create_file(device, path_str, file_type)?;

		// check file existance
		let tmp = ffs.file_exists(device, path_str)?;
		assert!(tmp.0);
		assert!(tmp.1.is_some());
		assert_eq!(tmp.1.unwrap(), file_type);

		// check update of the metadata
		assert_eq!(ffs.superblock.free_data, initial_free_data - 1);
		assert_eq!(ffs.superblock.free_inodes, initial_free_inodes - 1);

		// check the validity of the directory entry
		let parent_inode_ind = ffs.directory_handler.traverse(device, parent, ffs.superblock.root_inode, &ffs.inode_handler)?;
		let parent_inode = ffs.inode_handler.read_inode(device, parent_inode_ind)?;
		let tmp = ffs.directory_handler.find(device, &parent_inode, filename.as_bytes())?;
		assert!(tmp.is_some());

		let (entry, block, offset) = tmp.unwrap();
		assert_eq!(entry.file_type, file_type);
		assert_eq!(entry.name_len, filename.len() as u16);
		assert_eq!(entry.record_len, DirEntry::min_record_len(entry.name_len));
		assert!(entry.inode != INVALID_ADDRESS);
		assert!(entry.inode < ffs.inode_allocator.max_index());
		assert!(ffs.inode_allocator.is_allocated(device, entry.inode)?);

		// check directory inode validity
		let dir_inode_ind = entry.inode;
		let inode = ffs.inode_handler.read_inode(device, entry.inode)?;
		assert_eq!(inode.file_type, file_type);
		assert_eq!(inode.links, 1);
		assert_eq!(inode.size, BLOCK_SIZE as u64);
		assert_eq!(inode.blocks, 1);
		assert!(ffs.block_allocator.is_allocated(device, inode.direct[0])?);

		// check directory content
		let mut count = 0;
		ffs.directory_handler.iterate(device, &inode, |entry, block, offset| -> bool {
			if count == 0 {
				assert_eq!(entry.name_len, 1);
				assert_eq!(entry.name[0], b'.');
				assert_eq!(entry.record_len, DirEntry::min_record_len(entry.name_len));
				assert_eq!(entry.inode, dir_inode_ind);
				assert_eq!(entry.file_type, FileType::Directory);
				assert!(!entry.is_free());
			} else if count == 1 {
				assert_eq!(entry.name_len, 2);
				assert_eq!(entry.name[0], b'.');
				assert_eq!(entry.name[1], b'.');
				assert_eq!(entry.record_len, DirEntry::min_record_len(entry.name_len));
				assert_eq!(entry.inode, parent_inode_ind);
				assert_eq!(entry.file_type, FileType::Directory);
				assert!(!entry.is_free());
			} else if count == 2 {
				assert!(entry.is_free());
				assert_eq!(entry.record_len, (BLOCK_SIZE - offset) as u16);
			} else {
				assert!(false);
			}

			count += 1;
			false
		})?;
		
		Ok(inode)
	}


	fn delete_file<D: BlockDevice>(device: &mut D, ffs: &mut FormatV1, path_str: &str) -> io::Result<()> {
		//! deletes a valid file

		let file_type = FileType::File;
		let initial_free_data = ffs.superblock.free_data;
		let initial_free_inodes = ffs.superblock.free_inodes;

		let path = Path::new(path_str);
		let parent = path.parent().unwrap();
		let filename = path.file_name().unwrap().to_str().unwrap();

		// get the parent inode
		let parent_inode_ind = ffs.directory_handler.traverse(device, parent, ffs.superblock.root_inode, &ffs.inode_handler)?;
		let parent_inode = ffs.inode_handler.read_inode(device, parent_inode_ind)?;
		let tmp = ffs.directory_handler.find(device, &parent_inode, filename.as_bytes())?;
		let (entry, block, offset) = tmp.unwrap();

		// get the file inode
		let inode = ffs.inode_handler.read_inode(device, entry.inode)?;

		// delete the file
		ffs.delete_file(device, path_str, file_type)?;

		// check file existance
		let tmp = ffs.file_exists(device, path_str)?;
		assert!(!tmp.0);

		// check update of the metadata
		assert_eq!(ffs.superblock.free_data, initial_free_data + inode.blocks as u32);
		assert_eq!(ffs.superblock.free_inodes, initial_free_inodes + 1);

		// check bitmap
		assert!(!ffs.inode_allocator.is_allocated(device, entry.inode)?);
		for b in 0..inode.blocks {
			assert!(!ffs.block_allocator.is_allocated(device, inode.direct[b as usize])?);
		}
		
		Ok(())
	}

	fn delete_directory<D: BlockDevice>(device: &mut D, ffs: &mut FormatV1, path_str: &str) -> io::Result<()> {
		//! deletes a valid directory

		let file_type = FileType::Directory;
		let initial_free_data = ffs.superblock.free_data;
		let initial_free_inodes = ffs.superblock.free_inodes;

		let path = Path::new(path_str);
		let parent = path.parent().unwrap();
		let filename = path.file_name().unwrap().to_str().unwrap();

		// get the parent inode
		let parent_inode_ind = ffs.directory_handler.traverse(device, parent, ffs.superblock.root_inode, &ffs.inode_handler)?;
		let parent_inode = ffs.inode_handler.read_inode(device, parent_inode_ind)?;
		let tmp = ffs.directory_handler.find(device, &parent_inode, filename.as_bytes())?;
		let (entry, block, offset) = tmp.unwrap();

		// get the file inode
		let inode = ffs.inode_handler.read_inode(device, entry.inode)?;

		// delete the file
		ffs.delete_file(device, path_str, file_type)?;

		// check file existance
		let tmp = ffs.file_exists(device, path_str)?;
		assert!(!tmp.0);

		// check update of the metadata
		assert_eq!(ffs.superblock.free_data, initial_free_data + inode.blocks as u32);
		assert_eq!(ffs.superblock.free_inodes, initial_free_inodes + 1);

		// check bitmap
		assert!(!ffs.inode_allocator.is_allocated(device, entry.inode)?);
		for b in 0..inode.blocks {
			assert!(!ffs.block_allocator.is_allocated(device, inode.direct[b as usize])?);
		}
		
		Ok(())
	}




	impl FormatV1 {
		pub(crate) fn get_inode_handler(&self) -> &INodeTableHandler {
			&self.inode_handler
		}
		pub(crate) fn get_directory_handler(&self) -> &DirectoryHandler {
			&self.directory_handler
		}
		pub(crate) fn get_block_allocator(&self) -> &BitmapAllocator {
			&self.block_allocator
		}
		pub(crate) fn get_inode_allocator(&self) -> &BitmapAllocator {
			&self.inode_allocator
		}
	}
}
