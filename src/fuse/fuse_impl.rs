#![allow(unused_imports)]

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, LockOwner, 
	ReplyDirectory, ReplyEntry, Request, Generation, TimeOrNow, BsdFileFlags, WriteFlags,
	ReplyData, ReplyOpen, ReplyEmpty, OpenFlags, AccessFlags, FopenFlags, ReplyWrite,
	KernelConfig, ReplyStatfs, RenameFlags, ReplyCreate,
};

use std::{
	ffi::OsStr,
	time::{Duration, SystemTime},
	path::Path,
	io,
};

use super::fuse_ffs::{FuseFFS, FuseFFSInner};
use crate::device::block_device::BlockDevice;

impl<D: BlockDevice + Send + 'static> Filesystem for FuseFFS<D> {

	fn init(&mut self, req: &Request, config: &mut KernelConfig) -> io::Result<()> {
		println!("FuseFFS - init - start");
		self.init_(req, config)
	}
	fn destroy(&mut self) {
		println!("FuseFFS - destroy - start");
		self.destroy_();
	}
	fn statfs(&self, req: &Request, ino: INodeNo, reply: ReplyStatfs) {
		println!("FuseFFS - statfs - start");
		self.statfs_(req, ino, reply);
	}
	

	// ATTRIBUTE OPERATIONS

	fn getattr(&self, req: &Request, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr) {
		println!("FuseFFS - getattr - start");
		self.getattr_(req, ino, fh, reply);
	}
	fn setattr(&self, req: &Request, ino: INodeNo, 
			   mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, 
			   atime: Option<TimeOrNow>, mtime: Option<TimeOrNow>, ctime: Option<SystemTime>, fh: Option<FileHandle>, 
			   crtime: Option<SystemTime>, chgtime: Option<SystemTime>, bkuptime: Option<SystemTime>, 
			   flags: Option<BsdFileFlags>, reply: ReplyAttr) {
		println!("FuseFFS - setattr - start");
		self.setattr_(req, ino, mode, uid, gid, size, atime, mtime, ctime, fh, crtime, chgtime, bkuptime, flags, reply);
	}

	// DIRECTORY OPERATIONS

	fn opendir(&self, req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
		println!("FuseFFS - opendir - ino={:?}", ino);
		self.opendir_(req, ino, flags, reply);
	}
	fn mkdir(&self, req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, reply: ReplyEntry) {
		println!("FuseFFS - mkdir - start");
		self.mkdir_(req, parent, name, mode, umask, reply);
	}
	fn readdir(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, reply: ReplyDirectory) {
		println!("FuseFFS - readdir - start");
		self.readdir_(req, ino, fh, offset, reply);
	}
	fn lookup(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
		println!("FuseFFS - lookup - start");
		self.lookup_(req, parent, name, reply);
	}
	fn rmdir(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
		println!("FuseFFS - rmdir - start");
		self.rmdir_(req, parent, name, reply);
	}
	fn fsyncdir(&self, req: &Request, ino: INodeNo, fh: FileHandle, datasync: bool, reply: ReplyEmpty) {
		println!("FuseFFS - fsyncdir - start");
		self.fsyncdir_(req, ino, fh, datasync, reply);
	}
	fn releasedir(&self, req: &Request, ino: INodeNo, fh: FileHandle, flags: OpenFlags, reply: ReplyEmpty) {
		println!("FuseFFS - releasedir - start");
		self.releasedir_(req, ino, fh, flags, reply);
	}

	// LINK OPERATIONS

	fn readlink(&self, req: &Request, ino: INodeNo, reply: ReplyData) {
		println!("FuseFFS - readlink - start");
		self.readlink_(req, ino, reply);
	}
	fn symlink(&self, req: &Request, parent: INodeNo, link_name: &OsStr, target: &Path, reply: ReplyEntry) {
		println!("FuseFFS - symlink - start");
		self.symlink_(req, parent, link_name, target, reply);
	}

	// PERMISSIONS OPERATIONS
	
	fn access(&self, req: &Request, ino: INodeNo, mask: AccessFlags, reply: ReplyEmpty) {
		println!("FuseFFS - access - ino={:?}, mask={:#x}", ino, mask);
		self.access_(req, ino, mask, reply);
	}

	// FILE OPERATIONS

	fn mknod(&self, req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, rdev: u32, reply: ReplyEntry) {
		println!("FuseFFS - mknod - start");
		self.mknod_(req, parent, name, mode, umask, rdev, reply);
	}
	fn unlink(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
		println!("FuseFFS - unlink - start");
		self.unlink_(req, parent, name, reply);
	}
	fn link(&self, req: &Request, ino: INodeNo, newparent: INodeNo, newname: &OsStr, reply: ReplyEntry) {
		println!("FuseFFS - link - start");
		self.link_(req, ino, newparent, newname, reply);
	}
	fn rename(&self, req: &Request, parent: INodeNo, name: &OsStr, newparent: INodeNo, newname: &OsStr, flags: RenameFlags, reply: ReplyEmpty) {
		println!("FuseFFS - rename - start");
		self.rename_(req, parent, name, newparent, newname, flags, reply);
	}
	fn create(&self, req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, flags: i32, reply: ReplyCreate) {
		println!("FuseFFS - create - start");
		self.create_(req, parent, name, mode, umask, flags, reply);
	}

	// FILE IO OPERATIONS

	fn open(&self, req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
		println!("FuseFFS - open - start");
		self.open_(req, ino, flags, reply);
	}
	fn read(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, size: u32, flags: OpenFlags, lock_owner: Option<LockOwner>, reply: ReplyData) {
		println!("FuseFFS - read - start");
		self.read_(req, ino, fh, offset, size, flags, lock_owner, reply);
	}
	fn write(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, data: &[u8], write_flags: WriteFlags, flags: OpenFlags, lock_owner: Option<LockOwner>, reply: ReplyWrite) {
		println!("FuseFFS - write - start");
		self.write_(req, ino, fh, offset, data, write_flags, flags, lock_owner, reply);
	}
	fn flush(&self, req: &Request, ino: INodeNo, fh: FileHandle, lock_owner: LockOwner, reply: ReplyEmpty) {
		println!("FuseFFS - flush - start");
		self.flush_(req, ino, fh, lock_owner, reply);
	}
	fn release(&self, req: &Request, ino: INodeNo, fh: FileHandle, flags: OpenFlags, lock_owner: Option<LockOwner>, flush: bool, reply: ReplyEmpty) {
		println!("FuseFFS - release - start");
		self.release_(req, ino, fh, flags, lock_owner, flush, reply);
	}
	fn fsync(&self, req: &Request, ino: INodeNo, fh: FileHandle, datasync: bool, reply: ReplyEmpty) {
		println!("FuseFFS - fsync - start");
		self.fsync_(req, ino, fh, datasync, reply);
	}
}