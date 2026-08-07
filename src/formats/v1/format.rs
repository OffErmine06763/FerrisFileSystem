use crate::fs_utils::*;
use crate::formats::format::FsFormat;
use crate::device::block_device::BlockDevice;

use super::inode::{INode, FileType};
use super::superblock::Superblock;
use super::inode_handler::INodeTableHandler;
use super::bitmap_allocator::BitmapAllocator;
use super::directory::{Directory, DirEntry};
use super::directory_handler::DirectoryHandler;

use std::io;
use std::path::{self, Path};


pub struct FormatV1 {
	superblock: Superblock,
	inode_allocator: BitmapAllocator,
	block_allocator: BitmapAllocator,
	inode_handler: INodeTableHandler,
	directory_handler: DirectoryHandler,
}



impl<D: BlockDevice> FsFormat<D> for FormatV1 {
	fn create_file(&mut self, device: &mut D, path_str: &str) -> io::Result<()> {
		// TODO: if any operation fails after the allocation, we should dealloc those blocks.

		let path = Path::new(path_str);
		let parent = path.parent().unwrap(); // TODO: handle ill formatted paths
		let filename = path.file_name().unwrap().to_str().unwrap();

		let dir_inode_ind: u32 = self.directory_handler.traverse(device, parent, self.superblock.root_inode)?;
		let mut dir_inode = self.inode_handler.read_inode(device, dir_inode_ind)?;

		let mut entry = DirEntry::new(INVALID_ADDRESS, FileType::File, filename);
		let fit_result = self.directory_handler.can_fit(device, &dir_inode, &mut entry)?;
		

		// allocate blocks
		let inodes_to_alloc = 1;
		let data_to_alloc = if fit_result != None { 1 } else { 2 };
		let (allocated_inodes, allocated_data) = self.allocate(device, inodes_to_alloc, data_to_alloc)?;

		let inode_index = allocated_inodes[0];
		let block_addr  = self.superblock.data_start + allocated_data[0];
		let inode = INode::empty(FileType::File, u16::MAX, u32::MAX as u64);
		entry.inode = inode_index;


		// write inode and file contents
		self.inode_handler.write_inode(device, inode_index, &inode)?;
		let buf = [1u8; BLOCK_SIZE];
		device.write_block(block_addr, &buf)?;


		// write directory entry
		if fit_result == None {
			dir_inode.size += BLOCK_SIZE as u64;
			dir_inode.add_block(allocated_data[1]);
			self.directory_handler.add_entry_grow(device, &dir_inode, &mut entry, allocated_data[1]);
			self.inode_handler.write_inode(device, dir_inode_ind, &dir_inode)?;
		}
		else {
			let (block, entry_to_shrink) = fit_result.unwrap();
			self.directory_handler.add_entry_here(device, &dir_inode, &mut entry, block, entry_to_shrink);
		}


		#[cfg(debug_assertions)]
		{
			println!("Created file '{}'", path);
			println!("data block: {} (0x{:08X})", block_addr, block_addr as u64 * BLOCK_SIZE as u64);
			println!("inode index: {}", inode_index);
			let inode_block = inode_index / INode::inodes_per_block();
			println!("inode block: {} (0x{:08X})", inode_block, inode_block as u64 * BLOCK_SIZE as u64);
			println!();
			// TODO: add more info
		}

		Ok(())
	}
	fn delete_file(&mut self) {
		todo!()
	}

	fn read(&mut self, inode: u32, buf: &mut [u8]) {
		todo!()
	}
	fn write(&mut self) {
		todo!()
	}
}



impl FormatV1 {
	pub const VERSION: Version = Version::V1;
	// recommended: 1 inode per 16 KB
	pub const INODE_DENSITY: u32 = 1 * 1024;

	fn allocate<D: BlockDevice>(&mut self, device: &mut D, inodes: u32, data: u32) -> io::Result<(Vec<u32>, Vec<u32>)> {
		//! returns a vector containing the indices of the allocated blocks, first the inodes, then the data blocks.
		//! succeeds only if all the requested allocations succeed.
		let mut inode_inds: Vec<u32> = self.inode_allocator.find_free(device, inodes)?;
		let mut data_inds: Vec<u32> = self.block_allocator.find_free(device, data)?;

		if inode_inds.len() < inodes as usize {
			return Err(io::Error::new(io::ErrorKind::StorageFull, "not enough free inodes"));
		}
		if data_inds.len() < data as usize {
			return Err(io::Error::new(io::ErrorKind::StorageFull, "not enough free data blocks"));
		}

		self.inode_allocator.allocate(device, &inode_inds)?;
		self.block_allocator.allocate(device, &data_inds)?;

		Ok((inode_inds, data_inds))
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
			directory_handler
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

		let superblock = Superblock::new(MAGIC, 1, BLOCK_SIZE as u32, blocks, inode_bitmap_blocks, block_bitmap_blocks, inode_table_blocks, 0);


		// CREATE ROOT NODE

		let inode_handler = INodeTableHandler::new(superblock.inode_table_start, inode_table_blocks);
		let mut root_inode = INode::empty(FileType::Directory, u16::MAX, u32::MAX as u64);
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

		/*
		let s = "inode bitmap";
		buf[..s.len()].copy_from_slice(s.as_bytes());
		buf[s.len()..].fill(0);
		device.write_block(superblock.inode_bitmap_start, &buf)?;
		let s = "block bitmap";
		buf[..s.len()].copy_from_slice(s.as_bytes());
		buf[s.len()..].fill(0);
		device.write_block(superblock.block_bitmap_start, &buf)?;
		let s = "inode table";
		buf[..s.len()].copy_from_slice(s.as_bytes());
		buf[s.len()..].fill(0);
		device.write_block(superblock.inode_table_start, &buf)?;
		let s = "data blocks";
		buf[..s.len()].copy_from_slice(s.as_bytes());
		buf[s.len()..].fill(0);
		device.write_block(superblock.data_start, &buf)?;
		*/
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