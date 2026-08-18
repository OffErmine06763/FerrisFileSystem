pub const MAGIC: u64 = 0x2E2E616E616E6162;
pub const BLOCK_SIZE: usize = 256;
pub const INVALID_ADDRESS: u32 = u32::MAX;


#[derive(PartialEq, Clone, Copy, Debug)]
pub enum FileType {
	File,
	Directory,
	Symlink,
	Unknown,
}

impl std::fmt::Display for FileType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::File      => write!(f, "File"),
			Self::Directory => write!(f, "Directory"),
			Self::Symlink   => write!(f, "Symlink"),
			Self::Unknown   => write!(f, "Unknown"),
		}
	}
}

impl FileType {
	pub fn to_le_bytes(&self) -> [u8; 1] {
		match self {
			FileType::File      => { [0; 1] }
			FileType::Directory => { [1; 1] }
			FileType::Symlink   => { [2; 1] }
			FileType::Unknown   => { [3; 1] }
		}
	}
	pub fn from_le_bytes(buf: &[u8; 1]) -> Self {
		match buf[0] {
			0 => { FileType::File      }
			1 => { FileType::Directory }
			2 => { FileType::Symlink   }
			_ => { FileType::Unknown   }
		}
	}
}


pub enum Version {
	V1,
}

pub fn read_version(buf: &[u8; BLOCK_SIZE]) -> Version {
	let version_bytes = &buf[8..12];
	let version = u32::from_le_bytes(version_bytes.try_into().unwrap());

	match version {
		1 => Version::V1,
		_ => panic!("unsupported filesystem version"),
	}
}