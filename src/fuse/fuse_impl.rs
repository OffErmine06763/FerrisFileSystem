use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, ReplyDirectory, ReplyEntry, Request,
};

use std::{
	ffi::OsStr,
	time::{Duration, SystemTime},
};

use crate::FuseFFS;
use crate::device::block_device::BlockDevice;

// Time To Live (how long to cache the metadata)
const TTL: Duration = Duration::from_secs(1);

impl<D: BlockDevice + Send + 'static> Filesystem for FuseFFS<D> {
	fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
		if ino != INodeNo::ROOT {
			reply.error(Errno::ENOENT);
			return;
		}

		let res = self.do_sth("path");
		if res.is_err() {
			reply.error(Errno::EIO);
			return;
		}

		let now = SystemTime::now();

		let attr = FileAttr {
			ino: INodeNo::ROOT,
			size: 0,
			blocks: 0,
			atime: now,
			mtime: now,
			ctime: now,
			crtime: now,
			kind: FileType::Directory,
			perm: 0o755,
			nlink: 2,
			uid: 1,
			gid: 2,
			rdev: 0,
			blksize: 4096,
			flags: 0,
		};

		reply.attr(&TTL, &attr);
	}

	fn readdir(&self, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
		if ino != INodeNo::ROOT {
			reply.error(Errno::ENOENT);
			return;
		}

		if offset == 0 {
			reply.add(INodeNo::ROOT, 1, FileType::Directory, ".");
			reply.add(INodeNo::ROOT, 2, FileType::Directory, "..");
			reply.add(INodeNo(2), 3, FileType::RegularFile, "hello.txt");
		}

		reply.ok();
	}
}