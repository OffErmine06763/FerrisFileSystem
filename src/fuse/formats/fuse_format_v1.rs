use crate::{BlockDevice, FormatV1, FSResult};
use crate::formats::v1::inode::INode;
use crate::fs_utils::*;
use crate::fuse::fuse_format::FuseCompatibleFormat;
use crate::fuse::fuse_utils::*;
use crate::file;

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, 
	ReplyDirectory, ReplyEntry, Request, Generation, TimeOrNow, BsdFileFlags,
	ReplyData
};

use std::{
	ffi::OsStr,
	os::unix::ffi::OsStrExt,
	time::{Duration, SystemTime, UNIX_EPOCH},
};


// Time To Live (how long to cache the metadata)
const TTL: Duration = Duration::from_secs(1);

fn to_v1_inode_number(ino: INodeNo) -> u32 {
	// Fuse ROOT is inode 1;
	(ino.0 - 1) as u32
}
fn from_v1_inode_number(inode: u32) -> INodeNo {
	INodeNo((inode + 1) as u64)
}

fn file_attr_from_inode(inode: &INode, ino: INodeNo) -> FileAttr {
	let attr = FileAttr {
			ino: ino,
			size: inode.size,
			blocks: inode.size / 512,
			atime: UNIX_EPOCH + Duration::from_secs(inode.modified),
			mtime: UNIX_EPOCH + Duration::from_secs(inode.modified),
			ctime: UNIX_EPOCH + Duration::from_secs(inode.created),
			crtime: UNIX_EPOCH + Duration::from_secs(inode.created), // mac only
			kind: my_to_fuse_file_type(&inode.file_type),
			perm: inode.permissions,
			nlink: inode.links as u32,
			uid: 1,
			gid: 2,
			rdev: 0,
			blksize: BLOCK_SIZE as u32,
			flags: 0, // mac only
		};

	attr
}

/// Implementation of fuse functions for format v1
impl<D: BlockDevice> FuseCompatibleFormat<D> for FormatV1 {
	fn getattr(&self, device: &mut D, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let inode_res = self.get_inode(device, to_v1_inode_number(ino));
		if inode_res.is_err() {
			println!("V1 - getattr - no inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		let inode = inode_res.unwrap();

		let attr = file_attr_from_inode(&inode, ino);
		reply.attr(&TTL, &attr);
	}

	fn setattr(&self, device: &mut D, _req: &Request, ino: INodeNo, 
			   mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, size: Option<u64>, 
			   _atime: Option<TimeOrNow>, mtime: Option<TimeOrNow>, ctime: Option<SystemTime>, _fh: Option<FileHandle>, 
			   _crtime: Option<SystemTime>, _chgtime: Option<SystemTime>, _bkuptime: Option<SystemTime>, 
			   _flags: Option<BsdFileFlags>, reply: ReplyAttr) {

		let inode_ind = to_v1_inode_number(ino);
		let inode_res = self.get_inode(device, inode_ind);
		if inode_res.is_err() {
			println!("V1 - setattr - no inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		let mut inode = inode_res.unwrap();


		if let Some(mode) = mode {
			inode.permissions = (mode & 0xFFFF) as u16;
		}
		if let Some(size) = size {
			if inode.file_type != file::FileType::File {
				println!("V1 - setattr - attempted resize of a non-regular file");
				reply.error(Errno::ENOENT);
				return;
			}
			if size < inode.size {
				// TODO: dealloc, possibly add a flag to truncate to do deallocation
			}
			// No need to allocate if size > inode.size, it will happen on the first write
			inode.size = size;
		}
		if let Some(mtime) = mtime {
			inode.modified = match mtime {
				TimeOrNow::Now => { SystemTime::now() },
				TimeOrNow::SpecificTime(time) => { time }
			}.duration_since(UNIX_EPOCH).unwrap().as_secs();
		}
		if let Some(ctime) = ctime {
			inode.created = ctime.duration_since(UNIX_EPOCH).unwrap().as_secs();
		}

		let write_res = self.write_inode(device, inode_ind, &inode);
		if write_res.is_err() {
			println!("V1 - setattr - failed to update the inode");
			reply.error(Errno::ENOENT);
			return;
		}

		let attr = file_attr_from_inode(&inode, ino);
		reply.attr(&TTL, &attr);
	}
	
	fn readdir(&self, device: &mut D, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
		let inode_ind = to_v1_inode_number(ino);

        let inode_res = self.get_inode(device, inode_ind);
		if inode_res.is_err() {
			println!("V1 - readdir - no inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let inode = inode_res.unwrap();
		if inode.file_type != file::FileType::Directory {
			println!("V1 - readdir - not a directory");
			reply.error(Errno::ENOENT); // TODO: check this error code
			return;
		}

		let dir_content_res = self.get_directory_from_inode(device, &inode, inode_ind);
		if dir_content_res.is_err() {
			println!("V1 - readdir - failed to read directory");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let dir_content = dir_content_res.unwrap().entries;

		let mut i = offset;
		while i < dir_content.len() as u64 {
			let entry = dir_content[i as usize];
			let full = reply.add(from_v1_inode_number(entry.inode), i + 1, my_to_fuse_file_type(&entry.file_type), OsStr::from_bytes(&entry.name));
			if full {
				break;
			}
		}

		reply.ok();
	}

	fn readlink(&self, device: &mut D, _req: &Request, ino: INodeNo, reply: ReplyData) {
		let inode_ind = to_v1_inode_number(ino);
		
		let mut buf = [0u8; BLOCK_SIZE];
		let read_res = self.read_symlink(device, inode_ind, &mut buf);
		if read_res.is_err() {
			println!("V1 - readlink - failed to read symlink");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		
		let read = read_res.unwrap();
		reply.data(&buf[0..read]);
	}

	fn lookup(&self, device: &mut D, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
		let parent_inode_ind = to_v1_inode_number(parent);

		let entry_inode_ind_res = self.get_dir_entry_inode(device, parent_inode_ind, name.as_bytes());
		if entry_inode_ind_res.is_err() {
			println!("V1 - lookup - failed to read directory");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let entry_inode_ind_opt = entry_inode_ind_res.unwrap();
		if entry_inode_ind_opt.is_none() {
			println!("V1 - lookup - directory entry not found");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let entry_inode_ind = entry_inode_ind_opt.unwrap();
        let entry_inode_res = self.get_inode(device, entry_inode_ind);
		if entry_inode_res.is_err() {
			println!("V1 - lookup - failed to get directory entry inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		
		let inode = entry_inode_res.unwrap();
		let attr = file_attr_from_inode(&inode, from_v1_inode_number(entry_inode_ind));
		reply.entry(&TTL, &attr, Generation(0));
	}

}