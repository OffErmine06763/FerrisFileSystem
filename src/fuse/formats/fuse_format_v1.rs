use crate::{BlockDevice, FormatV1, FSResult};
use crate::formats::v1::inode::INode;
use crate::fs_utils::*;
use crate::fuse::fuse_format::FuseCompatibleFormat;
use crate::fuse::fuse_utils::*;
use crate::file;

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, ReplyDirectory, ReplyEntry, Request,
};

use std::{
	ffi::OsStr,
	os::unix::ffi::OsStrExt,
	time::{Duration, SystemTime, UNIX_EPOCH},
};


// Time To Live (how long to cache the metadata)
const TTL: Duration = Duration::from_secs(1);

fn ToV1INodeNumber(ino: INodeNo) -> u32 {
	// Fuse ROOT is inode 1;
	(ino.0 - 1) as u32
}
fn FromV1INodeNumber(inode: u32) -> INodeNo {
	INodeNo((inode + 1) as u64)
}

/// Implementation of fuse functions for format v1
impl<D: BlockDevice> FuseCompatibleFormat<D> for FormatV1 {
	fn getattr(&self, device: &mut D, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let inode_opt = self.get_inode(device, ToV1INodeNumber(ino));
		if inode_opt.is_err() {
			println!("Error 1");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		let inode = inode_opt.unwrap();

		let attr = FileAttr {
			ino: ino,
			size: inode.size,
			blocks: inode.size / 512,
			atime: UNIX_EPOCH + Duration::from_secs(inode.modified),
			mtime: UNIX_EPOCH + Duration::from_secs(inode.modified),
			ctime: UNIX_EPOCH + Duration::from_secs(inode.created),
			crtime: UNIX_EPOCH + Duration::from_secs(inode.created), // mac only
			kind: MyToFuseFileType(&inode.file_type),
			perm: inode.permissions,
			nlink: inode.links as u32,
			uid: 1,
			gid: 2,
			rdev: 0,
			blksize: BLOCK_SIZE as u32,
			flags: 0, // mac only
		};

		reply.attr(&TTL, &attr);
	}
	fn readdir(&self, device: &mut D, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
		let inode_ind = ToV1INodeNumber(ino);

        let inode_opt = self.get_inode(device, inode_ind);
		if inode_opt.is_err() {
			println!("Error 2");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let inode = inode_opt.unwrap();
		if inode.file_type != file::FileType::Directory {
			println!("Error 3");
			reply.error(Errno::ENOENT); // TODO: check this error code
			return;
		}

		let dir_content_opt = self.get_directory_from_inode(device, &inode, inode_ind);
		if dir_content_opt.is_err() {
			println!("Error 4");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let dir_content = dir_content_opt.unwrap().entries;

		let mut i = offset;
		while i < dir_content.len() as u64 {
			let entry = dir_content[i as usize];
			let full = reply.add(FromV1INodeNumber(entry.inode), i + 1, MyToFuseFileType(&entry.file_type), OsStr::from_bytes(&entry.name));
			if full {
				break;
			}
		}

		reply.ok();
	}
}