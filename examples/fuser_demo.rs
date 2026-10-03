use fuser::{mount, MountOption, Config};
use FerrisFileSystem::{FormatV1, MemoryDevice, FSResult};


fn main() -> FSResult<()>  {
    let device_size = 100;

    let mut device = MemoryDevice::empty(device_size);
    
	FormatV1::format(&mut device)?;
    let fs = FormatV1::mount(&mut device)?;

    let mut config = Config::default();
    config.mount_options = vec![
            MountOption::FSName("ferrisfs".to_string()),
        ];
    config.acl = fuser::SessionACL::Owner;

    mount(fs, "/tmp/ffs", &config)?;

    Ok(())
}