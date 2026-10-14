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
	KernelConfig, ReplyStatfs, RenameFlags, ReplyCreate,
};

use std::ffi::OsStr;
use std::time::SystemTime;
use std::path::Path;
use std::io;


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
	
	

	pub fn init_(&mut self, req: &Request, config: &mut KernelConfig) -> io::Result<()> {
		Ok(())		
	}
	pub fn destroy_(&mut self) {

	}
	pub fn statfs_(&self, req: &Request, ino: INodeNo, reply: ReplyStatfs) {
	
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
	pub fn fsyncdir_(&self, req: &Request, ino: INodeNo, fh: FileHandle, datasync: bool, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.fsyncdir(device, req, ino, fh, datasync, reply);
	}
	pub fn releasedir_(&self, req: &Request, ino: INodeNo, fh: FileHandle, flags: OpenFlags, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.releasedir(device, req, ino, fh, flags, reply);
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
	pub fn rename_(&self, req: &Request, parent: INodeNo, name: &OsStr, newparent: INodeNo, newname: &OsStr, flags: RenameFlags, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.rename(device, req, parent, name, newparent, newname, flags, reply);
	}
	pub fn create_(&self, req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, flags: i32, reply: ReplyCreate) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.create(device, req, parent, name, mode, umask, flags, reply);
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
	pub fn flush_(&self, req: &Request, ino: INodeNo, fh: FileHandle, lock_owner: LockOwner, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.flush(device, req, ino, fh, lock_owner, reply);
	}
	pub fn release_(&self, req: &Request, ino: INodeNo, fh: FileHandle, flags: OpenFlags, lock_owner: Option<LockOwner>, flush: bool, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.release(device, req, ino, fh, flags, lock_owner, flush, reply);
	}
	pub fn fsync_(&self, req: &Request, ino: INodeNo, fh: FileHandle, datasync: bool, reply: ReplyEmpty) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.fsync(device, req, ino, fh, datasync, reply);
	}
}