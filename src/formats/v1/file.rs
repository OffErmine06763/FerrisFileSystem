pub enum OpenFileMode {
	Read, Write, ReadWrite,
}

pub struct FileMetadata {
	pub inode: u32,
	pub offset: u64,
	// do not keep the size here, since multiple handles can write to the same file,
	// so the cached size might be invalid
	pub mode: OpenFileMode,
}

impl FileMetadata {
    pub fn new(inode: u32, mode: OpenFileMode) -> Self {
        Self { inode, offset: 0, mode }
    }
}