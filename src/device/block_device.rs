use crate::fs_utils::*;
use crate::fs_error::*;
use std::io;

/// Abstracts where the physical storage is (RAM, file).
/// It is a generic collection of blocks, exposing the operations on them
pub trait BlockDevice {
	fn read_block(&mut self, block: u32, buf: &mut [u8; BLOCK_SIZE]) -> FSResult<()>;
	fn write_block(&mut self, block: u32, buf: &[u8; BLOCK_SIZE]) -> FSResult<()>;
	fn block_count(&self) -> u32;
	fn resize(&mut self, blocks: u32) -> FSResult<()>;
	fn flush(&mut self) -> FSResult<()>;
}