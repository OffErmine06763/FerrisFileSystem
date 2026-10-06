use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, 
	ReplyDirectory, ReplyEntry, Request, Generation, TimeOrNow, BsdFileFlags,
	ReplyData
};

use std::{
	ffi::OsStr,
	time::{Duration, SystemTime},
};

use super::fuse_ffs::{FuseFFS, FuseFFSInner};
use crate::device::block_device::BlockDevice;

impl<D: BlockDevice + Send + 'static> Filesystem for FuseFFS<D> {
	fn getattr(&self, req: &Request, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr) {
		self.getattr_(req, ino, fh, reply);
	}
	fn setattr(&self, req: &Request, ino: INodeNo, 
			   mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, 
			   atime: Option<TimeOrNow>, mtime: Option<TimeOrNow>, ctime: Option<SystemTime>, fh: Option<FileHandle>, 
			   crtime: Option<SystemTime>, chgtime: Option<SystemTime>, bkuptime: Option<SystemTime>, 
			   flags: Option<BsdFileFlags>, reply: ReplyAttr) {
		self.setattr_(req, ino, mode, uid, gid, size, atime, mtime, ctime, fh, crtime, chgtime, bkuptime, flags, reply);
	}
	fn readdir(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, reply: ReplyDirectory) {
		self.readdir_(req, ino, fh, offset, reply);
	}
	fn readlink(&self, req: &Request, ino: INodeNo, reply: ReplyData) {
		self.readlink_(req, ino, reply);
	}
	fn lookup(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
		self.lookup_(req, parent, name, reply);
	}
}