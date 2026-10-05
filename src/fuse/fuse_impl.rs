use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, ReplyDirectory, ReplyEntry, Request,
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
	fn readdir(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, reply: ReplyDirectory) {
		self.readdir_(req, ino, fh, offset, reply);
	}
}