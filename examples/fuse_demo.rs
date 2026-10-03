#[cfg(feature = "fuse")]
use fuser::{mount, MountOption, Config};
#[cfg(feature = "fuse")]
use FerrisFileSystem::{FSResult, FormatV1, MemoryDevice, FuseFFS};


#[cfg(feature = "fuse")]
fn main() -> FSResult<()>  {
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
	mount(fs, "/tmp/ffs", &config)?;

	Ok(())
}

#[cfg(not(feature = "fuse"))]
fn main() {
	print!("Error: FUSE Feature Disabled!!")
}