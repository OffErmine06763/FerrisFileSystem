#![allow(unused_imports)]

use super::fuse_format::FuseCompatibleFormat;
use crate::fs_utils::*;
use crate::device::block_device::BlockDevice;
use crate::formats::v1::format::FormatV1;
use crate::fs_error::FSResult;

use std::sync::Mutex;

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, LockOwner, 
	ReplyDirectory, ReplyEntry, Request, Generation, TimeOrNow, BsdFileFlags, WriteFlags,
	ReplyData, ReplyOpen, ReplyEmpty, OpenFlags, AccessFlags, FopenFlags, ReplyWrite,
};

use std::ffi::OsStr;
use std::time::SystemTime;
use std::path::Path;


/// FFS with underlying format compatible with FUSE
pub struct FuseFFS<D: BlockDevice + Send + 'static> {
    inner: Mutex<FuseFFSInner<D>>,
}

/// Synchronous FFS with underlying format compatible with FUSE
pub struct FuseFFSInner<D: BlockDevice> {
    device: D,
    format: Box<dyn FuseCompatibleFormat<D> + Send>,
}


impl<D: BlockDevice + Send + 'static> FuseFFS<D> {
	pub fn format(device: &mut D, version: Version) -> FSResult<()> {
		match version {
			Version::V1 => { return FormatV1::format(device); }
		}
	}
	pub fn mount(mut device: D) -> FSResult<Self> {
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(0, &mut buf)?;

		let version = read_version(&buf);
		if !version_supports_fuse(&version) {
			panic!("Version does NOT support FUSE"); // TODO: replace panics with FSErrors
		}
		let format: Box<dyn FuseCompatibleFormat<D> + Send + Sync> = match version {
			Version::V1 => { Box::new(FormatV1::mount(&mut device)?) }
		};

		Ok(Self { inner: Mutex::new(FuseFFSInner::<D> { device, format }) })
	}



	pub fn getattr_(&self, req: &Request, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.getattr(device, req, ino, fh, reply);
	}
	pub fn setattr_(&self, req: &Request, ino: INodeNo, 
			   mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, 
			   atime: Option<TimeOrNow>, mtime: Option<TimeOrNow>, ctime: Option<SystemTime>, fh: Option<FileHandle>, 
			   crtime: Option<SystemTime>, chgtime: Option<SystemTime>, bkuptime: Option<SystemTime>, 
			   flags: Option<BsdFileFlags>, reply: ReplyAttr) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.setattr(device, req, ino, mode, uid, gid, size, atime, mtime, ctime, fh, crtime, chgtime, bkuptime, flags, reply);
	}
	pub fn opendir_(&self, req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.opendir(device, req, ino, flags, reply);
	}
	pub fn readdir_(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, reply: ReplyDirectory) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.readdir(device, req, ino, fh, offset, reply);
	}
	pub fn mkdir_(&self, req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, reply: ReplyEntry) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.mkdir(device, req, parent, name, mode, umask, reply);
	}
	pub fn lookup_(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.lookup(device, req, parent, name, reply);
	}
	pub fn rmdir_(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.rmdir(device, req, parent, name, reply);
	}
	pub fn readlink_(&self, req: &Request, ino: INodeNo, reply: ReplyData) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.readlink(device, req, ino, reply);
	}
	pub fn symlink_(&self, req: &Request, parent: INodeNo, link_name: &OsStr, target: &Path, reply: ReplyEntry) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.symlink(device, req, parent, link_name, target, reply);
	}
	pub fn access_(&self, req: &Request, ino: INodeNo, mask: AccessFlags, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.access(device, req, ino, mask, reply);
	}
	pub fn mknod_(&self, req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, rdev: u32, reply: ReplyEntry) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.mknod(device, req, parent, name, mode, umask, rdev, reply);
	}
	pub fn unlink_(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.unlink(device, req, parent, name, reply);
	}
	pub fn link_(&self, req: &Request, ino: INodeNo, newparent: INodeNo, newname: &OsStr, reply: ReplyEntry) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.link(device, req, ino, newparent, newname, reply);
	}
	pub fn open_(&self, req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.open(device, req, ino, flags, reply);
	}
	pub fn read_(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, size: u32, flags: OpenFlags, lock_owner: Option<LockOwner>, reply: ReplyData) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.read(device, req, ino, fh, offset, size, flags, lock_owner, reply);
	}
	pub fn write_(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, data: &[u8], write_flags: WriteFlags, flags: OpenFlags, lock_owner: Option<LockOwner>, reply: ReplyWrite) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.write(device, req, ino, fh, offset, data, write_flags, flags, lock_owner, reply);
	}
}