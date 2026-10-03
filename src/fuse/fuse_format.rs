use crate::{BlockDevice, FormatV1, FSResult};



/// Format compatible with FUSE.
/// Requires the format operations to expose more implementation-specific details
/// to map with the data required by fuse (not all formats might be fuse compatible)
pub trait FuseCompatibleFormat<D: BlockDevice> {
	fn do_sth(&mut self, device: &mut D, path: &str) -> FSResult<()>;
}

impl<D: BlockDevice> FuseCompatibleFormat<D> for FormatV1 {
	fn do_sth(&mut self, device: &mut D, path: &str) -> FSResult<()> {
		print!("Banana\n");
		Ok(())
	}
}