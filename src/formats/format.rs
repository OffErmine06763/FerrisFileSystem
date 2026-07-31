use crate::device::block_device::BlockDevice;

use std::io;


pub trait FsFormat<D: BlockDevice> {
	fn create_file(&mut self, device: &mut D, path: &str) -> io::Result<()>;
	fn delete_file(&mut self);

	fn read(&mut self, inode: u32, buf: &mut [u8]);
	fn write(&mut self);
}