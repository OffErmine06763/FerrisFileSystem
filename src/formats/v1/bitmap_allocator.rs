use crate::fs_utils::BLOCK_SIZE;
use crate::block_device::BlockDevice;

use std::io;


pub struct BitmapAllocator {
	/// first block where the bitmap resides
	start: u32,
	/// number of blocks dedicated to the bitmap
	size: u32,
}

impl BitmapAllocator {
	pub fn new(start: u32, size: u32) -> Self {
		Self { start, size }
	}

	pub fn allocate<D: BlockDevice>(&mut self, device: &mut D) -> io::Result<u32> {
		//! returns the index of the allocated block RELATIVE to the data region

		let mut buf = [0u8; BLOCK_SIZE];
		
		// scan the device from start to start + size for a zero bit
		for i in 0..self.size {
			let bitmap_addr = i + self.start;
			device.read_block(bitmap_addr, &mut buf)?;
			
			// read the buffer until the first zero bit, return it
			for j in 0..BLOCK_SIZE {
				if buf[j] == u8::MAX { continue; }

				let pos = buf[j].trailing_ones();
				buf[j] |= 1 << pos;
				device.write_block(bitmap_addr, &buf)?;
				// j * 8 + i * BLOCK_SIZE * 8 + pos
				return Ok((j as u32 + i * BLOCK_SIZE as u32) * 8 + pos as u32);
			}
		}

		Err(io::Error::new(io::ErrorKind::StorageFull, "all blocks are already in use"))
	}
}