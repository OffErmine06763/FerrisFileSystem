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
		ind: u16
	}, // "invalid inode direct address"
	DirectOOB {
		ind: u16
	}, // "inode direct address outside of addressable area"
}
#[derive(Debug, PartialEq)]
pub enum InvalidInputKind {
	None,

	DirEntriesOverfillBlock, // "records sizes exceed block size"
	DirEntriesUnderfillBlock, // "records do not fill the last data block"
	OffsetNotAtDirEntryStart, // "the provided offset must mark the start of a new entry"

	INodeIndexOOB {
		index: u32,
		max: u32,
	}, // "inode index past inode table region"
	BlockIndexOOB, // "block out of range"
	IndexOOB, // "index out of range"
	INodeBlockIndexOOB, // "index in the array of blocks assigned to an inode OOB"

	UnknownFileType, // "invalid file type provided"

	NotEnoughAllocatedBlocks,
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

	DirectoryNotEmpty,
	FileNotOpen {
		file_id: u32
	},

	DirFreeRegionTooSmall,
	DirEntryNotFree,

	StorageFull(StorageFullKind),
	
	MaxINodeSize,

	EmptySymlink {
		path: String,	
	},
	MaximumSymlinkDepthReached,

	DeletingRoot,
	
	InvalidDirEntry(InvalidDirEntryKind),
	InvalidDir(InvalidDirKind),

	InvalidINode(InvalidINodeKind),
	
	/// class of errors for when the parameters of a function are ill formed.
	/// trying to read a closed file is not invalid input of the filename.
	/// trying to index an inode out of bounds is, it is not well formed
	InvalidInput(InvalidInputKind),
	
	InvalidMagic, // "invalid magic number, there is no valid FS in the device!"

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
			
			Self::DirFreeRegionTooSmall => FSErrorCode::DirFreeRegionTooSmall,
			Self::DirEntryNotFree       => FSErrorCode::DirEntryNotFree,

			Self::StorageFull(StorageFullKind::None)           => FSErrorCode::StorageFull,
			Self::StorageFull(StorageFullKind::INodeTableFull) => FSErrorCode::INodeTableFull,
			Self::StorageFull(StorageFullKind::DataRegionFull) => FSErrorCode::DataRegionFull,
			
			Self::MaxINodeSize => FSErrorCode::MaxINodeSize,
			
			Self::EmptySymlink { .. } => FSErrorCode::EmptySymlink,
			Self::MaximumSymlinkDepthReached { .. } => FSErrorCode::MaximumSymlinkDepthReached,
			
			Self::DeletingRoot => FSErrorCode::DeletingRoot,

			Self::InvalidDirEntry(InvalidDirEntryKind::None)            => FSErrorCode::InvalidDirEntry,
			Self::InvalidDirEntry(InvalidDirEntryKind::ZeroLength)      => FSErrorCode::ZeroLengthDirEntry,
			Self::InvalidDirEntry(InvalidDirEntryKind::IsFree)          => FSErrorCode::DirEntryIsFree,
			Self::InvalidDirEntry(InvalidDirEntryKind::InvalidINode)    => FSErrorCode::DirEntryInvalidINode,
			Self::InvalidDirEntry(InvalidDirEntryKind::INodeOOB)        => FSErrorCode::DirEntryINodeOOB,
			Self::InvalidDirEntry(InvalidDirEntryKind::InvalidFileType) => FSErrorCode::DirEntryInvalidFileType,
			
			Self::InvalidDir(InvalidDirKind::None)                 => FSErrorCode::InvalidDir,
			Self::InvalidDir(InvalidDirKind::EntriesOverfillBlock) => FSErrorCode::DirEntriesOverfillBlock,

			Self::InvalidINode(InvalidINodeKind::None)							 => FSErrorCode::InvalidINode,
			Self::InvalidINode(InvalidINodeKind::SizeGreaterThanAllocatedRegion) => FSErrorCode::INodeSizeGreaterThanAllocatedRegion,
			Self::InvalidINode(InvalidINodeKind::InvalidDirect { .. })			 => FSErrorCode::INodeInvalidDirect,
			Self::InvalidINode(InvalidINodeKind::DirectOOB { .. })				 => FSErrorCode::INodeDirectOOB,

			Self::InvalidInput(InvalidInputKind::None)                     => FSErrorCode::InvalidInput,
			Self::InvalidInput(InvalidInputKind::DirEntriesOverfillBlock)  => FSErrorCode::InputDirEntriesOverfillBlock,
			Self::InvalidInput(InvalidInputKind::DirEntriesUnderfillBlock) => FSErrorCode::InputDirEntriesUnderfillBlock,
			Self::InvalidInput(InvalidInputKind::OffsetNotAtDirEntryStart) => FSErrorCode::InputOffsetNotAtDirEntryStart,
			Self::InvalidInput(InvalidInputKind::INodeIndexOOB { .. })	   => FSErrorCode::InputINodeIndexOOB,
			Self::InvalidInput(InvalidInputKind::BlockIndexOOB { .. })	   => FSErrorCode::InputBlockIndexOOB,
			Self::InvalidInput(InvalidInputKind::IndexOOB { .. })	       => FSErrorCode::InputIndexOOB,
			Self::InvalidInput(InvalidInputKind::INodeBlockIndexOOB { .. })=> FSErrorCode::InputINodeBlockIndexOOB,
			Self::InvalidInput(InvalidInputKind::UnknownFileType)		   => FSErrorCode::InputUnknownFileType,
			Self::InvalidInput(InvalidInputKind::NotEnoughAllocatedBlocks) => FSErrorCode::InputNotEnoughAllocatedBlocks,

			Self::InvalidMagic => FSErrorCode::InvalidMagic,

			Self::IO { .. } => todo!(),
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
			_ => { write!(f, "") }
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
