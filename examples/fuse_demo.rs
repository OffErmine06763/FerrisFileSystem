#[cfg(feature = "fuse")]
use fuser::{mount, MountOption, Config};
#[cfg(feature = "fuse")]
use FerrisFileSystem::{FSResult, FormatV1, MemoryDevice, FuseFFS};
#[cfg(feature = "fuse")]
use std::env;


#[cfg(feature = "fuse")]
fn main() -> FSResult<()>  {
	let args: Vec<String> = env::args().collect();
	if args.len() == 0 {
		println!("Provide the mount point as first command line argument");
		return Ok(());
	}

	// Initialize temporary device
	let device_size = 100;
	let mut device = MemoryDevice::empty(device_size);
	FormatV1::format(&mut device)?;

	// Mount the device
	let fs = FuseFFS::mount(device)?;

	// Set the mount options
	let mut config = Config::default();
	config.mount_options = vec![
			MountOption::FSName("ferrisfs".to_string()),
			MountOption::AutoUnmount,
		];
	config.acl = fuser::SessionACL::RootAndOwner;

	// Mount (blocking call)
	mount(fs, args[1].as_str(), &config)?;

	Ok(())
}

#[cfg(not(feature = "fuse"))]
fn main() {
	print!("Error: FUSE Feature Disabled!!")
}