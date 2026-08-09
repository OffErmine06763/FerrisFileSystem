use crate::fs_utils::*;



impl FileType {
	pub fn to_le_bytes(&self) -> [u8; 1] {
		match self {
			FileType::File      => { [0; 1] }
			FileType::Directory => { [1; 1] }
			FileType::Symlink   => { [2; 1] }
		}
	}
	pub fn from_le_bytes(buf: &[u8; 1]) -> Self {
		match buf[0] {
			0 => { FileType::File      }
			1 => { FileType::Directory }
			_ => { FileType::Symlink   }
		}
	}
}

pub struct INode {
	pub size: u64,
	pub file_type: FileType,
	pub permissions: u16,

	pub direct: [u32; 12],
	pub indirect: u32,

	pub created: u64,
	pub modified: u64,

	pub links: u16,
	pub blocks: u16,
}


impl INode {
	pub fn on_disk_size() -> usize {
		const _: () = assert!(std::mem::size_of::<INode>() >= 80, "allocated memory size insufficient for storing the inode");
		return 80;
	}
	pub fn inodes_per_block() -> u32 {
		(BLOCK_SIZE / Self::on_disk_size()) as u32
	}


	pub fn empty(file_type: FileType, permissions: u16, created: u64) -> Self {
		Self { size: 0, file_type, permissions, direct: [INVALID_ADDRESS; 12], indirect: 0, created, modified: created, links: 1, blocks: 0 }
	}


	pub fn add_block(&mut self, block: u32) {
		if self.blocks >= 12 {
			todo!();
		}

		self.direct[self.blocks as usize] = block;
		self.blocks += 1;
	}


	pub fn serialize(&self, buf: &mut [u8; BLOCK_SIZE]) {
		let mut offset = 0;

		macro_rules! write_field {
			($value:expr) => {{
				let bytes = $value.to_le_bytes();
				buf[offset..offset + bytes.len()].copy_from_slice(&bytes);
				offset += bytes.len();
			}};
		}

		write_field!(self.size);
		write_field!(self.file_type);

		let pad = 0u8;
		write_field!(pad);
		write_field!(self.permissions);
		write_field!(self.links);
		write_field!(self.blocks);

		for i in self.direct {
			write_field!(i);
		}
		write_field!(self.indirect);

		write_field!(self.created);
		write_field!(self.modified);
	}

	pub fn deserialize(buf: &[u8]) -> Self {
		let mut offset = 0;

		macro_rules! read_field {
			($ty:ty) => {{
				let size = core::mem::size_of::<$ty>();
				let value = <$ty>::from_le_bytes(
					buf[offset..offset + size]
						.try_into()
						.expect("buffer too small"),
				);
				offset += size;
				value
			}};
		}

		
		let size = read_field!(u64);
		let file_type = read_field!(FileType);
		
		read_field!(u8);

		let permissions = read_field!(u16);
		let links = read_field!(u16);
		let blocks = read_field!(u16);

		let mut direct = [0u32; 12];
		for i in 0..12 {
			direct[i] = read_field!(u32);
		}

		Self {
			size, file_type, permissions, direct,
			indirect: read_field!(u32),
			created: read_field!(u64),
			modified: read_field!(u64),
			links, blocks,
		}
	}


	pub fn print(&self) {
	}
}