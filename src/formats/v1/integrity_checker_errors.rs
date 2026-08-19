use crate::fs_utils::*;
use crate::file::FileType;
use crate::formats::format::{IntegrityResult, IntegrityError};

use std::io;



pub enum UnallocatedDataData {
	INode {
		ind: u32,
	},
	Data { 
		ind: u32,
		inode: u32,
	},
}
pub struct DoubleReferenceData {
	pub block: u32,
}
pub enum InvalidDirectoryEntryData {
	/// unknown file type, sign of corruption.
	FileTypeUnknown {
		dir_inode: u32,
		entry_inode: u32,
		block: u32,
		offset: u16,
	},
	/// the last entry in the block overflows in the next block.
	/// this could be caused by under-filling and parsing a garbage entry at the end of the block
	Overflow {
		dir_inode: u32,
		entry_inode: u32,
		block: u32,
		offset: u16,
	},
	/// name_len greater than the maximum, sign of corruption.
	NameOverflow {
		dir_inode: u32,
		entry_inode: u32,
		block: u32,
		offset: u16,
		name_len: u16
	},
	/// two adjacent free regions are not allowed, even if not problematic.
	AdjacentFree {
		dir_inode: u32,
		block: u32,
		first_offset: u16,
		first_size: u16,
		second_size: u16,
	},
	/// inode pointer is INVALID_ADDRESS
	InvalidAddress,
	/// inode pointer points out of the addressable region
	OutOfBoundsAddress,
}
pub enum InvalidDirectoryStructureData {
	MissingSelf,
	MissingParent,
}
pub enum UnreachableDataData {
	INode {
		ind: u32,
	},
	Data { 
		ind: u32,
	},
}
pub enum InconsistentMetadataData {
	FreeInodes {
		actual: u32,
		expected: u32,
	},
	FreeData {
		actual: u32,
		expected: u32,
	},
}
pub enum InvalidInodeData {
	/// unknown file type, sign of corruption.
	FileTypeUnknown {
		inode: u32,
	},
	/// direct pointer is INVALID_ADDRESS
	InvalidAddress {
		inode: u32,
		direct_ind: u16,
	},
	/// direct pointer points out of the addressable region
	OutOfBoundsAddress {
		inode: u32,
		direct_ind: u16,
	},
}
pub struct MismatchedFileTypeData {
	pub actual: FileType,
	pub expected: FileType,
	pub inode: u32,
}


pub enum V1IntegrityError {
	Ok,
	/// block that is referenced but marked as free
	UnallocatedData(UnallocatedDataData),
	/// data block that is in the direct/indirect of two or more inodes
	DoubleReference(DoubleReferenceData),
	/// structurally wrong directory entry (overflows to the next block, doesn't fill the block...)
	InvalidDirectoryEntry(InvalidDirectoryEntryData),
	/// structurally wrong directory (missing . or .. as first two entries)
	InvalidDirectoryStructure(InvalidDirectoryStructureData),
	/// allocated blocks that cannot be accessed with a filename
	UnreachableData(UnreachableDataData),
	/// stuff like free_inodes in the superblock doesn't match the evidence
	InconsistentMetadata(InconsistentMetadataData),
	/// structurally wrong inode (INVALID_ADDRESS, FileType::Unknown...)
	InvalidInode(InvalidInodeData),
	/// the inode and the dir entry disagree on the file type
	MismatchedFileType(MismatchedFileTypeData),
}

impl V1IntegrityError {
	fn is_ok(&self) -> bool {
		match self {
			V1IntegrityError::Ok => true,
			_ => false,
		}
	}
}

impl IntegrityError for V1IntegrityError {
	fn is_recoverable(&self) -> bool {
		return true;
	}

	fn to_string(&self) -> String {
		todo!()
	}
}




pub struct IntegrityCheckerBitmap {
	pub bitmap: Vec<u8>,
	pub count: u32,
}

impl IntegrityCheckerBitmap {
	pub fn new(bits: u32) -> Self {
		let mut bitmap = Vec::<u8>::new();
		bitmap.resize(bits.div_ceil(8) as usize, 0);
		Self { bitmap, count: 0 }
	}

	pub fn visit(&mut self, bit: u32) -> bool {
		//! returns whether it was already visited or not
		if !self.visited(bit) {
			self.bitmap[(bit / 8) as usize] |= 1 << (bit % 8);
			self.count += 1;
			return false;
		}
		else {
			return true;
		}
	}
	pub fn visited(&self, bit: u32) -> bool {
		self.bitmap[(bit / 8) as usize] & (1 << (bit % 8)) != 0
	}
}