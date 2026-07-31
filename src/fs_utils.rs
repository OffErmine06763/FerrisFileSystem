pub const MAGIC: u64 = 0x2E2E616E616E6162;
pub const BLOCK_SIZE: usize = 256;


pub enum Version {
	V1,
}

pub fn read_version(buf: &[u8; BLOCK_SIZE]) -> Version {
	let version_bytes = &buf[8..12];
	let version = u32::from_le_bytes(version_bytes.try_into().unwrap());

	match version {
		1 => Version::V1,
		_ => panic!("unsupported filesystem version"),
	}
}