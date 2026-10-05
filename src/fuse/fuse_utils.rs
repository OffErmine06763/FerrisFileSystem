use crate::file;

use fuser::{
	self, Errno, FileAttr, FileHandle, Filesystem, INodeNo, ReplyAttr, ReplyDirectory, ReplyEntry, Request,
};

pub fn MyToFuseFileType(file_type: &file::FileType) -> fuser::FileType {
    match file_type {
        file::FileType::File      => fuser::FileType::RegularFile,
        file::FileType::Directory => fuser::FileType::Directory,
        file::FileType::Symlink   => fuser::FileType::Symlink,
        _ => fuser::FileType::RegularFile,
    }
}

pub fn FuseToMyFileType(file_type: &fuser::FileType) -> file::FileType {
    match file_type {
        fuser::FileType::RegularFile => file::FileType::File,
        fuser::FileType::Directory   => file::FileType::Directory,
        fuser::FileType::Symlink     => file::FileType::Symlink,
        _ => file::FileType::Unknown,
    }
}