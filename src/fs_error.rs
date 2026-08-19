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

	IsADirectory		  = Self::generic(3),
	NotADirectory		  = Self::generic(4),
	IsAFile				  = Self::generic(5),
	NotAFile			  = Self::generic(6),

	DirectoryNotEmpty = Self::generic(7),
	FileNotOpen       = Self::generic(8),

	DirFreeRegionTooSmall = Self::generic(9),
	DirEntryNotFree		  = Self::generic(10),

	StorageFull	   = Self::generic(11),
	INodeTableFull = Self::generic(12),
	DataRegionFull = Self::generic(13),

	InvalidInput				  = Self::generic(14),
	InputDirEntriesOverfillBlock  = Self::generic(15),
	InputDirEntriesUnderfillBlock = Self::generic(16),
	InputOffsetNotAtDirEntryStart = Self::generic(17),
	InputINodeIndexOOB			  = Self::generic(18),
	InputBlockIndexOOB			  = Self::generic(19),
	InputUnknownFileType		  = Self::generic(20),

	InvalidDirEntry      = Self::corruption(1),
	ZeroLengthDirEntry   = Self::corruption(2),
	DirEntryIsFree		 = Self::corruption(3),
	DirEntryInvalidINode = Self::corruption(4),
	DirEntryINodeOOB	 = Self::corruption(5),

	InvalidDir				= Self::corruption(6),
	DirEntriesOverfillBlock	= Self::corruption(7),

	InvalidINode						= Self::corruption(8),
	INodeSizeGreaterThanAllocatedRegion = Self::corruption(9),
	INodeInvalidDirect					= Self::corruption(10),
	INodeDirectOOB						= Self::corruption(11),

	InvalidMagic = Self::corruption(12),
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

	UnknownFileType, // "invalid file type provided"
}

#[derive(Debug)]
pub enum FSError {
	FileDoesNotExist {
		path: String,
	},
	DirectoryDoesNotExist {
		path: String,
	}, // "directory doesn't exist"
	
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

	DirectoryNotEmpty,
	FileNotOpen {
		file_id: u32
	},

	DirFreeRegionTooSmall,
	DirEntryNotFree,

	StorageFull(StorageFullKind),
	
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
			
			Self::IsADirectory  { .. } => FSErrorCode::IsADirectory,
			Self::NotADirectory { .. } => FSErrorCode::NotADirectory,
			Self::IsAFile       { .. } => FSErrorCode::IsAFile,
			Self::NotAFile      { .. } => FSErrorCode::NotAFile,

			Self::DirectoryNotEmpty { .. } => FSErrorCode::DirectoryNotEmpty,
			Self::FileNotOpen		{ .. } => FSErrorCode::FileNotOpen,
			
			Self::DirFreeRegionTooSmall => FSErrorCode::DirFreeRegionTooSmall,
			Self::DirEntryNotFree       => FSErrorCode::DirEntryNotFree,

			Self::StorageFull(StorageFullKind::None)           => FSErrorCode::StorageFull,
			Self::StorageFull(StorageFullKind::INodeTableFull) => FSErrorCode::INodeTableFull,
			Self::StorageFull(StorageFullKind::DataRegionFull) => FSErrorCode::DataRegionFull,

			Self::InvalidDirEntry(InvalidDirEntryKind::None)         => FSErrorCode::InvalidDirEntry,
			Self::InvalidDirEntry(InvalidDirEntryKind::ZeroLength)   => FSErrorCode::ZeroLengthDirEntry,
			Self::InvalidDirEntry(InvalidDirEntryKind::IsFree)       => FSErrorCode::DirEntryIsFree,
			Self::InvalidDirEntry(InvalidDirEntryKind::InvalidINode) => FSErrorCode::DirEntryInvalidINode,
			Self::InvalidDirEntry(InvalidDirEntryKind::INodeOOB)     => FSErrorCode::DirEntryINodeOOB,
			
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
			Self::InvalidInput(InvalidInputKind::UnknownFileType)		   => FSErrorCode::InputUnknownFileType,

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
