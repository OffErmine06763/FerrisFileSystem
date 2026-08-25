use crate::fs_utils::*;
use super::inode::INode;

pub struct Superblock {
	pub magic: u64,
	pub version: u32,

	pub block_size: u32,
	pub total_blocks: u32,

	pub inode_bitmap_start: u32,
	pub inode_bitmap_blocks: u32,

	pub block_bitmap_start: u32,
	pub block_bitmap_blocks: u32,

	pub inode_table_start: u32,
	pub inode_table_blocks: u32,

	pub data_start: u32,

	pub root_inode: u32,

	pub free_inodes: u32,
	pub free_data: u32,
}

impl Superblock {
	pub fn new(magic: u64, version: u32, block_size: u32, total_blocks: u32, inode_bitmap_blocks: u32, block_bitmap_blocks: u32, inode_table_blocks: u32, root_inode: u32) -> Self {
		let inode_bitmap_start = 1; // superblock
		let block_bitmap_start = inode_bitmap_start + inode_bitmap_blocks;
		let  inode_table_start = block_bitmap_start + block_bitmap_blocks;
		let         data_start =  inode_table_start + inode_table_blocks;

		Self { 
			magic, version, block_size, total_blocks, 
			inode_bitmap_start, inode_bitmap_blocks, 
			block_bitmap_start, block_bitmap_blocks, 
			inode_table_start, inode_table_blocks, 
			data_start, 
			root_inode,
			free_inodes: inode_table_blocks * INode::inodes_per_block(),
			free_data: total_blocks - data_start
		}
	}

	pub fn serialize(&self, buf: &mut [u8; BLOCK_SIZE]) {
		let mut _offset = 0;

		macro_rules! write_field {
			($value:expr) => {{
				let bytes = $value.to_le_bytes();
				buf[_offset.._offset + bytes.len()].copy_from_slice(&bytes);
				_offset += bytes.len();
			}};
		}

		write_field!(self.magic);
		write_field!(self.version);
		write_field!(self.block_size);
		write_field!(self.total_blocks);

		write_field!(self.inode_bitmap_start);
		write_field!(self.inode_bitmap_blocks);

		write_field!(self.block_bitmap_start);
		write_field!(self.block_bitmap_blocks);

		write_field!(self.inode_table_start);
		write_field!(self.inode_table_blocks);

		write_field!(self.data_start);

		write_field!(self.root_inode);

		write_field!(self.free_inodes);
		write_field!(self.free_data);
	}

	pub fn deserialize(buf: &[u8]) -> Self {
		let mut _offset = 0;

		macro_rules! read_field {
			($ty:ty) => {{
				let size = core::mem::size_of::<$ty>();
				let value = <$ty>::from_le_bytes(
					buf[_offset.._offset + size]
						.try_into()
						.expect("buffer too small"),
				);
				_offset += size;
				value
			}};
		}

		Self {
			magic: read_field!(u64),
			version: read_field!(u32),

			block_size: read_field!(u32),
			total_blocks: read_field!(u32),

			inode_bitmap_start: read_field!(u32),
			inode_bitmap_blocks: read_field!(u32),

			block_bitmap_start: read_field!(u32),
			block_bitmap_blocks: read_field!(u32),

			inode_table_start: read_field!(u32),
			inode_table_blocks: read_field!(u32),

			data_start: read_field!(u32),
			
			root_inode: read_field!(u32),

			free_inodes: read_field!(u32),
			free_data: read_field!(u32),
		}
	}


	pub fn print(&self) {
		println!("Superblock");
		println!("==========");
		println!("{:<15} {:>12} {:>12}", "Region", "Start", "Blocks");
		println!("{:<15}-{:>12}-{:>12}", "---------------", "------------", "------------");

		println!(
			"{:<15} {:>12} {:>12}",
			"inode bitmap", self.inode_bitmap_start, self.inode_bitmap_blocks
		);
		println!(
			"{:<15} {:>12} {:>12}",
			"block bitmap", self.block_bitmap_start, self.block_bitmap_blocks
		);
		println!(
			"{:<15} {:>12} {:>12}",
			"inode table", self.inode_table_start, self.inode_table_blocks
		);
		println!(
			"{:<15} {:>12}",
			"data start", self.data_start
		);

		println!();
		println!("Metadata");
		println!("========");
		println!("magic:        0x{:016X}", self.magic);
		println!("version:      {}", self.version);
		println!("block size:   {}", self.block_size);
		println!("total blocks: {}", self.total_blocks);
		println!("root inode:   {}", self.root_inode);


		println!();
		println!("Info");
		println!("====");
		println!("number of inodes:      {} ({} per block)", self.inode_table_blocks * INode::inodes_per_block(), INode::inodes_per_block());
		let data_blocks = self.total_blocks - self.inode_bitmap_blocks - self.inode_table_blocks - self.block_bitmap_blocks - 1;
		println!("number of data blocks: {}", data_blocks);

	}
}