use super::super::file::File;

use super::format::FormatV1;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::cell::RefCell;
use std::rc::Rc;


pub struct FileMetadata {
	pub inode: u32,
	pub offset: u64,
	// do not keep the size here, since multiple handles can write to the same file,
	// so the cached size might be invalid
}

impl FileMetadata {
    pub fn new(inode: u32) -> Self {
        Self { inode, offset: 0 }
    }
}