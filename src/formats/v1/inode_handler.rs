use crate::fs_utils::*;
use crate::device::block_device::BlockDevice;
use crate::fs_error::*;

use super::inode::*;

use std::io;


/// Manages the contents of the whole table of inodes, since it has to deal with sub-block placement
/// and manages data blocks assigned to inodes, since they require resolution and sub-block placement
pub struct INodeTableHandler {
	pub inode_table_start: u32,
	pub inode_table_blocks: u32,
	pub data_start: u32,
}

impl INodeTableHandler {
	pub fn new(inode_table_start: u32, inode_table_blocks: u32, data_start: u32) -> Self {
		Self { inode_table_start, inode_table_blocks, data_start }
	}

	pub fn write_inode<D: BlockDevice>(&self, device: &mut D, index: u32, inode: &INode) -> FSResult<()> {
		let max_inode_index = self.inode_table_blocks * INode::inodes_per_block();
		if index >= max_inode_index {
			return Err(FSError::InvalidInput(InvalidInputKind::INodeIndexOOB{ index, max: max_inode_index } ));
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

	pub fn read_inode<D: BlockDevice>(&self, device: &mut D, index: u32) -> FSResult<INode> {
		let max_inode_index = self.inode_table_blocks * INode::inodes_per_block();
		if index >= max_inode_index {
			return Err(FSError::InvalidInput(InvalidInputKind::INodeIndexOOB{ index, max: max_inode_index } ));
		}

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



	pub fn blocks_necessary_to_grow_n(&self, inode: &INode, n: u32) -> u32 {
		//! returns the number of new metadata blocks necessary to support an increment of n data block assigned to the inode
		fn metadata_blocks(blocks: usize) -> usize {
			if blocks <= INode::DIRECT_COUNT {
				0
			} else if blocks <= INode::DIRECT_COUNT + INode::INDIRECT_COUNT {
				1
			} else {
				let double_blocks = blocks - INode::DIRECT_COUNT - INode::INDIRECT_COUNT;
				1 + 1 + double_blocks.div_ceil(INode::INDIRECT_COUNT)
			}
		}

		let current = inode.blocks as usize;
		let new = current + n as usize;

		(metadata_blocks(new) - metadata_blocks(current)) as u32
	}
	pub fn blocks_necessary_to_grow(&self, inode: &INode) -> u32 {
		//! returns the number of new metadata blocks necessary to support an increment of 1 data block assigned to the inode
		
		self.blocks_necessary_to_grow_n(inode, 1)
	}

	pub fn add_blocks<D: BlockDevice>(&self, device: &mut D, inode: &mut INode, blocks: &Vec<u32>, pool: &Vec<u32>) -> FSResult<()> {
		//! given the blocks RELATIVE address, adds them to the inode, using the blocks in the pool if necessary.
		//! does NOT perform allocation or checks the allocation of the given block(s).
		//! to know how many new blocks are necessary for adding the data block, use blocks_necessary_to_grow_n
		if blocks.is_empty() {
			return Ok(());
		}

		let current = inode.blocks as usize;
		let new_count = current + blocks.len();
		if new_count > INode::MAX_BLOCKS {
			return Err(FSError::MaxINodeSize);
		}

		// Determine how many metadata blocks are required.
		let required_metadata = self.blocks_necessary_to_grow_n(inode, blocks.len() as u32) as usize;
		if pool.len() < required_metadata {
			return Err(FSError::InvalidInput(InvalidInputKind::NotEnoughAllocatedBlocks));
		}

		let mut pool_index = 0;

		for &block in blocks {
			let index = inode.blocks as usize;

			if index < INode::DIRECT_COUNT {
				inode.direct[index] = block;
			} else if index < INode::DIRECT_COUNT + INode::INDIRECT_COUNT {
				let index = index - INode::DIRECT_COUNT;
				self.write_indirect(device, inode, block, pool, &mut pool_index, index)?;
			} else if index < INode::DIRECT_COUNT + INode::INDIRECT_COUNT + INode::DOUBLE_COUNT {
				let index = index - INode::DIRECT_COUNT - INode::INDIRECT_COUNT;
				self.write_double_indirect(device, inode, block, pool, &mut pool_index, index)?;
			} else {
				return Err(FSError::MaxINodeSize);
			}

			inode.blocks += 1;
		}

		Ok(())
	}

	pub fn add_block<D: BlockDevice>(&self, device: &mut D, inode: &mut INode, block: u32, pool: &Vec<u32>) -> FSResult<()> {
		//! given the block RELATIVE address, adds it to the inode, using the blocks in the pool if necessary.
		//! does NOT perform allocation or checks the allocation of the given block(s).
		//! to know how many new blocks are necessary for adding the data block, use blocks_necessary_to_grow
		let index = inode.blocks as usize;
		if index >= INode::MAX_BLOCKS {
			return Err(FSError::MaxINodeSize);
		}

		if index < INode::DIRECT_COUNT {
			inode.direct[index] = block;
		} else if index < INode::DIRECT_COUNT + INode::INDIRECT_COUNT {
			let index = inode.blocks as usize - INode::DIRECT_COUNT;
			self.write_indirect(device, inode, block, pool, &mut 0, index)?;
		} else if index < INode::DIRECT_COUNT + INode::INDIRECT_COUNT + INode::DOUBLE_COUNT {
			let index = inode.blocks as usize - INode::DIRECT_COUNT - INode::INDIRECT_COUNT;
			self.write_double_indirect(device, inode, block, pool, &mut 0, index)?;
		} else {
			return Err(FSError::MaxINodeSize);
		}

		inode.blocks += 1;
		Ok(())
	}
	fn write_indirect<D: BlockDevice>(&self, device: &mut D, inode: &mut INode, block: u32, pool: &Vec<u32>, pool_index: &mut usize, index: usize) -> FSResult<()> {
		//! puts the given block at the provided index of the indirect region.
		//! NOTE: assumes that index is in the region.
		let mut buf = [0u8; BLOCK_SIZE];

		let indirect;
		if index == 0 {
			// first usage of the indirect block, assign a new metadata block to the inode
			if pool.is_empty() {
				return Err(FSError::InvalidInput(InvalidInputKind::NotEnoughAllocatedBlocks));
			}
			indirect = pool[*pool_index];
			*pool_index += 1;
		} else {
			indirect = inode.indirect;
		}

		device.read_block(indirect + self.data_start, &mut buf)?;
		let offset = index * 4;
		let bytes = block.to_le_bytes();
		buf[offset..offset + bytes.len()].copy_from_slice(&bytes);
		device.write_block(indirect + self.data_start, &buf)?;

		inode.indirect = indirect;
		Ok(())
	}
	fn write_double_indirect<D: BlockDevice>(&self, device: &mut D, inode: &mut INode, block: u32, pool: &Vec<u32>, pool_index: &mut usize,index: usize) -> FSResult<()> {
		//! puts the given block at the provided index of the double indirect region.
		//! NOTE: assumes that index is in the region.
		let mut buf = [0u8; BLOCK_SIZE];
		let indirect_offset = (index / INode::INDIRECT_COUNT) * 4;
		let direct_offset = (index % INode::INDIRECT_COUNT) * 4;

		let double;
		if index == 0 {
			// first usage of the double indirect block, assign a new metadata block to the inode
			if pool.len() < 2 {
				return Err(FSError::InvalidInput(InvalidInputKind::NotEnoughAllocatedBlocks));
			}
			double = pool[*pool_index];
			*pool_index += 1;
		} else {
			double = inode.double;
		}

		device.read_block(double + self.data_start, &mut buf)?;
		
		let indirect;
		if direct_offset == 0 {
			// new indirect within the double indirect
			if pool.is_empty() {
				return Err(FSError::InvalidInput(InvalidInputKind::NotEnoughAllocatedBlocks));
			}
			indirect = pool[*pool_index];
			*pool_index += 1;
		} else {
			indirect = u32::from_le_bytes(buf[indirect_offset..indirect_offset + 4].try_into().expect("buffer too small"));
		}

		device.read_block(indirect + self.data_start, &mut buf)?;
		let bytes = block.to_le_bytes();
		buf[direct_offset..direct_offset + bytes.len()].copy_from_slice(&bytes);
		device.write_block(indirect + self.data_start, &buf)?;

		// update the double indirect after the indirect write succeeded
		if direct_offset == 0 {
			device.read_block(double + self.data_start, &mut buf)?;
			let bytes = indirect.to_le_bytes();
			buf[indirect_offset..indirect_offset + bytes.len()].copy_from_slice(&bytes);
			device.write_block(double + self.data_start, &buf)?;
		}

		inode.double = double;
		Ok(())
	}

	pub fn get_block<D: BlockDevice>(&self, device: &mut D, inode: &INode, index: u32) -> FSResult<u32> {
		//! returns the index-th block RELATIVE address.
		//! throws: InvalidInput(INodeBlockIndexOOB) if index is greater than the maximum number of blocks an inode can store
		//!			InvalidInput(IndexOOB) if index is greater than the number of blocks of the inode

		if index as usize >= INode::MAX_BLOCKS {
			return Err(FSError::InvalidInput(InvalidInputKind::INodeBlockIndexOOB));
		} if index >= inode.blocks {
			return Err(FSError::InvalidInput(InvalidInputKind::IndexOOB));
		}

		let mut index = index as usize;
		if index < INode::DIRECT_COUNT {
			return Ok(inode.direct[index]);
		}
		index -= INode::DIRECT_COUNT;
		if index < INode::INDIRECT_COUNT {
			return self.resolve_indirect(device, inode.indirect, index as u32);
		}
		index -= INode::INDIRECT_COUNT;
		if index < INode::DOUBLE_COUNT {
			// index of the indirect address within the double indirect and
			// index of the direct address within the indirect
			let indirect_index = index as usize / INode::INDIRECT_COUNT;
			let direct_index = index as usize % INode::INDIRECT_COUNT;
			let indirect = self.resolve_double(device, inode.double, indirect_index as u32)?;
			return self.resolve_indirect(device, indirect, direct_index as u32);
		}

		// This shouldn't happen, as it's covered by the early-out at the start of the function
		// anyway it means that the index is greater than the maximum number of blocks.
		Err(FSError::InvalidInput(InvalidInputKind::INodeBlockIndexOOB))
	}


	fn resolve_indirect<D: BlockDevice>(&self, device: &mut D, indirect: u32, index: u32) -> FSResult<u32> {
		//! returns the index-th data block RELATIVE address contained in the provided indirect block (RELATIVE address)
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(self.data_start + indirect, &mut buf)?;

		let start = (index * 4) as usize;
		let end = start + 4;
		let result = u32::from_le_bytes(buf[start..end].try_into().expect("buffer too small"));

		Ok(result)
	}
	fn resolve_double<D: BlockDevice>(&self, device: &mut D, double: u32, index: u32) -> FSResult<u32> {
		//! returns the index-th indirect RELATIVE address contained in the provided double indirect block (RELATIVE address)

		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(self.data_start + double, &mut buf)?;

		let start = (index * 4) as usize;
		let end = start + 4;
		let result = u32::from_le_bytes(buf[start..end].try_into().expect("buffer too small"));

		Ok(result)
	}
}