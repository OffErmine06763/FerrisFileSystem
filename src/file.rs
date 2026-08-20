

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum FileType {
	File,
	Directory,
	/// File that contains another path.
	/// If the path is absolute (/path) it is relative to root.
	/// If the path is relative (./path) it is relative to the directory it is in.
	/// Operations such as open/read/write file should target the file pointed to by the symlink.
	/// Operations such as delete/rename should target the symlink itself.
	Symlink,
	Unknown,
}


pub struct File {
	pub id: u32,
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