use crate::fs_utils::*;
use crate::fs_error::*;
use super::block_device::BlockDevice;
use super::memory_device::MemoryDevice;

use std::collections::HashMap;
use std::io;
use std::path::Path;


struct CachedBlock {
	data: [u8; BLOCK_SIZE],
	dirty: bool,

	/// forces this block to stay in cache [NOT IMPLEMENTED]
	pinned: bool,

	/// address of the block accessed right after this one
	more_recent: u32,
	/// address of the block accessed right before this one
	less_recent: u32,
}



pub struct CachedDevice<D: BlockDevice> {
	device: D,

	/// Number of cached blocks. Must be greater than 1 (guarantees that on replacement lru != mru)
	size: usize,
	cache: HashMap<u32, CachedBlock>,
	cached_count: usize,

	/// least recently used block address
	lru: u32,
	/// most  recently used block address
	mru: u32,
}



impl<D: BlockDevice> BlockDevice for CachedDevice<D> {
	fn read_block(&mut self, block: u32, buf: &mut [u8; BLOCK_SIZE]) -> FSResult<()> {
		let cached = self.cache.get(&block);
		if cached.is_some() {
			// to fight the borrow checker we first have to get the entry as immutable
			// and copy the relevant fields
			let entry = cached.unwrap();
			let entry_more_recent = entry.more_recent;	
			let entry_less_recent = entry.less_recent;

			// get entry.more_recent and set its less_recent to entry.less_recent (and vv)
			if entry_more_recent != INVALID_ADDRESS {
				let more_recent = self.cache.get_mut(&entry_more_recent).unwrap();
				more_recent.less_recent = entry_less_recent;
			
				// update the entry.less_recent.more_recent only if entry isn't already the mru
				// otherwise it gets updated to INVALID_ADDRESS
				if entry_less_recent != INVALID_ADDRESS {
					let less_recent = self.cache.get_mut(&entry_less_recent).unwrap();
					less_recent.more_recent = entry_more_recent;
				}
			}

			// if this is the lru, update the lru to entry.more_recent, unless it is INVALID_ADDRESS
			// (which happens when this is the only entry in the cache, in which case this remain the lru)
			if self.lru == block && entry_more_recent != INVALID_ADDRESS {
				self.lru = entry_more_recent;
			}

			// get the mru and update its more_recent to block
			{
				let mru = self.cache.get_mut(&self.mru).unwrap();
				mru.more_recent = block;
			}

			// now we can re-borrow the entry as mutable and update it
			let entry = self.cache.get_mut(&block).unwrap();
			entry.more_recent = INVALID_ADDRESS;
			entry.less_recent = self.mru;
			self.mru = block;

			// finally read the entry data
			buf.copy_from_slice(entry.data.as_slice());
		}
		else {
			self.device.read_block(block, buf)?;
			// this works only with SIZE > 1, otherwise self.mru is !INVALID_ADDRESS but it will be removed from the cache
			let entry = CachedBlock { data: buf.clone(), dirty: false, pinned: false, more_recent: INVALID_ADDRESS, less_recent: self.mru };

			self.miss_update(block);
			self.insert(block, entry)?;
		}

		Ok(())
	}


	fn write_block(&mut self, block: u32, buf: &[u8; BLOCK_SIZE]) -> FSResult<()> {
		let cached = self.cache.get(&block);
		if cached.is_some() {
			// to fight the borrow checker we first have to get the entry as immutable
			// and copy the relevant fields
			let entry = cached.unwrap();
			let entry_more_recent = entry.more_recent;	
			let entry_less_recent = entry.less_recent;

			// get entry.more_recent and set its less_recent to entry.less_recent (and vv)
			if entry_more_recent != INVALID_ADDRESS {
				let more_recent = self.cache.get_mut(&entry_more_recent).unwrap();
				more_recent.less_recent = entry_less_recent;

				// update the entry.less_recent.more_recent only if entry isn't already the mru
				// otherwise it gets updated to INVALID_ADDRESS
				if entry_less_recent != INVALID_ADDRESS {
					let less_recent = self.cache.get_mut(&entry_less_recent).unwrap();
					less_recent.more_recent = entry_more_recent;
				}
			}

			// if this is the lru, update the lru to entry.more_recent, unless it is INVALID_ADDRESS
			// (which happens when this is the only entry in the cache, in which case this remain the lru)
			if self.lru == block && entry_more_recent != INVALID_ADDRESS {
				self.lru = entry_more_recent;
			}

			// get the mru and update its more_recent to block
			{
				let mru = self.cache.get_mut(&self.mru).unwrap();
				mru.more_recent = block;
			}

			// now we can re-borrow the entry as mutable and update it
			let entry = self.cache.get_mut(&block).unwrap();
			entry.more_recent = INVALID_ADDRESS;
			entry.less_recent = self.mru;
			entry.dirty = true;
			self.mru = block;
			
			// finally write the entry data
			entry.data.copy_from_slice(buf.as_slice());
		}
		else {
			// this works only with SIZE > 1, otherwise self.mru is !INVALID_ADDRESS but it will be removed from the cache
			let entry = CachedBlock { data: buf.clone(), dirty: true, pinned: false, more_recent: INVALID_ADDRESS, less_recent: self.mru };
			
			self.miss_update(block);
			self.insert(block, entry)?;
		}

		Ok(())
	}


	fn block_count(&self) -> u32 {
		self.device.block_count()
	}
	fn resize(&mut self, blocks: u32) -> FSResult<()> {
		self.device.resize(blocks)
	}


	fn flush(&mut self) -> FSResult<()> {
		//! writes back the modified blocks.
		//! automatically called on destruction.
		
		for (key, value) in &mut self.cache {
			if value.dirty {
				self.device.write_block(*key, &value.data)?;
				value.dirty = false;
			}
		}

		Ok(())
	}
}



impl<D: BlockDevice> CachedDevice<D> {
	pub fn new(device: D, size: usize) -> CachedDevice<D> {
		let size = size.max(2);
		Self { device, size, cache: HashMap::with_capacity(size), cached_count: 0, mru: INVALID_ADDRESS, lru: INVALID_ADDRESS }
	}

	fn insert(&mut self, block: u32, entry: CachedBlock) -> FSResult<()> {
		if self.cached_count >= self.size {
			let to_remove = self.lru;

			// write-back the lru and update lru
			{
				let lru = self.cache.get_mut(&self.lru).unwrap();
				if lru.dirty {
					self.device.write_block(self.lru, &lru.data)?;
				}
				self.lru = lru.more_recent;
			}
			// update the new lru
			{
				let lru = self.cache.get_mut(&self.lru).unwrap();
				lru.less_recent = INVALID_ADDRESS;
			}

			self.cache.remove(&to_remove);
			self.cached_count -= 1;

			// even when SIZE == 1 the code above works, because mru == lru so first we update mru.more_recent = block, 
			// and here we update lru to lru.more_recent == mru.more_recent == block, which is what we want when replacing the only cached entry
		}
		
		self.cached_count += 1;
		self.cache.insert(block, entry);

		Ok(())
	}

	fn miss_update(&mut self, block: u32) {
		// get the mru and update its more_recent to block
		if self.mru != INVALID_ADDRESS {
			let mru = self.cache.get_mut(&self.mru).unwrap();
			mru.more_recent = block;
		} else {
			// first insertion in the cache, thus the mru is also the lru
			self.lru = block;
		}
			
		self.mru = block;
	}
}



impl CachedDevice<MemoryDevice> {
	pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
		self.device.save(path)
	}
}



impl<D: BlockDevice> Drop for CachedDevice<D> {
	fn drop(&mut self) {
		let res = self.flush();
		#[cfg(debug_assertions)]
		{
			if res.is_err() {
				println!("Failed to flush cache on cache destruction, error: {}", res.err().unwrap());
			}
		}
	}
}






#[cfg(test)]
mod tests {
	use std::{assert_matches, error::Error};
	use super::*;
	use crate::device::memory_device::MemoryDevice;

	// Test Cases - Read Sequences:
	// 1 - sequence of only misses
	// 2 - miss(1) miss(2) miss(3 -> 1) miss(1 -> 2) hit(3) miss(2 -> 1)
	// 3 - miss(1) miss(2) hit(1) miss(3 -> 2) hit(1) miss(2 -> 3)
	// 4 - miss(1) hit(1) hit(1) miss(2) hit(1) hit(2) miss(3 -> 1)

	// Test Cases - Write Sequences:
	// 1 - miss(1) miss(1) miss(2) miss(3 -> 1)


	struct FakeDevice {
		locked: bool,
		read: u32,
		wrote: u32,
	}

	impl BlockDevice for FakeDevice {
		fn read_block(&mut self, block: u32, buf: &mut [u8; BLOCK_SIZE]) -> FSResult<()> {
			if self.locked {
				assert!(false, "attempted reading block {}", block)
			}
			self.read += 1;
			Ok(())
		}
		fn write_block(&mut self, block: u32, buf: &[u8; BLOCK_SIZE]) -> FSResult<()> {
			if self.locked {
				assert!(false, "attempted writing block {}", block)
			}
			self.wrote += 1;
			Ok(())
		}

		fn block_count(&self) -> u32 { 0 }
		fn resize(&mut self, _blocks: u32) -> FSResult<()> { Ok(()) }
		fn flush(&mut self) -> FSResult<()> { Ok(()) }
	}

	impl FakeDevice {
		pub fn new() -> Self {
			Self { locked: false, read: 0, wrote: 0 }
		}

		pub fn lock(&mut self) {
			self.locked = true;
		}
		pub fn unlock(&mut self) {
			self.locked = false;
		}
		pub fn read(&mut self) -> u32 {
			let res = self.read;
			self.read = 0;
			res
		}
		pub fn wrote(&mut self) -> u32 {
			let res = self.wrote;
			self.wrote = 0;
			res
		}
	}

	impl CachedDevice<FakeDevice> {
		pub fn lock(&mut self) {
			self.device.lock();
		}
		pub fn unlock(&mut self) {
			self.device.unlock();
		}
		pub fn read(&mut self) -> u32 {
			self.device.read()
		}
		pub fn wrote(&mut self) -> u32 {
			self.device.wrote()
		}
	}



	#[test]
	fn read_sequence1() -> FSResult<()> {
		let cache_size = 4;

		let mut device = CachedDevice::new(FakeDevice::new(), cache_size);
		let mut buf = [0u8; BLOCK_SIZE];
		let buf = &mut buf;

		device.read_block(1, buf)?;
		assert_contains(&device, &[1]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 1);

		device.read_block(2, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 2);

		device.read_block(3, buf)?;
		assert_contains(&device, &[1, 2, 3]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 3);

		device.read_block(4, buf)?;
		assert_contains(&device, &[1, 2, 3, 4]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 4);

		device.read_block(5, buf)?;
		assert_contains(&device, &[5, 2, 3, 4]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 5);

		device.read_block(6, buf)?;
		assert_contains(&device, &[5, 6, 3, 4]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 6);

		device.read_block(7, buf)?;
		assert_contains(&device, &[5, 6, 7, 4]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 7);

		device.read_block(8, buf)?;
		assert_contains(&device, &[5, 6, 7, 8]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 8);

		device.read_block(9, buf)?;
		assert_contains(&device, &[9, 6, 7, 8]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 9);

		Ok(())
	}

	#[test]
	fn read_sequence2() -> FSResult<()> {
		let cache_size = 2;

		let mut device = CachedDevice::new(FakeDevice::new(), cache_size);
		let mut buf = [0u8; BLOCK_SIZE];
		let buf = &mut buf;

		device.read_block(1, buf)?;
		assert_contains(&device, &[1, ]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 1);

		device.read_block(2, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 2);

		device.read_block(3, buf)?;
		assert_contains(&device, &[3, 2]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 3);
		
		device.read_block(1, buf)?;
		assert_contains(&device, &[3, 1]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 1);
		
		device.read_block(3, buf)?;
		assert_contains(&device, &[3, 1]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 3);
		
		device.read_block(2, buf)?;
		assert_contains(&device, &[3, 2]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 2);

		Ok(())
	}

	#[test]
	fn read_sequence3() -> FSResult<()> {
		let cache_size = 2;

		let mut device = CachedDevice::new(FakeDevice::new(), cache_size);
		let mut buf = [0u8; BLOCK_SIZE];
		let buf = &mut buf;

		device.read_block(1, buf)?;
		assert_contains(&device, &[1, ]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 1);
		
		device.read_block(2, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 2);
		
		device.read_block(1, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 1);
		
		device.read_block(3, buf)?;
		assert_contains(&device, &[1, 3]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 3);
		
		device.read_block(1, buf)?;
		assert_contains(&device, &[1, 3]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 1);
		
		device.read_block(2, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 2);

		Ok(())
	}

	#[test]
	fn read_sequence4() -> FSResult<()> {
		let cache_size = 2;

		let mut device = CachedDevice::new(FakeDevice::new(), cache_size);
		let mut buf = [0u8; BLOCK_SIZE];
		let buf = &mut buf;

		device.read_block(1, buf)?;
		assert_contains(&device, &[1, ]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 1);
		
		device.read_block(1, buf)?;
		assert_contains(&device, &[1, ]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 1);
		
		device.read_block(1, buf)?;
		assert_contains(&device, &[1, ]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 1);
		
		device.read_block(2, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 2);
		
		device.read_block(1, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 1);
		
		device.read_block(2, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 2);
		
		device.read_block(3, buf)?;
		assert_contains(&device, &[3, 2]);
		assert_rw_count(&mut device, 1, 0);
		assert_eq!(device.mru, 3);

		Ok(())
	}

	#[test]
	fn write_sequence1() -> FSResult<()> {
		let cache_size = 2;

		let mut device = CachedDevice::new(FakeDevice::new(), cache_size);
		let buf = [0u8; BLOCK_SIZE];
		let buf = &buf;

		device.write_block(1, buf)?;
		assert_contains(&device, &[1, ]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 1);

		device.write_block(1, buf)?;
		assert_contains(&device, &[1, ]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 1);

		device.write_block(2, buf)?;
		assert_contains(&device, &[1, 2]);
		assert_rw_count(&mut device, 0, 0);
		assert_eq!(device.mru, 2);
		
		device.write_block(3, buf)?;
		assert_contains(&device, &[3, 2]);
		assert_rw_count(&mut device, 0, 1);
		assert_eq!(device.mru, 3);

		device.flush()?;
		assert_rw_count(&mut device, 0, 2);

		Ok(())
	}


	// UTILS

	fn assert_contains<D: BlockDevice>(device: &CachedDevice<D>, blocks: &[u32]) {
		for block in blocks {
			assert!(device.cache.contains_key(block));
		}
	}

	fn assert_rw_count(device: &mut CachedDevice<FakeDevice>, read: u32, wrote: u32) {
		assert_eq!(device.read(), read);
		assert_eq!(device.wrote(), wrote);
	}
}