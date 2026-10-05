use super::fuse_format::FuseCompatibleFormat;
use crate::fs_utils::*;
use crate::device::block_device::BlockDevice;
use crate::formats::v1::format::FormatV1;
use crate::fs_error::FSResult;

use std::sync::Mutex;

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, ReplyDirectory, ReplyEntry, Request,
};


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
	pub fn readdir_(&self, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, reply: ReplyDirectory) {
		let mut inner = self.inner.lock().unwrap();
		let FuseFFSInner {
			device,
			format,
		} = &mut *inner;

		format.readdir(device, req, ino, fh, offset, reply);
	}

}