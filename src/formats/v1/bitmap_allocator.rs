use crate::fs_utils::*;
use crate::block_device::BlockDevice;

use std::io;


pub struct BitmapAllocator {
	/// first block where the bitmap resides
	start: u32,
	/// number of blocks dedicated to the bitmap
	size: u32,
	/// 1 past the highest value that may be returned by allocate
	/// (the bitmap itself might be bigger than the region it maps to)
	max_index: u32,
}

impl BitmapAllocator {
	pub const ERR_MAPPED_REGION_FULL: &str = "cannot allocate, mapped region full";
	pub const ERR_BITMAP_FULL: &str = "cannot allocate, bitmap full";

	pub fn new(start: u32, size: u32, max_index: u32) -> Self {
		Self { start, size, max_index }
	}

	pub fn get_index(bitmap_block: u32, byte_in_block: u32, bit_in_byte: u32) -> u32 {
		return (byte_in_block as u32 + bitmap_block * BLOCK_SIZE as u32) * 8 + bit_in_byte;
	}
	pub fn unpack_index(index: u32) -> (u32, u32, u8) {
		let byte_index = index / 8;
		let bit_in_byte = index % 8;

		let bitmap_block = byte_index / BLOCK_SIZE as u32;
		let byte_in_block = byte_index % BLOCK_SIZE as u32;

		(bitmap_block, byte_in_block, bit_in_byte as u8)
	}


	pub fn find_free<D: BlockDevice>(&self, device: &mut D, num: u32) -> io::Result<Vec<u32>> {
		//! return a vector containing the indices of free blocks, which length is up to 'num'.
		//! it might return fewer elements if there are not enough free blocks.
		if num == 0 { return Ok(vec![]) }

		let mut res: Vec<u32> = vec![];
		let num = num as usize;
		res.reserve(num);
		let mut buf = [0u8; BLOCK_SIZE];
		
		for i in 0..self.size {
			let bitmap_addr = i + self.start;
			device.read_block(bitmap_addr, &mut buf)?;
			
			for j in 0..BLOCK_SIZE {
				let mut el = buf[j];
				if el == u8::MAX { continue; }
				
				let offset = Self::get_index(i, j as u32, 0);
				for pos in 0..8u32 {
					if el & 1 == 0 {
						let index = offset + pos;
						if index >= self.max_index { return Ok(res); }
						res.push(index);
						if res.len() == num { return Ok(res); }
					}
					el >>= 1;
				}
			}
		}

		Ok(res)
	}
	
	pub fn allocate<D: BlockDevice>(&mut self, device: &mut D, blocks: &Vec<u32>) -> io::Result<()> {
		//! mark the provided indices as allocated.
		//! for optimal performance, keep close together the indices that are located in the same bitmap block.
		//! ideally just pass the result of find_free().
		self.modify(device, blocks, |byte, bit| *byte |= 1 << bit)

	}

	pub fn deallocate<D: BlockDevice>(&mut self, device: &mut D, blocks: &Vec<u32>) -> io::Result<()> {
		//! mark the provided indices as free.
		//! for optimal performance, keep close together the indices that are located in the same bitmap block.
		self.modify(device, blocks, |byte, bit| *byte &= !(1 << bit))
	}

	pub fn find_allocate<D: BlockDevice>(&mut self, device: &mut D) -> io::Result<u32> {
		//! finds one free block and allocates it.
		//! returns the index of the allocated block RELATIVE to the data region and not greater than the data region size.
		//! this is more efficient than allocate(find_free(1)), since it doesn't have to re-read the block with the free bit.

		let mut buf = [0u8; BLOCK_SIZE];
		
		// scan the device from start to start + size for a zero bit
		for i in 0..self.size {
			let bitmap_addr = i + self.start;
			device.read_block(bitmap_addr, &mut buf)?;
			
			// read the buffer until the first zero bit, return it
			for j in 0..BLOCK_SIZE {
				if buf[j] == u8::MAX { continue; }

				let pos = buf[j].trailing_ones();
				let index = (j as u32 + i * BLOCK_SIZE as u32) * 8 + pos as u32;
				if index >= self.max_index {
					return Err(io::Error::new(io::ErrorKind::StorageFull, Self::ERR_MAPPED_REGION_FULL));
				}

				buf[j] |= 1 << pos;
				device.write_block(bitmap_addr, &buf)?;
				// j * 8 + i * BLOCK_SIZE * 8 + pos
				return Ok(index);
			}
		}

		Err(io::Error::new(io::ErrorKind::StorageFull, Self::ERR_BITMAP_FULL))
	}



	fn modify<D: BlockDevice, F: Fn(&mut u8, u8)>(&mut self, device: &mut D, blocks: &[u32], op: F) -> io::Result<()> {
		if blocks.is_empty() { return Ok(()) }

		let mut buf = [0u8; BLOCK_SIZE];
		let mut cached_block = INVALID_ADDRESS;

		for &b in blocks {
			let (bitmap_block, byte_in_block, bit_in_byte) = Self::unpack_index(b);

			if bitmap_block != cached_block {
				if cached_block != INVALID_ADDRESS {
					device.write_block(cached_block + self.start, &buf)?;
				}
				cached_block = bitmap_block;
				device.read_block(bitmap_block + self.start, &mut buf)?;
			}

			op(&mut buf[byte_in_block as usize], bit_in_byte);
		}

		if cached_block != INVALID_ADDRESS {
			device.write_block(cached_block + self.start, &buf)?;
		}

		Ok(())
	}
}