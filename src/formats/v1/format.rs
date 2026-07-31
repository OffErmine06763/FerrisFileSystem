use crate::fs_utils::*;
use crate::formats::format::FsFormat;
use crate::formats::v1::inode::INode;
use crate::device::block_device::BlockDevice;

use super::superblock::Superblock;
use super::inode_handler::INodeHandler;
use super::bitmap_allocator::BitmapAllocator;

use std::io;


pub struct FormatV1 {
	superblock: Superblock,
	inode_allocator: BitmapAllocator,
	block_allocator: BitmapAllocator,
	inode_handler: INodeHandler,
}

impl<D: BlockDevice> FsFormat<D> for FormatV1 {
	fn create_file(&mut self, device: &mut D, path: &str) -> io::Result<()> {
		// get the INDEX of the allocated inode, relative to the start of the inode table region, and create the inode
		let inode_index = self.inode_allocator.allocate(device)?;
		self.inode_handler.create_inode(device, inode_index)?;

		let block_addr = self.superblock.data_start + self.block_allocator.allocate(device)?;

		let buf = [1u8; BLOCK_SIZE];
		device.write_block(block_addr, &buf)?;

		#[cfg(debug_assertions)]
		{
			println!("Created file '{}'", path);
			println!("data block: {} (0x{:08X})", block_addr, block_addr as u64 * BLOCK_SIZE as u64);
			println!("inode index: {}", inode_index);
			let inode_block = inode_index / INode::inodes_per_block();
			println!("inode block: {} (0x{:08X})", inode_block, inode_block as u64 * BLOCK_SIZE as u64);
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


	pub fn mount<D: BlockDevice>(device: &mut D) -> io::Result<Self> {
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(0, &mut buf)?;
		let superblock = Superblock::deserialize(&buf);
		if superblock.magic != MAGIC {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic number, there is no valid FS in the device!"));
		}

		// NOTE: the bitmaps might contain more bits than the blocks available
		let inode_allocator = BitmapAllocator::new(superblock.inode_bitmap_start, superblock.inode_bitmap_blocks);
		let block_allocator = BitmapAllocator::new(superblock.block_bitmap_start, superblock.block_bitmap_blocks);
		let inode_handler   = INodeHandler::new(superblock.inode_table_start, superblock.inode_table_blocks);

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
		})
	}


	pub fn format<D: BlockDevice>(device: &mut D) -> io::Result<()> {
		let blocks = device.block_count();
		let bits_per_block = 8 * BLOCK_SIZE as u32;


		let inode_count = (BLOCK_SIZE as u32 * blocks).div_ceil(FormatV1::INODE_DENSITY); // number of inodes
		let inode_table_bytes = inode_count * std::mem::size_of::<INode>() as u32;		  // number of bytes to store the desired number of inodes
		let inode_table_blocks = inode_table_bytes.div_ceil(BLOCK_SIZE as u32);			  // number of blocks to store the desired number of inodes
		
		let inode_bitmap_blocks = inode_table_blocks.div_ceil(bits_per_block);

		let block_bitmap_blocks = blocks.div_ceil(bits_per_block);
		// the above still wastes some bits, those corresponding to the block bitmap blocks itself, who cares.

		let superblock = Superblock::new(MAGIC, 1, BLOCK_SIZE as u32, blocks, inode_bitmap_blocks, block_bitmap_blocks, inode_table_blocks);

		let mut buf = [0u8; BLOCK_SIZE];

		// zero inode bitmap
		let start = superblock.inode_bitmap_start;
		let count = superblock.inode_bitmap_blocks;
		for i in start..(start+count) {
			device.write_block(i, &buf)?;
		}

		// zero data bitmap
		let start = superblock.block_bitmap_start;
		let count = superblock.block_bitmap_blocks;
		for i in start..(start+count) {
			device.write_block(i, &buf)?;
		}

		// zero inode tables
		let start = superblock.inode_table_start;
		let count = superblock.inode_table_blocks;
		for i in start..(start+count) {
			device.write_block(i, &buf)?;
		}

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