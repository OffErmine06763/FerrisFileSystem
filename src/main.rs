#![allow(dead_code, unused_imports)]

mod ffs;
mod formats;
mod fs_utils;
mod device;
mod file;
mod fs_error;

use fs_utils::*;
use fs_error::*;
use ffs::FFS;
use device::block_device::{self, BlockDevice};
use device::file_device::FileDevice;
use device::memory_device::MemoryDevice;
use device::cached_device::CachedDevice;
use formats::format::*;
use file::{File, FileType};
use formats::v1::format::FormatV1;

use std::io::{self};
use std::fs;
use std::path::{Path, PathBuf};


fn main() -> FSResult<()> {
	let path = "../../disks/disk1.img";
	let device_size = 100;
	let cache_size = 50;

	let mut device = CachedDevice::new(MemoryDevice::empty(device_size), cache_size);
	FormatV1::format(&mut device)?;
	let mut ffs = FFS::mount(device)?;
	

	ffs.create_file("name.txt")?;
	ffs.create_directory("fold")?;
	ffs.create_file("fold/inner.txt")?;
	ffs.create_hardlink("name.txt", "fold/name_inner.txt")?;

	
	print_dir("./fold", &mut ffs)?;

	let file = ffs.open_file("fold/inner.txt")?;
	file_io(&file, &mut ffs)?;

	
	ffs.save(path)?;

	let ok = ffs.check_integrity()?;


	if ok.is_ok() {
		println!("TS WORKS!");
	}
	else {
		println!(":(");
	}



	Ok(())
}



fn file_io<D: BlockDevice>(file: &File, ffs: &mut FFS<D>) -> FSResult<()> {
	let text = b"banana";
	let mut buf = [0u8; 6];

	ffs.write(&file, text.as_slice())?;
	ffs.write(&file, text.as_slice())?;

	ffs.seek(&file, io::SeekFrom::Current(-6))?;
	ffs.read(&file, buf.as_mut_slice())?;

	for c in buf {
		print!("{}", c as char); // banana
	}
	println!();

	ffs.seek(&file, io::SeekFrom::Start(2))?;
	ffs.read(&file, buf.as_mut_slice())?;

	for c in buf {
		print!("{}", c as char); // nanaba
	}
	println!();

	ffs.seek(&file, io::SeekFrom::End(-8))?;
	ffs.read(&file, buf.as_mut_slice())?;

	for c in buf {
		print!("{}", c as char); // nabana
	}
	println!();

	Ok(())
}



fn print_dir_from_path<D: BlockDevice>(dir_path: &Path, ffs: &mut FFS<D>) -> FSResult<()> {
	let content = ffs.get_directory_content(dir_path.to_str().unwrap())?;

	println!("{}", dir_path.display());
	print_dir_recursive(&content, 0, ffs, dir_path.to_path_buf())?;

	Ok(())
}
fn print_dir<D: BlockDevice>(dir_str: &str, ffs: &mut FFS<D>) -> FSResult<()> {
	print_dir_from_path(&Path::new(dir_str), ffs)
}

fn print_dir_recursive<D: BlockDevice>(content: &DirectoryContentResult, indent: u32, ffs: &mut FFS<D>, parent_path: PathBuf) -> FSResult<()> {
	let mut count = 0;
	for e in &content.entries {
		print!("{}", " ".repeat(indent as usize));
		println!("|- {:<20} {}", e.filename, e.file_type);
		if count >= 2 && e.file_type == FileType::Directory {
			let child_path = parent_path.join(Path::new(&e.filename));
			let inner = ffs.get_directory_content(&child_path.to_str().unwrap())?;
			print_dir_recursive(&inner, indent + 3, ffs, child_path)?;
		}
		count += 1;
	}

	Ok(())
}