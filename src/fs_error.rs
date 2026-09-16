use crate::fs_utils::BLOCK_SIZE;
use std::fmt;


#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum FSErrorCategory {
	/// Generic reasons for which the operation failed: 
	/// - invalid inputs / data (without signaling corruption)
	/// - device is full
	/// Those are acceptable events, caused by improper use of the functions or physical system limitations
	Generic    = 0,
	/// The operation failed for a signal of corruption in the device.
	/// This is an event that shouldn't happen, unless there is a bug or the device contents where modified externally
	Corruption = 1,
}


#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u16)]
pub enum FSErrorCode {
	FileDoesNotExist	  = Self::generic(1),
	DirectoryDoesNotExist = Self::generic(2),
	DoesNotExist		  = Self::generic(3),
	AlreadyExists		  = Self::generic(4),

	IsADirectory		  = Self::generic(5),
	NotADirectory		  = Self::generic(6),
	IsAFile				  = Self::generic(7),
	NotAFile			  = Self::generic(8),
	IsASymlink			  = Self::generic(9),
	NotASymlink			  = Self::generic(10),

	DirectoryNotEmpty = Self::generic(11),
	FileNotOpen       = Self::generic(12),

	DirFreeRegionTooSmall = Self::generic(13),
	DirEntryNotFree		  = Self::generic(14),

	StorageFull	   = Self::generic(15),
	INodeTableFull = Self::generic(16),
	DataRegionFull = Self::generic(17),
	
	MaxINodeSize = Self::generic(18),
	
	InvalidInput				  = Self::generic(19),
	InputDirEntriesOverfillBlock  = Self::generic(20),
	InputDirEntriesUnderfillBlock = Self::generic(21),
	InputOffsetNotAtDirEntryStart = Self::generic(22),
	InputINodeIndexOOB			  = Self::generic(23),
	InputBlockIndexOOB			  = Self::generic(24),
	InputIndexOOB				  = Self::generic(25),
	InputINodeBlockIndexOOB		  = Self::generic(26),
	InputUnknownFileType		  = Self::generic(27),
	InputNotEnoughAllocatedBlocks = Self::generic(28),

	EmptySymlink               = Self::generic(29),
	MaximumSymlinkDepthReached = Self::generic(30),
	
	DeletingRoot = Self::generic(31),

	IO = Self::generic(32),

	InvalidDirEntry         = Self::corruption(1),
	ZeroLengthDirEntry      = Self::corruption(2),
	DirEntryIsFree		    = Self::corruption(3),
	DirEntryInvalidINode    = Self::corruption(4),
	DirEntryINodeOOB	    = Self::corruption(5),
	DirEntryInvalidFileType = Self::corruption(6),

	InvalidDir				= Self::corruption(7),
	DirEntriesOverfillBlock	= Self::corruption(8),

	InvalidINode						= Self::corruption(9),
	INodeSizeGreaterThanAllocatedRegion = Self::corruption(10),
	INodeInvalidDirect					= Self::corruption(11),
	INodeDirectOOB						= Self::corruption(12),

	InvalidMagic = Self::corruption(13),
}

impl FSErrorCode {
	const CATEGORY_SHIFT: u16 = 8;
	const ERROR_MASK: u16 = (1 << Self::CATEGORY_SHIFT) - 1;
	const CATEGORY_MASK: u16 = !Self::ERROR_MASK;

	const fn generic(error: u16) -> u16 {
		Self::error_code(FSErrorCategory::Generic as u16, error)
	}
	const fn corruption(error: u16) -> u16 {
		Self::error_code(FSErrorCategory::Corruption as u16, error)
	}
	const fn error_code(category: u16, error: u16) -> u16 {
		(category << Self::CATEGORY_SHIFT) | error
	}

	pub fn category(&self) -> FSErrorCategory {
		let value = ((*self as u16) & Self::CATEGORY_MASK) >> Self::CATEGORY_SHIFT;
		unsafe { std::mem::transmute(value) }
	}
}


#[derive(Debug, PartialEq)]
pub enum StorageFullKind {
	None,
	INodeTableFull,
	DataRegionFull,
}
#[derive(Debug, PartialEq)]
pub enum InvalidDirEntryKind {
	None,
	ZeroLength, // "directory entry has zero record length"
	IsFree, // "directory entry to delete is already marked as free"
	InvalidINode,
	INodeOOB,
	InvalidFileType,
}
#[derive(Debug, PartialEq)]
pub enum InvalidDirKind {
	None,
	EntriesOverfillBlock, // "records sizes exceed block size"
}
#[derive(Debug, PartialEq)]
pub enum InvalidINodeKind {
	None,
	SizeGreaterThanAllocatedRegion, // "file size greater than the region allocated for it"
	InvalidDirect {
		ind: u16,
		address: u32,
	}, // "invalid inode direct address"
	DirectOOB {
		ind: u16,
		address: u32,
		max: u32,
	}, // "inode direct address outside of addressable area"
}
#[derive(Debug, PartialEq)]
pub enum InvalidInputKind {
	None,

	DirEntriesOverfillBlock { block: u32, offset: usize, record_len: usize }, // "records sizes exceed block size"
	DirEntriesUnderfillBlock { block: u32, filled: usize }, // "records do not fill the last data block"
	OffsetNotAtDirEntryStart { block: u32, offset: u16 }, // "the provided offset must mark the start of a new entry"

	INodeIndexOOB {
		index: u32,
		max: u32,
	}, // "inode index past inode table region"
	BlockIndexOOB { index: u32, max: u32 }, // "block out of range"
	IndexOOB { index: u32, max: u32 }, // "index out of range"
	INodeBlockIndexOOB { index: u32, max: u32 }, // "index in the array of blocks assigned to an inode OOB"

	UnknownFileType, // "invalid file type provided"

	NotEnoughAllocatedBlocks { required: u32, provided: u32 },
}

#[derive(Debug)]
pub enum FSError {
	FileDoesNotExist {
		path: String,
	},
	DirectoryDoesNotExist {
		path: String,
	}, // "directory doesn't exist"
	DoesNotExist {
		path: String,
	},
	AlreadyExists {
		path: String,
	},
	
	IsADirectory {
		path: String,
	},
	NotADirectory {
		path: String,
	}, // "path component is not a directory"
	IsAFile {
		path: String,
	},
	NotAFile {
		path: String,
	},
	IsASymlink {
		path: String,
	},
	NotASymlink {
		path: String,
	},

	DirectoryNotEmpty { path: String },
	FileNotOpen {
		file_id: u32
	},

	DirFreeRegionTooSmall { block: u32, offset: u16, available: u16, required: u16 },
	DirEntryNotFree { block: u32, offset: u16 },

	StorageFull { kind: StorageFullKind, requested: u32, available: u32 },
	
	MaxINodeSize { requested_blocks: u32, max_blocks: u32 },

	EmptySymlink {
		path: String,	
	},
	MaximumSymlinkDepthReached { path: String, max_depth: u32 },

	DeletingRoot { path: String },
	
	InvalidDirEntry { kind: InvalidDirEntryKind, block: u32, offset: usize },
	InvalidDir { kind: InvalidDirKind, block: u32, offset: usize },

	InvalidINode { kind: InvalidINodeKind, size: u64, blocks: u32 },
	
	/// class of errors for when the parameters of a function are ill formed.
	/// trying to read a closed file is not invalid input of the filename.
	/// trying to index an inode out of bounds is, it is not well formed
	InvalidInput(InvalidInputKind),
	
	InvalidMagic { expected: u64, actual: u64 }, // "invalid magic number, there is no valid FS in the device!"

	IO(std::io::Error),
}

impl FSError {
	pub fn code(&self) -> FSErrorCode {
		match self {
			Self::FileDoesNotExist      { .. } => FSErrorCode::FileDoesNotExist,
			Self::DirectoryDoesNotExist { .. } => FSErrorCode::DirectoryDoesNotExist,
			Self::DoesNotExist			{ .. } => FSErrorCode::DoesNotExist,
			Self::AlreadyExists			{ .. } => FSErrorCode::AlreadyExists,
			
			Self::IsADirectory  { .. } => FSErrorCode::IsADirectory,
			Self::NotADirectory { .. } => FSErrorCode::NotADirectory,
			Self::IsAFile       { .. } => FSErrorCode::IsAFile,
			Self::NotAFile      { .. } => FSErrorCode::NotAFile,
			Self::IsASymlink    { .. } => FSErrorCode::IsASymlink,
			Self::NotASymlink   { .. } => FSErrorCode::NotASymlink,

			Self::DirectoryNotEmpty { .. } => FSErrorCode::DirectoryNotEmpty,
			Self::FileNotOpen		{ .. } => FSErrorCode::FileNotOpen,
			
			Self::DirFreeRegionTooSmall { .. } => FSErrorCode::DirFreeRegionTooSmall,
			Self::DirEntryNotFree { .. }       => FSErrorCode::DirEntryNotFree,

			Self::StorageFull { kind: StorageFullKind::None, .. }           => FSErrorCode::StorageFull,
			Self::StorageFull { kind: StorageFullKind::INodeTableFull, .. } => FSErrorCode::INodeTableFull,
			Self::StorageFull { kind: StorageFullKind::DataRegionFull, .. } => FSErrorCode::DataRegionFull,
			
			Self::MaxINodeSize { .. } => FSErrorCode::MaxINodeSize,
			
			Self::EmptySymlink { .. } => FSErrorCode::EmptySymlink,
			Self::MaximumSymlinkDepthReached { .. } => FSErrorCode::MaximumSymlinkDepthReached,
			
			Self::DeletingRoot { .. } => FSErrorCode::DeletingRoot,

			Self::InvalidDirEntry { kind: InvalidDirEntryKind::None, .. }            => FSErrorCode::InvalidDirEntry,
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::ZeroLength, .. }      => FSErrorCode::ZeroLengthDirEntry,
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::IsFree, .. }          => FSErrorCode::DirEntryIsFree,
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::InvalidINode, .. }    => FSErrorCode::DirEntryInvalidINode,
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::INodeOOB, .. }        => FSErrorCode::DirEntryINodeOOB,
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::InvalidFileType, .. } => FSErrorCode::DirEntryInvalidFileType,
			
			Self::InvalidDir { kind: InvalidDirKind::None, .. }                 => FSErrorCode::InvalidDir,
			Self::InvalidDir { kind: InvalidDirKind::EntriesOverfillBlock, .. } => FSErrorCode::DirEntriesOverfillBlock,

			Self::InvalidINode { kind: InvalidINodeKind::None, .. }							 => FSErrorCode::InvalidINode,
			Self::InvalidINode { kind: InvalidINodeKind::SizeGreaterThanAllocatedRegion, .. } => FSErrorCode::INodeSizeGreaterThanAllocatedRegion,
			Self::InvalidINode { kind: InvalidINodeKind::InvalidDirect { .. }, .. }			 => FSErrorCode::INodeInvalidDirect,
			Self::InvalidINode { kind: InvalidINodeKind::DirectOOB { .. }, .. }				 => FSErrorCode::INodeDirectOOB,

			Self::InvalidInput(InvalidInputKind::None)                     => FSErrorCode::InvalidInput,
			Self::InvalidInput(InvalidInputKind::DirEntriesOverfillBlock { .. })  => FSErrorCode::InputDirEntriesOverfillBlock,
			Self::InvalidInput(InvalidInputKind::DirEntriesUnderfillBlock { .. }) => FSErrorCode::InputDirEntriesUnderfillBlock,
			Self::InvalidInput(InvalidInputKind::OffsetNotAtDirEntryStart { .. }) => FSErrorCode::InputOffsetNotAtDirEntryStart,
			Self::InvalidInput(InvalidInputKind::INodeIndexOOB { .. })	   => FSErrorCode::InputINodeIndexOOB,
			Self::InvalidInput(InvalidInputKind::BlockIndexOOB { .. })	   => FSErrorCode::InputBlockIndexOOB,
			Self::InvalidInput(InvalidInputKind::IndexOOB { .. })	       => FSErrorCode::InputIndexOOB,
			Self::InvalidInput(InvalidInputKind::INodeBlockIndexOOB { .. })=> FSErrorCode::InputINodeBlockIndexOOB,
			Self::InvalidInput(InvalidInputKind::UnknownFileType)		   => FSErrorCode::InputUnknownFileType,
			Self::InvalidInput(InvalidInputKind::NotEnoughAllocatedBlocks { .. }) => FSErrorCode::InputNotEnoughAllocatedBlocks,

			Self::InvalidMagic { .. } => FSErrorCode::InvalidMagic,

			Self::IO { .. } => FSErrorCode::IO,
		}
	}
}

impl From<std::io::Error> for FSError {
	fn from(err: std::io::Error) -> Self {
		FSError::IO(err)
	}
}

impl fmt::Display for FSError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::FileDoesNotExist { path } => write!(f, "file does not exist: {path}"),
			Self::DirectoryDoesNotExist { path } => write!(f, "directory does not exist: {path}"),
			Self::DoesNotExist { path } => write!(f, "path does not exist: {path}"),
			Self::AlreadyExists { path } => write!(f, "path already exists: {path}"),

			Self::IsADirectory { path } => write!(f, "path is a directory: {path}"),
			Self::NotADirectory { path } => write!(f, "path is not a directory: {path}"),
			Self::IsAFile { path } => write!(f, "path is a file: {path}"),
			Self::NotAFile { path } => write!(f, "path is not a file: {path}"),
			Self::IsASymlink { path } => write!(f, "path is a symbolic link: {path}"),
			Self::NotASymlink { path } => write!(f, "path is not a symbolic link: {path}"),

			Self::DirectoryNotEmpty { path } => write!(f, "directory is not empty: {path}"),
			Self::FileNotOpen { file_id } => write!(f, "file is not open: {file_id}"),
			Self::DirFreeRegionTooSmall { block, offset, available, required } => write!(f, "directory free region at block {block}, offset {offset} is too small: {available} bytes available, {required} required"),
			Self::DirEntryNotFree { block, offset } => write!(f, "directory entry at block {block}, offset {offset} is not free"),

			Self::StorageFull { kind: StorageFullKind::None, requested, available } => write!(f, "storage is full: requested {requested} blocks, {available} available"),
			Self::StorageFull { kind: StorageFullKind::INodeTableFull, requested, available } => write!(f, "inode table is full: requested {requested} inodes, {available} available"),
			Self::StorageFull { kind: StorageFullKind::DataRegionFull, requested, available } => write!(f, "data region is full: requested {requested} blocks, {available} available"),
			Self::MaxINodeSize { requested_blocks, max_blocks } => write!(f, "maximum inode size reached: requested {requested_blocks} blocks, maximum {max_blocks}"),

			Self::EmptySymlink { path } => write!(f, "symbolic link has an empty target: {path}"),
			Self::MaximumSymlinkDepthReached { path, max_depth } => write!(f, "maximum symbolic link depth ({max_depth}) reached while resolving: {path}"),
			Self::DeletingRoot { path } => write!(f, "cannot delete the root directory: {path}"),

			Self::InvalidDirEntry { kind: InvalidDirEntryKind::None, block, offset } => write!(f, "invalid directory entry at block {block}, offset {offset}"),
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::ZeroLength, block, offset } => write!(f, "invalid directory entry at block {block}, offset {offset}: record length is zero"),
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::IsFree, block, offset } => write!(f, "invalid directory entry at block {block}, offset {offset}: entry is marked free"),
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::InvalidINode, block, offset } => write!(f, "invalid directory entry at block {block}, offset {offset}: invalid inode"),
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::INodeOOB, block, offset } => write!(f, "invalid directory entry at block {block}, offset {offset}: inode is out of bounds"),
			Self::InvalidDirEntry { kind: InvalidDirEntryKind::InvalidFileType, block, offset } => write!(f, "invalid directory entry at block {block}, offset {offset}: invalid file type"),

			Self::InvalidDir { kind: InvalidDirKind::None, block, offset } => write!(f, "invalid directory at block {block}, offset {offset}"),
			Self::InvalidDir { kind: InvalidDirKind::EntriesOverfillBlock, block, offset } => write!(f, "invalid directory at block {block}, offset {offset}: entries exceed the block size"),

			Self::InvalidINode { kind: InvalidINodeKind::None, size, blocks } => write!(f, "invalid inode (size {size}, {blocks} blocks)"),
			Self::InvalidINode { kind: InvalidINodeKind::SizeGreaterThanAllocatedRegion, size, blocks } => write!(f, "invalid inode: size {size} exceeds its allocated region of {blocks} blocks"),
			Self::InvalidINode { kind: InvalidINodeKind::InvalidDirect { ind, address }, size, blocks } => write!(f, "invalid inode (size {size}, {blocks} blocks): direct block address {address} at index {ind} is invalid"),
			Self::InvalidINode { kind: InvalidINodeKind::DirectOOB { ind, address, max }, size, blocks } => write!(f, "invalid inode (size {size}, {blocks} blocks): direct block address {address} at index {ind} exceeds maximum {max}"),

			Self::InvalidInput(InvalidInputKind::None) => write!(f, "invalid input"),
			Self::InvalidInput(InvalidInputKind::DirEntriesOverfillBlock { block, offset, record_len }) => write!(f, "invalid input: directory entry of {record_len} bytes at block {block}, offset {offset} exceeds the block size"),
			Self::InvalidInput(InvalidInputKind::DirEntriesUnderfillBlock { block, filled }) => write!(f, "invalid input: directory block {block} is underfilled ({filled} of {BLOCK_SIZE} bytes)"),
			Self::InvalidInput(InvalidInputKind::OffsetNotAtDirEntryStart { block, offset }) => write!(f, "invalid input: offset {offset} in directory block {block} is not at an entry start"),
			Self::InvalidInput(InvalidInputKind::INodeIndexOOB { index, max }) => write!(f, "invalid input: inode index {index} is out of bounds (maximum {max})"),
			Self::InvalidInput(InvalidInputKind::BlockIndexOOB { index, max }) => write!(f, "invalid input: block index {index} is out of bounds (maximum {max})"),
			Self::InvalidInput(InvalidInputKind::IndexOOB { index, max }) => write!(f, "invalid input: index {index} is out of bounds (maximum {max})"),
			Self::InvalidInput(InvalidInputKind::INodeBlockIndexOOB { index, max }) => write!(f, "invalid input: inode block index {index} is out of bounds (maximum {max})"),
			Self::InvalidInput(InvalidInputKind::UnknownFileType) => write!(f, "invalid input: unknown file type"),
			Self::InvalidInput(InvalidInputKind::NotEnoughAllocatedBlocks { required, provided }) => write!(f, "invalid input: not enough allocated blocks: {provided} provided, {required} required"),

			Self::InvalidMagic { expected, actual } => write!(f, "invalid filesystem magic number: expected {expected:#010x}, found {actual:#010x}"),
			Self::IO(err) => write!(f, "I/O error: {err}"),
		}
	}
}


pub type FSResult<T> = std::result::Result<T, FSError>;






pub fn assert_error_code<T>(actual: FSResult<T>, expected: FSErrorCode) {
	match actual {
		Ok(_) => { assert!(false) }
		Err(e) => {
			assert_eq!(e.code(), expected);
		}
	}
}
