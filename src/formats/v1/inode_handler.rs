use crate::fs_utils::*;
use super::inode::*;
use crate::device::block_device::BlockDevice;

use std::io;


/// Manages the contents of the whole table of inodes, since it has to deal with sub-block placement
pub struct INodeTableHandler {
	pub inode_table_start: u32,
	pub inode_table_blocks: u32,
}

impl INodeTableHandler {
	pub fn new(inode_table_start: u32, inode_table_blocks: u32) -> Self {
		Self { inode_table_start, inode_table_blocks }
	}

	pub fn write_inode<D: BlockDevice>(&self, device: &mut D, index: u32, inode: &INode) -> io::Result<()> {
		let max_inode_index = self.inode_table_blocks * INode::inodes_per_block();
		if index >= max_inode_index {
			return Err(io::Error::new(io::ErrorKind::InvalidInput, "inode index past inode table region"));
		}

		let inode_block = self.inode_table_start + index / INode::inodes_per_block();
		let offset = index % INode::inodes_per_block();

		let mut buf = [0u8; BLOCK_SIZE];
		inode.serialize(&mut buf);

		let mut block = [0u8; BLOCK_SIZE];
		device.read_block(inode_block, &mut block)?;
		
		let size = INode::on_disk_size();
		let start = offset as usize * size;
		let end = start as usize + size;
		block[start..end].copy_from_slice(&buf[0..size]);
		device.write_block(inode_block, &block)?;

		Ok(())
	}

	pub fn read_inode<D: BlockDevice>(&self, device: &mut D, index: u32) -> io::Result<INode> {
		let inode_block = self.inode_table_start + index / INode::inodes_per_block();
		let offset = index % INode::inodes_per_block();
		
		let mut block = [0u8; BLOCK_SIZE];
		device.read_block(inode_block, &mut block)?;
		
		let size = INode::on_disk_size();
		let start = offset as usize * size;
		let end = start as usize + size;
		
		let mut buf = [0u8; BLOCK_SIZE];
		buf[0..size].copy_from_slice(&block[start..end]);
		let inode = INode::deserialize(&buf);
		
		Ok(inode)
	}
}