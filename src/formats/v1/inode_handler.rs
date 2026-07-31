use crate::fs_utils::*;
use super::inode::*;
use crate::device::block_device::BlockDevice;

use std::io;


pub struct INodeHandler {
	pub inode_table_start: u32,
	pub inode_table_blocks: u32,
}

impl INodeHandler {
	pub fn new(inode_table_start: u32, inode_table_blocks: u32) -> Self {
		Self { inode_table_start, inode_table_blocks }
	}

	pub fn create_inode<D: BlockDevice>(&mut self, device: &mut D, index: u32) -> io::Result<()> {
		let max_inode_index = self.inode_table_blocks * INode::inodes_per_block();
		if index >= max_inode_index {
			return Err(io::Error::new(io::ErrorKind::StorageFull, "out of free inodes"));
		}
		let inode_block = self.inode_table_start + index / INode::inodes_per_block();
		let offset = index % INode::inodes_per_block();

		let inode = INode::empty(FileType::File, u16::MAX, u32::MAX as u64);

		let mut buf = [0u8; BLOCK_SIZE];
		inode.serialize(&mut buf);

		let mut block = [0u8; BLOCK_SIZE];
		device.read_block(inode_block, &mut block)?;
		
		let size = std::mem::size_of::<INode>();
		let start = offset as usize * size;
		let end = start as usize + size;
		block[start..end].copy_from_slice(&buf[0..size]);
		device.write_block(inode_block, &block)?;

		Ok(())
	}
}