#![allow(unused_imports)]

use crate::{BlockDevice, FormatV1, FSResult};
use crate::formats::v1::inode::INode;
use crate::formats::format::FsFormat;
use crate::fs_utils::*;
use crate::fuse::fuse_format::FuseCompatibleFormat;
use crate::fuse::fuse_utils::*;
use crate::file;

use fuser::{
	Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, ReplyAttr, LockOwner,
	ReplyDirectory, ReplyEntry, Request, Generation, TimeOrNow, BsdFileFlags, WriteFlags,
	ReplyData, ReplyOpen, ReplyEmpty, OpenFlags, AccessFlags, FopenFlags, ReplyWrite,
};

use std::{
	ffi::OsStr,
	os::unix::ffi::OsStrExt,
	time::{Duration, SystemTime, UNIX_EPOCH},
	path::Path,
	io::SeekFrom,
};
use libc;


// Time To Live (how long to cache the metadata)
const TTL: Duration = Duration::from_secs(1);

fn to_v1_inode_number(ino: INodeNo) -> u32 {
	// Fuse ROOT is inode 1;
	(ino.0 - 1) as u32
}
fn from_v1_inode_number(inode: u32) -> INodeNo {
	INodeNo((inode + 1) as u64)
}

fn file_attr_from_inode(inode: &INode, ino: INodeNo) -> FileAttr {
	let attr = FileAttr {
			ino: ino,
			size: inode.size,
			blocks: inode.size / 512,
			atime: UNIX_EPOCH + Duration::from_secs(inode.modified),
			mtime: UNIX_EPOCH + Duration::from_secs(inode.modified),
			ctime: UNIX_EPOCH + Duration::from_secs(inode.created),
			crtime: UNIX_EPOCH + Duration::from_secs(inode.created), // mac only
			kind: my_to_fuse_file_type(&inode.file_type),
			perm: 0o755, // inode.permissions, // if the permissions are wrong, ts will break
			nlink: inode.links as u32,
			uid: 1,
			gid: 2,
			rdev: 0,
			blksize: BLOCK_SIZE as u32,
			flags: 0, // mac only
		};

	attr
}

/// Implementation of fuse functions for format v1
impl<D: BlockDevice> FuseCompatibleFormat<D> for FormatV1 {
	fn getattr(&self, device: &mut D, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        let inode_res = self.get_inode(device, to_v1_inode_number(ino));
		if inode_res.is_err() {
			println!("V1 - getattr - no inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		let inode = inode_res.unwrap();

		let attr = file_attr_from_inode(&inode, ino);
		reply.attr(&TTL, &attr);
	}

	fn setattr(&self, device: &mut D, _req: &Request, ino: INodeNo, 
			   mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, size: Option<u64>, 
			   _atime: Option<TimeOrNow>, mtime: Option<TimeOrNow>, ctime: Option<SystemTime>, _fh: Option<FileHandle>, 
			   _crtime: Option<SystemTime>, _chgtime: Option<SystemTime>, _bkuptime: Option<SystemTime>, 
			   _flags: Option<BsdFileFlags>, reply: ReplyAttr) {

		let inode_ind = to_v1_inode_number(ino);
		let inode_res = self.get_inode(device, inode_ind);
		if inode_res.is_err() {
			println!("V1 - setattr - no inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		let mut inode = inode_res.unwrap();


		if let Some(mode) = mode {
			inode.permissions = (mode & 0xFFFF) as u16;
		}
		if let Some(size) = size {
			if inode.file_type != file::FileType::File {
				println!("V1 - setattr - attempted resize of a non-regular file");
				reply.error(Errno::ENOENT);
				return;
			}
			if size < inode.size {
				// TODO: dealloc, possibly add a flag to truncate to do deallocation
			}
			// No need to allocate if size > inode.size, it will happen on the first write
			inode.size = size;
		}
		if let Some(mtime) = mtime {
			inode.modified = match mtime {
				TimeOrNow::Now => { SystemTime::now() },
				TimeOrNow::SpecificTime(time) => { time }
			}.duration_since(UNIX_EPOCH).unwrap().as_secs();
		}
		if let Some(ctime) = ctime {
			inode.created = ctime.duration_since(UNIX_EPOCH).unwrap().as_secs();
		}

		let write_res = self.write_inode(device, inode_ind, &inode);
		if write_res.is_err() {
			println!("V1 - setattr - failed to update the inode");
			reply.error(Errno::ENOENT);
			return;
		}

		let attr = file_attr_from_inode(&inode, ino);
		reply.attr(&TTL, &attr);
	}
	
	fn opendir(&self, device: &mut D, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
		let inode_ind = to_v1_inode_number(ino);

        let inode_res = self.get_inode(device, inode_ind);
		if inode_res.is_err() {
			println!("V1 - readdir - no inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let inode = inode_res.unwrap();
		if inode.file_type != file::FileType::Directory {
			println!("V1 - readdir - not a directory");
			reply.error(Errno::ENOTDIR); // TODO: check this error code
			return;
		}

		// no temporary per-open state, just return a fh containing the inode number
		// otherwise we would need a distinct fh number for each open
		reply.opened(FileHandle(inode_ind as u64), FopenFlags::empty());
	}

	fn readdir(&self, device: &mut D, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
		let inode_ind = to_v1_inode_number(ino);

        let inode_res = self.get_inode(device, inode_ind);
		if inode_res.is_err() {
			println!("V1 - readdir - no inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let inode = inode_res.unwrap();
		if inode.file_type != file::FileType::Directory {
			println!("V1 - readdir - not a directory");
			reply.error(Errno::ENOTDIR); // TODO: check this error code
			return;
		}

		let dir_content_res = self.get_directory_from_inode(device, &inode, inode_ind);
		if dir_content_res.is_err() {
			println!("V1 - readdir - failed to read directory");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let dir_content = dir_content_res.unwrap().entries;

		let mut i = offset;
		while i < dir_content.len() as u64 {
			let entry = dir_content[i as usize];
			let full = reply.add(from_v1_inode_number(entry.inode), i + 1, my_to_fuse_file_type(&entry.file_type), OsStr::from_bytes(&entry.name));
			if full {
				break;
			}
			i += 1;
		}

		reply.ok();
	}

	fn mkdir(&mut self, device: &mut D, _req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, reply: ReplyEntry) {
		let parent_inode_ind = to_v1_inode_number(parent);
		
		let name_str = name.to_str();
		if name_str.is_none() {
			println!("V1 - mkdir - node name invalid");
			reply.error(Errno::EIO);
			return;
		}
		let name_str = name_str.unwrap();

		let create_res = self.create_child(device, file::FileType::Directory, name_str, parent_inode_ind);
		if create_res.is_err() {
			println!("V1 - mkdir - failed creating directory"); // TODO: check the actual error and reply with the correct Errno
			reply.error(Errno::EIO);
			return;
		}
		
		let created = create_res.unwrap();
		let inode = created.0;
		let entry_inode_number = created.1;
		let entry_ino = from_v1_inode_number(entry_inode_number);
		let attr = file_attr_from_inode(&inode, entry_ino);
		reply.entry(&TTL, &attr, Generation(0));
	}

	fn lookup(&self, device: &mut D, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
		let parent_inode_ind = to_v1_inode_number(parent);

		let entry_inode_ind_res = self.get_dir_entry_inode(device, parent_inode_ind, name.as_bytes());
		if entry_inode_ind_res.is_err() {
			println!("V1 - lookup - failed to read directory");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let entry_inode_ind_opt = entry_inode_ind_res.unwrap();
		if entry_inode_ind_opt.is_none() {
			println!("V1 - lookup - directory entry not found");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}

		let entry_inode_ind = entry_inode_ind_opt.unwrap();
        let entry_inode_res = self.get_inode(device, entry_inode_ind);
		if entry_inode_res.is_err() {
			println!("V1 - lookup - failed to get directory entry inode");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		
		let inode = entry_inode_res.unwrap();
		let attr = file_attr_from_inode(&inode, from_v1_inode_number(entry_inode_ind));
		reply.entry(&TTL, &attr, Generation(0));
	}

	fn rmdir(&mut self, device: &mut D, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
		let parent_inode_ind = to_v1_inode_number(parent);
		
		let name_str = name.to_str();
		if name_str.is_none() {
			println!("V1 - rmdir - node name invalid");
			reply.error(Errno::EIO);
			return;
		}
		let name_str = name_str.unwrap();

		let delete_res = self.delete_child(device, name_str, parent_inode_ind, Some(file::FileType::Directory));
		if delete_res.is_err() {
			println!("V1 - rmdir - failed deleting directory"); // TODO: check the actual error and reply with the correct Errno
			reply.error(Errno::EIO);
			return;
		}

		reply.ok();
	}

	fn readlink(&self, device: &mut D, _req: &Request, ino: INodeNo, reply: ReplyData) {
		let inode_ind = to_v1_inode_number(ino);
		
		let mut buf = [0u8; BLOCK_SIZE];
		let read_res = self.read_symlink(device, inode_ind, &mut buf);
		if read_res.is_err() {
			println!("V1 - readlink - failed to read symlink");
			reply.error(Errno::ENOENT); // TODO: check error codes
			return;
		}
		
		let read = read_res.unwrap();
		reply.data(&buf[0..read]);
	}

	fn symlink(&mut self, device: &mut D, _req: &Request, parent: INodeNo, link_name: &OsStr, target: &Path, reply: ReplyEntry) {
		let parent_inode_ind = to_v1_inode_number(parent);
		
		let name_str = link_name.to_str();
		if name_str.is_none() {
			println!("V1 - symlink - node name invalid");
			reply.error(Errno::EIO);
			return;
		}
		let name_str = name_str.unwrap();

		let create_res = self.create_child(device, file::FileType::Symlink, name_str, parent_inode_ind);
		if create_res.is_err() {
			println!("V1 - symlink - failed creating symlink"); // TODO: check the actual error and reply with the correct Errno
			reply.error(Errno::EIO);
			return;
		}

		let created = create_res.unwrap();
		let mut inode = created.0;
		let entry_inode_number = created.1;
		let entry_ino = from_v1_inode_number(entry_inode_number);

		let write_res = self.write_symlink(device, &mut inode, entry_inode_number, target.to_str().unwrap());
		if write_res.is_err() {
			println!("V1 - symlink - failed writing symlink content"); // TODO: check the actual error and reply with the correct Errno
			reply.error(Errno::EIO);
			return;
		}
		
		let attr = file_attr_from_inode(&inode, entry_ino);
		reply.entry(&TTL, &attr, Generation(0));
	}

	fn access(&self, _device: &mut D, _req: &Request, _ino: INodeNo, _mask: AccessFlags, reply: ReplyEmpty) {
		// permissions not implemented, every access is allowed
		reply.ok();
	}

	fn mknod(&mut self, device: &mut D, _req: &Request, parent: INodeNo, name: &OsStr, mode: u32, umask: u32, rdev: u32, reply: ReplyEntry) {
		let parent_inode_ind = to_v1_inode_number(parent);
		
		let name_str = name.to_str();
		if name_str.is_none() {
			println!("V1 - mknod - node name invalid");
			reply.error(Errno::EIO);
			return;
		}
		let name_str = name_str.unwrap();

		let file_type = mode & libc::S_IFMT;
		if file_type != libc::S_IFREG {
			println!("V1 - mknod - attempted creating unsupported file type");
			reply.error(Errno::ENOSYS);
			return;
		}

		let permissions = (mode & 0o7777) & !umask; // TODO: use those permissions
		let file_type = file::FileType::File;

		let create_res = self.create_child(device, file_type, name_str, parent_inode_ind);
		if create_res.is_err() {
			println!("V1 - mknod - failed creating node"); // TODO: check the actual error and reply with the correct Errno
			reply.error(Errno::EIO);
			return;
		}
		
		let created = create_res.unwrap();
		let inode = created.0;
		let entry_inode_number = created.1;
		let entry_ino = from_v1_inode_number(entry_inode_number);
		let attr = file_attr_from_inode(&inode, entry_ino);
		reply.entry(&TTL, &attr, Generation(0));
	}

	fn unlink(&mut self, device: &mut D, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
		let parent_inode_ind = to_v1_inode_number(parent);
		
		let name_str = name.to_str();
		if name_str.is_none() {
			println!("V1 - unlink - node name invalid");
			reply.error(Errno::EIO);
			return;
		}
		let name_str = name_str.unwrap();

		let delete_res = self.delete_child(device, name_str, parent_inode_ind, Some(file::FileType::File));
		if delete_res.is_err() {
			println!("V1 - unlink - failed deleting directory"); // TODO: check the actual error and reply with the correct Errno
			reply.error(Errno::EIO);
			return;
		}

		reply.ok();
	}

	fn link(&mut self, device: &mut D, req: &Request, ino: INodeNo, newparent: INodeNo, newname: &OsStr, reply: ReplyEntry) {
		let dst_parent_inode_ind = to_v1_inode_number(newparent);
		let tgt_inode_ind = to_v1_inode_number(ino);
		
		let tgt_inode_res = self.get_inode(device, tgt_inode_ind);
		if tgt_inode_res.is_err() {
			println!("V1 - link - target file does not exist");
			reply.error(Errno::ENOENT);
			return;
		}
		let mut tgt_inode = tgt_inode_res.unwrap();
		// reject links to directories, generally you don't want them, even if the system supports them
		if tgt_inode.file_type == file::FileType::Directory {
			println!("V1 - link - target file is a directory");
			reply.error(Errno::EPERM);
			return;
		}

		let dst_parent_inode_res = self.get_inode(device, dst_parent_inode_ind);
		if dst_parent_inode_res.is_err() {
			println!("V1 - link - destination directory does not exist");
			reply.error(Errno::ENOENT);
			return;
		}
		let mut dst_parent_inode = dst_parent_inode_res.unwrap();
		if dst_parent_inode.file_type != file::FileType::Directory {
			println!("V1 - link - destination file is not a directory");
			reply.error(Errno::ENOTDIR);
			return;
		}

		let res = self.create_child_hardlink(device, &mut tgt_inode, tgt_inode_ind, newname.to_str().unwrap(), &mut dst_parent_inode, dst_parent_inode_ind);
		if res.is_err() {
			println!("V1 - link - failed to create hardlink");
			reply.error(Errno::EIO);
			return;
		}

		let attr = file_attr_from_inode(&tgt_inode, ino);
		reply.entry(&TTL, &attr, Generation(0));		
	}

	fn open(&mut self, device: &mut D, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
		let inode_ind = to_v1_inode_number(ino);
		let open_res = self.open_file_from_inode(device, inode_ind);
		if open_res.is_err() {
			println!("V1 - open - failed to open file");
			reply.error(Errno::ENOENT);
			return;
		}
		let opened = open_res.unwrap();

		// files do have a per-open state, here we just have to convert our handle to fuser one.
		reply.opened(FileHandle(opened.id as u64), FopenFlags::empty());
	}
	
	fn read(&mut self, device: &mut D, _req: &Request, _ino: INodeNo, fh: FileHandle, offset: u64, size: u32, flags: OpenFlags, _lock_owner: Option<LockOwner>, reply: ReplyData) {
		let mut buf = vec![0u8; size as usize];
		let file = file::File{ id: fh.0 as u32 };
		
		// seek from start is inexpensive, just do it every time
		let seek_res = self.seek(device, &file, SeekFrom::Start(offset));
		if seek_res.is_err() {
			println!("V1 - read - failed to seek file");
			reply.error(Errno::ENOENT);
			return;
		}
		let read_res = FsFormat::<D>::read(self, device, &file, &mut buf);
		if read_res.is_err() {
			println!("V1 - read - failed to read file");
			reply.error(Errno::ENOENT);
			return;
		}

		let read = read_res.unwrap();
		reply.data(&buf[0..read]);
	}
	
	fn write(&mut self, device: &mut D, _req: &Request, _ino: INodeNo, fh: FileHandle, offset: u64, data: &[u8], write_flags: WriteFlags, flags: OpenFlags, _lock_owner: Option<LockOwner>, reply: ReplyWrite) {
		let file = file::File{ id: fh.0 as u32 };
		
		// seek from start is inexpensive, just do it every time
		let seek_res = self.seek(device, &file, SeekFrom::Start(offset));
		if seek_res.is_err() {
			println!("V1 - write - failed to seek file");
			reply.error(Errno::ENOENT);
			return;
		}
		let write_res = FsFormat::<D>::write(self, device, &file, data);
		if write_res.is_err() {
			println!("V1 - write - failed to write file");
			reply.error(Errno::ENOENT);
			return;
		}

		let wrote = write_res.unwrap();
		reply.written(wrote as u32);
	}
}