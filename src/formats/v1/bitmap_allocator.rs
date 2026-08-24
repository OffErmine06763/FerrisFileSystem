use crate::fs_utils::*;
use crate::block_device::BlockDevice;
use crate::fs_error::*;

use std::io;


pub struct BitmapAllocator {
	/// first block where the bitmap resides
	start: u32,
	/// number of blocks dedicated to the bitmap
	size: u32,
	/// 1 past the highest value that may be returned by allocate
	/// (the bitmap itself might be bigger than the region it maps to)
	max_index: u32,
	/// index of the last allocation, future allocations will start looking from here
	last_alloc: u32,
}

impl BitmapAllocator {

	pub fn new(start: u32, size: u32, max_index: u32) -> Self {
		Self { start, size, max_index, last_alloc: 0 }
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



	pub fn is_allocated<D: BlockDevice>(&self, device: &mut D, block: u32) -> FSResult<bool> {
		if block > self.max_index {
			return Ok(false);
		}

		let (bitmap_block, byte, bit) = Self::unpack_index(block);
		let mut buf = [0u8; BLOCK_SIZE];
		device.read_block(self.start + bitmap_block, &mut buf)?;
		let byte = buf[byte as usize];
		
		Ok(byte & (1 << bit) != 0)
	}



	pub fn find_free<D: BlockDevice>(&mut self, device: &mut D, num: u32) -> FSResult<Vec<u32>> {
		//! return a vector containing the indices of free blocks, which length is up to 'num'.
		//! it might return fewer elements if there are not enough free blocks.
		if num == 0 { return Ok(vec![]) }

		let mut res: Vec<u32> = vec![];
		let num = num as usize;
		res.reserve(num);
		
		let mut buf = [0u8; BLOCK_SIZE];

		let (start_block, start_byte, _start_bit) = Self::unpack_index(self.last_alloc);
		
		'outer:
		for i in start_block..self.size {
			let bitmap_addr = i + self.start;
			device.read_block(bitmap_addr, &mut buf)?;
			
			let start = if i == start_block { start_byte } else { 0 };
			let start = start as usize;

			for j in start..BLOCK_SIZE {
				let mut el = buf[j];
				if el == u8::MAX { continue; }
				
				let offset = Self::get_index(i, j as u32, 0);
				for pos in 0..8u32 {
					let index = offset + pos;
					if index >= self.max_index { break 'outer; }

					if el & 1 == 0 {
						res.push(index);
						if res.len() == num { return Ok(res); }
					}
					el >>= 1;
				}
			}
		}

		// if we couldn't find enough blocks, restart
		'outer:
		for i in 0..start_block {
			let bitmap_addr = i + self.start;
			device.read_block(bitmap_addr, &mut buf)?;
			
			let end = if i == start_block { start_byte as usize } else { BLOCK_SIZE };

			for j in 0..end {
				let mut el = buf[j];
				if el == u8::MAX { continue; }
				
				let offset = Self::get_index(i, j as u32, 0);
				for pos in 0..8u32 {
					let index = offset + pos;
					if index >= self.max_index { break 'outer; }

					if el & 1 == 0 {
						res.push(index);
						if res.len() == num { return Ok(res); }
					}
					el >>= 1;
				}
			}
		}

		Ok(res)
	}
	


	pub fn allocate<D: BlockDevice>(&mut self, device: &mut D, blocks: &Vec<u32>) -> FSResult<()> {
		//! mark the provided indices as allocated.
		//! for optimal performance, keep close together the indices that are located in the same bitmap block.
		//! ideally just pass the result of find_free().
		//! note: does NOT check that they are not allocated already.
		if blocks.len() == 0 { return Ok(()) }

		self.modify(device, blocks, |byte, bit| *byte |= 1 << bit)?;
		self.last_alloc = blocks[blocks.len() - 1];
		
		Ok(())
	}
	pub fn deallocate<D: BlockDevice>(&mut self, device: &mut D, blocks: &Vec<u32>) -> FSResult<()> {
		//! mark the provided indices as free.
		//! for optimal performance, keep close together the indices that are located in the same bitmap block.
		//! note: does NOT check that they are not free already.

		self.modify(device, blocks, |byte, bit| *byte &= !(1 << bit))?;
		
		Ok(())
	}



	pub fn find_allocate<D: BlockDevice>(&mut self, device: &mut D) -> FSResult<u32> {
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
					return Err(FSError::StorageFull(StorageFullKind::None)); // Self::ERR_MAPPED_REGION_FULL
				}

				buf[j] |= 1 << pos;
				device.write_block(bitmap_addr, &buf)?;

				self.last_alloc = index;
				
				return Ok(index);
			}
		}

		Err(FSError::StorageFull(StorageFullKind::None)) // Self::ERR_BITMAP_FULL
	}



	fn modify<D: BlockDevice, F: Fn(&mut u8, u8)>(&mut self, device: &mut D, blocks: &[u32], op: F) -> FSResult<()> {
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



	pub fn max_index(&self) -> u32 {
		self.max_index
	}



	pub fn count_allocated<D: BlockDevice>(&self, device: &mut D) -> FSResult<u32> {
		//! count the number of allocated blocks.
		//! NOTE: very expensive operation, prefer using the cached value in superblock when possible

		let mut count = 0;
		let mut buf = [0u8; BLOCK_SIZE];

		'outer:
		for i in 0..self.size {
			let bitmap_addr = i + self.start;
			device.read_block(bitmap_addr, &mut buf)?;
			
			for j in 0..BLOCK_SIZE {
				let mut el = buf[j];
				if el == 0 { continue; }
				
				let offset = Self::get_index(i, j as u32, 0);
				for pos in 0..8u32 {
					let index = offset + pos;
					if index >= self.max_index { break 'outer; }

					if el & 1 != 0 {
						count += 1;
					}
					el >>= 1;
				}
			}
		}

		Ok(count)
	}
}










#[test]
fn bitmap_allocator() -> FSResult<()> {
	use crate::device::memory_device::*;

	// 2 blocks for the bitmap, starting at 6th block, that excludes the last 10 bits
	let mut allocator = BitmapAllocator::new(5, 2, BLOCK_SIZE as u32 * 2 * 8 - 10);
	let mut device = MemoryDevice::empty(10);
	let mut buf = [0u8; BLOCK_SIZE];
	let mut expected = [0u8; BLOCK_SIZE];
	
	// manually allocate bit 50 in both bitmap blocks and others in the first.
	let mut blocks = vec![50u32, BLOCK_SIZE as u32 * 8 + 50, 5, 2, 60, 3];
	allocator.allocate(&mut device, &blocks)?;

	device.read_block(5, &mut buf)?;
	expected[50 / 8] |= 1 << (50 % 8); expected[5 / 8] |= 1 << (5 % 8); expected[2 / 8] |= 1 << (2 % 8); expected[60 / 8] |= 1 << (60 % 8); expected[3 / 8] |= 1 << (3 % 8);
	assert_eq!(buf, expected);
	
	device.read_block(6, &mut buf)?;
	expected = [0u8; BLOCK_SIZE];
	expected[50 / 8] = 1 << (50 % 8);
	assert_eq!(buf, expected);

	// find and allocate 5 blocks, note that there is no guarantee on where they will be allocated
	// so we can only count the number of 1s in the bitmap.
	let expected = blocks.len() + 5;
	blocks = allocator.find_free(&mut device, 5)?;
	assert_eq!(blocks.len(), 5);
	allocator.allocate(&mut device, &blocks)?;

	let mut actual = 0;
	device.read_block(5, &mut buf)?;
	for i in 0..BLOCK_SIZE {
		for j in 0..8 {
			if (buf[i] & (1 << j)) != 0 { actual += 1; }
		}
	}
	device.read_block(6, &mut buf)?;
	for i in 0..BLOCK_SIZE {
		for j in 0..8 {
			if i * 8 + j + BLOCK_SIZE * 8 >= BLOCK_SIZE * 2 * 8 - 10 {
				assert!(buf[i] & (1 << j) == 0);
			}
			else {
				if (buf[i] & (1 << j)) != 0 { actual += 1; }
			}
		}
	}
	
	assert_eq!(actual, expected);


	// allocate one block 
	let allocated = allocator.find_allocate(&mut device)?;
	assert!(allocated < BLOCK_SIZE as u32 * 2 - 10);
	if allocated < BLOCK_SIZE as u32 { device.read_block(5, &mut buf)?; }
	else { device.read_block(6, &mut buf)?; }
	assert_eq!(buf[allocated as usize / 8], 1 << (allocated % 8));

	// TODO: as of now it doesn't check whether the address is in valid range
	//       also it doen't check whether it isn't allocated already.
	// try to allocate past the end
	//let res = allocator.allocate(&mut device, &vec![BLOCK_SIZE as u32 * 2 * 8 - 10]).unwrap_err();
	//assert_eq!(res.kind(), io::ErrorKind::InvalidInput);
	//assert_eq!(res.to_string(), "block out of range");
	//let res = allocator.allocate(&mut device, &vec![BLOCK_SIZE as u32 * 2 * 8 -  5]).unwrap_err();
	//assert_eq!(res.kind(), io::ErrorKind::InvalidInput);
	//assert_eq!(res.to_string(), "block out of range");
	//let res = allocator.allocate(&mut device, &vec![BLOCK_SIZE as u32 * 2 * 8 + 10]).unwrap_err();
	//assert_eq!(res.kind(), io::ErrorKind::InvalidInput);
	//assert_eq!(res.to_string(), "block out of range");

	// allocate all the available blocks
	blocks = allocator.find_free(&mut device, BLOCK_SIZE as u32 * 2 * 8)?;
	// number of bits - allocated with find_free - allocated with find_allocate - 10 (invalid bits at the end)
	assert_eq!(blocks.len(), BLOCK_SIZE * 2 * 8 - actual - 1 - 10);
	allocator.allocate(&mut device, &blocks)?;

	// allocate in the full bitmap
	let res = allocator.find_allocate(&mut device).unwrap_err();
	assert_eq!(res.code(), FSErrorCode::StorageFull);

	// find free in the full bitmap
	blocks = allocator.find_free(&mut device, 5)?;
	assert_eq!(blocks.len(), 0);

	// deallocate one block
	allocator.deallocate(&mut device, &vec![0])?;
	device.read_block(5, &mut buf)?;
	assert_eq!(buf[0], 0b11111110);


	// allocate in a full bitmap where all bits are valid addresses
	let mut allocator = BitmapAllocator::new(5, 1, BLOCK_SIZE as u32 * 8);
	let mut device = MemoryDevice::empty(10);

	blocks = allocator.find_free(&mut device, BLOCK_SIZE as u32 * 8)?;
	allocator.allocate(&mut device, &blocks)?;

	let res = allocator.find_allocate(&mut device).unwrap_err();
	assert_eq!(res.code(), FSErrorCode::StorageFull);


	Ok(())
}