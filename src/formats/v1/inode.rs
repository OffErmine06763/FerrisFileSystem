use crate::fs_utils::*;



pub struct INode {
	pub size: u64,
	pub file_type: FileType,
	pub permissions: u16,

	pub direct: [u32; 12],
	pub indirect: u32,

	pub created: u64,
	pub modified: u64,

	pub links: u16,
	pub blocks: u32,
}


impl INode {
	pub fn on_disk_size() -> usize {
		const _: () = assert!(std::mem::size_of::<INode>() <= 88, "allocated memory size insufficient for storing the inode");
		return 88;
	}
	pub fn inodes_per_block() -> u32 {
		(BLOCK_SIZE / Self::on_disk_size()) as u32
	}


	pub fn empty(file_type: FileType, permissions: u16, created: u64) -> Self {
		Self { size: 0, file_type, permissions, direct: [INVALID_ADDRESS; 12], indirect: INVALID_ADDRESS, created, modified: created, links: 1, blocks: 0 }
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

		let pad = 0u16;
		write_field!(pad);

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

		read_field!(u16);

		let blocks = read_field!(u32);

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
		println!("INode");
		println!("  size:        {} (0x{:016X})", self.size, self.size);
		match self.file_type {
			FileType::Directory => println!("  file type:   Directory"),
			FileType::File      => println!("  file type:   File"),
			FileType::Symlink   => println!("  file type:   Symlink"),
			FileType::Unknown   => println!("  file type:   Unknown  WARNING"),
		}

		println!("  direct (relative):");
		for (i, block) in self.direct.iter().enumerate() {
			if *block == INVALID_ADDRESS {
				print!("       [{:2}] {:>10} (0x{:08X})", i, "INVALID", block);
			} else {
				print!("       [{:2}] {:>10} (0x{:08X})", i, block, block);
			}

			if i % 2 == 0 {
				if i >= self.blocks as usize { print!("  X  "); }
				else						 { print!("     "); }
			} else {
				if i >= self.blocks as usize { println!("  X"); }
				else						 { println!(); }
			}
		}

		if self.indirect == INVALID_ADDRESS {
			print!("  indirect (relative): {:>10} (0x{:08X})", "INVALID", self.indirect);
		} else {
			print!("  indirect (relative): {:>10} (0x{:08X})", self.indirect, self.indirect);
		}
		if self.blocks <= 12 { println!("  X"); }
		else                 { println!(); }
		println!();

		// TODO: print time and permissions nicely
		println!("  created:     {} (0x{:016X})", self.created, self.created);
		println!("  modified:    {} (0x{:016X})", self.modified, self.modified);
		println!("  permissions: {} (0x{:X})", self.permissions, self.permissions);
		println!();
		println!("  links:       {} (0x{:04X})", self.links, self.links);
		println!("  blocks:      {} (0x{:04X})", self.blocks, self.blocks);
		println!();

		let mut buf = [0u8; BLOCK_SIZE];
		self.serialize(&mut buf);
		print!("On disk representation:");
		for i in 0..Self::on_disk_size() {
			if i % 8 == 0 { print!("  "); }
			if i % 16 == 0 { println!(); print!("  "); }
			print!("{:02X}", buf[i]);
		}
		println!();
	}
}
