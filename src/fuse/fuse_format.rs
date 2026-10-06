use crate::{BlockDevice, FormatV1, FSResult};

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, 
	ReplyDirectory, ReplyEntry, Request, Generation, TimeOrNow, BsdFileFlags,
	ReplyData
};

use std::ffi::OsStr;
use std::time::SystemTime;


/// Format compatible with FUSE.
/// Requires the format operations to expose more implementation-specific details
/// to map with the data required by fuse (not all formats might be fuse compatible)
/// Basically implements the same fuse functions but with also the device passed as parameter
pub trait FuseCompatibleFormat<D: BlockDevice> {
	fn getattr(&self, device: &mut D, req: &Request, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr);
	fn setattr(&self, device: &mut D, req: &Request, ino: INodeNo, 
			   mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, 
			   atime: Option<TimeOrNow>, mtime: Option<TimeOrNow>, ctime: Option<SystemTime>, fh: Option<FileHandle>, 
			   crtime: Option<SystemTime>, chgtime: Option<SystemTime>, bkuptime: Option<SystemTime>, 
			   flags: Option<BsdFileFlags>, reply: ReplyAttr);
	fn readdir(&self, device: &mut D, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, reply: ReplyDirectory);
	fn readlink(&self, device: &mut D, req: &Request, ino: INodeNo, reply: ReplyData);
	fn lookup(&self, device: &mut D, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry);
}

