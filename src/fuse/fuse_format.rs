use crate::{BlockDevice, FormatV1, FSResult};

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, ReplyDirectory, ReplyEntry, Request,
};




/// Format compatible with FUSE.
/// Requires the format operations to expose more implementation-specific details
/// to map with the data required by fuse (not all formats might be fuse compatible)
/// Basically implements the same fuse functions but with also the device passed as parameter
pub trait FuseCompatibleFormat<D: BlockDevice> {
	fn getattr(&self, device: &mut D, req: &Request, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr);
	fn readdir(&self, device: &mut D, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, reply: ReplyDirectory);
}

