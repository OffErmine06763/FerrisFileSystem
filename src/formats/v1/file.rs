use super::super::file::FileIO;

use std::io::{self, SeekFrom, Seek, Read, Write};


pub struct File {
    inode: u32,
    offset: u64,
    // ...
}


impl File {
    pub fn new(inode: u32) -> Self {
        File { inode, offset: 0 }
    }
}


impl FileIO for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        todo!()
    }

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        todo!()
    }

    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        todo!()
    }
}