#![allow(unused_imports)]

use crate::{BlockDevice, FormatV1, FSResult};

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, LockOwner, 
	ReplyDirectory, ReplyEntry, Request, Generation, TimeOrNow, BsdFileFlags, WriteFlags,
	ReplyData, ReplyOpen, ReplyEmpty, OpenFlags, AccessFlags, FopenFlags, ReplyWrite,
};

use std::ffi::OsStr;
use std::time::SystemTime;
use std::path::Path;

/// Format compatible with FUSE.
/// Requires the format operations to expose more implementation-specific details
/// to map with the data required by fuse (not all formats might be fuse compatible)
/// Basically implements the same fuse functions but with also the device passed as parameter
pub trait FuseCompatibleFormat<D: BlockDevice> {
	/// Return the attributes of an inode.
	fn getattr(&self, device: &mut D, req: &Request, ino: INodeNo, fh: Option<FileHandle>, reply: ReplyAttr);
	/// Set the attributes of an inode.
	/// Size should be changed only for regular files.
	fn setattr(&self, device: &mut D, req: &Request, ino: INodeNo, 
			   mode: Option<u32>, uid: Option<u32>, gid: Option<u32>, size: Option<u64>, 
			   atime: Option<TimeOrNow>, mtime: Option<TimeOrNow>, ctime: Option<SystemTime>, fh: Option<FileHandle>, 
			   crtime: Option<SystemTime>, chgtime: Option<SystemTime>, bkuptime: Option<SystemTime>, 
			   flags: Option<BsdFileFlags>, reply: ReplyAttr);
	/// Equivalent of open for directories, basically used to check permissions and create a temporary handle
	/// for the directory (passed to the reply). This handle will be used to store the temporary per-open state
	/// of the directory, and will be passed as argument to other directory operations (readdir...).
	fn opendir(&self, device: &mut D, req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen);
	/// Returns the contents of a directory. The fh contains the handle returned by opendir.
	/// The reply is a buffer for several entries and it may not be able to store all the entries of the directory.
	/// Offset is a custom index, representing the first entry to be added to the reply.
	/// Each entry added to the reply also has to specify the offset of the NEXT entry.
	fn readdir(&self, device: &mut D, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, reply: ReplyDirectory);
	/// Search for a children of the given parent.
	/// There is no path resolution: the name must be a direct children of parent .
	fn lookup(&self, device: &mut D, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry);
	/// Create a directory given the parent inode.
	/// Mode contains the permissions, umask is the bits to ignore in mode.
	fn mkdir(&mut self, device: &mut D, req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, reply: ReplyEntry);
	/// Delete an entry in a directory and decrement the reference count of the directory it points to.
	/// The target must be an empty directory.
	fn rmdir(&mut self, device: &mut D, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty);
	/// Read the content of a symlink.
	fn readlink(&self, device: &mut D, req: &Request, ino: INodeNo, reply: ReplyData);
	fn symlink(&mut self, device: &mut D, req: &Request, parent: INodeNo, link_name: &OsStr, target: &Path, reply: ReplyEntry);
	/// Checks permissions to access a file for a certain set of operations contained in mask, based on the request ids.
	/// Note that permissions should checked before every operation, as they might change after calling access.
	fn access(&self, device: &mut D, req: &Request, ino: INodeNo, mask: AccessFlags, reply: ReplyEmpty);
	/// Create a regular file. It can actually be other fancy file types, but we don't support them.
	/// File type and permissions are encoded in parameter mode.
	fn mknod(&mut self, device: &mut D, req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, rdev: u32, reply: ReplyEntry);
	/// Delete an entry in a directory and decrement the reference count of the file it points to.
	/// The target must be a regular file.
	fn unlink(&mut self, device: &mut D, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty);
	/// Crete hardlink of file ino, as children of directory newparent, with name newname.
	/// The ino may not reference a directory.
	fn link(&mut self, device: &mut D, req: &Request, ino: INodeNo, newparent: INodeNo, newname: &OsStr, reply: ReplyEntry);
	fn open(&mut self, device: &mut D, req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen);
	fn read(&mut self, device: &mut D, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, size: u32, flags: OpenFlags, lock_owner: Option<LockOwner>, reply: ReplyData);
	fn write(&mut self, device: &mut D, req: &Request, ino: INodeNo, fh: FileHandle, offset: u64, data: &[u8], write_flags: WriteFlags, flags: OpenFlags, lock_owner: Option<LockOwner>, reply: ReplyWrite);
}

