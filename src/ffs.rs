use crate::fs_utils::*;
use crate::device::block_device::{self, BlockDevice};
use crate::device::memory_device::MemoryDevice;
use crate::formats::{self, format::FsFormat, v1::format::*};

use std::io;
use std::path::Path;


pub struct FFS<D: BlockDevice> {
	device: D,
	format: Box<dyn FsFormat<D>>,
}


impl<D: BlockDevice> FFS<D> {
	pub fn mount(mut device: D) -> io::Result<Self> {
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(0, &mut buf)?;

		let version = read_version(&buf);
		let format: Box<dyn FsFormat<D>> = match version {
			Version::V1 => { Box::new(FormatV1::mount(&mut device)?) }
		};

		Ok(Self { device, format })
	}

	pub fn create_file(&mut self, path: &str, file_type: FileType) -> io::Result<()> {
		self.format.create_file(&mut self.device, path, file_type)
	}
	pub fn delete_file(&mut self, path: &str) -> io::Result<()> {
		self.format.delete_file(&mut self.device, path)
	}
	pub fn read(&mut self, inode: u32, buf: &mut [u8]) {
		self.format.read(inode, buf);
	}
	pub fn write(&mut self) {
		self.format.write();
	}
}

impl FFS<MemoryDevice> {
	pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
		self.device.save(path)
	}
}




/*

pub enum Version {
	V1,
	V2,
}

pub struct SuperblockV1 {
	pub magic: u64,
	pub version: u32,

	pub block_size: u32,
	pub total_blocks: u64,

	pub inode_bitmap_start: u64,
	pub inode_bitmap_blocks: u64,

	pub block_bitmap_start: u64,
	pub block_bitmap_blocks: u64,

	pub inode_table_start: u64,
	pub inode_table_blocks: u64,

	pub data_start: u64,
}

pub struct BitmapAllocator {

}
pub struct INodeHandlerV1 {

}




pub trait FsFormat {
	fn create_file(&mut self, path: &str);
	fn delete_file(&mut self);

	fn read(&mut self, inode: u32, buf: &mut [u8]);
	fn write(&mut self);
}



pub struct FormatV1<D: BlockDevice> {
	device: D,
	superblock: SuperblockV1,
	inode_allocator: BitmapAllocator,
	block_allocator: BitmapAllocator,
	inode_handler: INodeHandlerV1,
}

impl<D: BlockDevice> FsFormat for FormatV1<D> {
	fn create_file(&mut self, path: &str) {
		let inode_addr = self.inode_allocator.allocate();
		let block_addr = self.block_allocator.allocate();
		self.inode_handler.create_inode(inode_addr); // this will do device.write_block(inode_addr, inode_data);
	}
	fn delete_file(&mut self) {
		todo!()
	}

	fn read(&mut self, inode: u32, buf: &mut [u8]) {
		todo!()
	}
	fn write(&mut self) {
		todo!()
	}
}

impl<D: BlockDevice> FormatV1<D> {
	pub const VERSION: Version = Version::V1;

	fn mount(device: D) -> Self {
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(0, buf);
		// initialize stuff like self.superblock, self.inode_allocator...
		let mut format: Self;
		format.device = *device;
		format
	}
	fn format(device: &mut D) {
		let mut buf = [0u8; BLOCK_SIZE];
		// initialize the buf with the proper data
		todo!();
		device.write_block(0, &buf);
	}
}



pub struct FormatV2<D: BlockDevice> {
	device: D,
	// TODO
}

impl<D: BlockDevice> FsFormat for FormatV2<D> {
	fn create_file(&mut self, path: &str) {
		todo!()
	}
	fn delete_file(&mut self) {
		todo!()
	}

	fn read(&mut self, inode: u32, buf: &mut [u8]) {
		todo!()
	}
	fn write(&mut self) {
		todo!()
	}
}


impl<D: BlockDevice> FormatV2<D> {
	pub const VERSION: Version = Version::V2;

	fn mount(device: D) -> Self {
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(0, buf);
		// initialize stuff like self.superblock, self.inode_allocator...
		let mut format: Self;
		format.device = device;
		format
	}
	fn format(device: &mut D) {
		let mut buf = [0u8; BLOCK_SIZE];
		// initialize the buf with the proper data
		todo!();
		device.write_block(0, &buf);
	}
}



pub struct FFS<F: FsFormat> {
	format: F,
}


pub fn create_ffs<D: BlockDevice>(device: &mut D) -> FFS<_> {
	let mut buf = [0u8; BLOCK_SIZE];
	device.read_block(0, &buf);
	let version = get_version(buf);
	let format = match version {
		Version::V1 => FormatV1::mount(device),
		Version::V2 => FormatV2::mount(device),
	};
	FFS { format }
}

impl<F: FsFormat> FFS<F> {
	fn read(&mut self, inode: u32, buf: &mut [u8]) {
		self.format.read(inode, buf);
	}
	fn write(&mut self) {
		self.format.write();
	}

	fn create_file(&mut self) {
		self.format.create_file("path");
	}
	fn delete_file(&mut self) {
		self.format.delete_file();
	}
}

*/




/*

pub trait Allocable {
	fn allocate(&self);
}
pub trait Writable {
	fn write(&self, block: u64, data: &[u8; BLOCK_SIZE]);
}

pub struct Superblock {

}

pub struct BlockBitmap {

}

pub struct BlockBitmapV2 {

}

pub trait 



pub trait Layout<D: BlockDevice> {
	fn allocate(&self);
	fn free(&self);
	fn write(&self, block: u64, data: &[u8; BLOCK_SIZE]);
	fn initilize(&self);
	fn create_file(&self);
}

pub struct LayoutV1<D: BlockDevice> {
	device: &D,
	superblock: SuperblockV1,
	bitmap: BitmapV1,
	inodes: InodesV1,
	data: DataV1,
}

pub struct LayoutV2<D: BlockDevice> {
	device: &D,
	superblock: SuperblockV1,
	bitmap: BitmapV2,
	inodes: InodesV1,
	data: DataV1,
}

impl<D: BlockDevice> Layout<D> for LayoutV1<D> {
	fn allocate(&self) {
		let block_addr = self.bitmap.allocate();
	}
	fn write(&self, block: u64, data: &[u8; BLOCK_SIZE]) {
		self.device.write_block(block, data);
	}
	fn free(&self) {
		
	}
	fn initilize(&self) {
		let mut buf = [0u8; BLOCK_SIZE];
		self.device.read_block(0, &buf);
		self.superblock.initialize(&buf);
		self.device.read_block(1, &buf);
		self.bitmap.initialize(&buf);
		// ...
	}
	fn create_file(&self) {
		let inode_addr = self.inodes.allocate();
		let block_addr = self.bitmap.allocate();
		self.inode.add_block(block_addr);
	}
}


*/