use crate::file::{FileType};


pub struct DirectoryContentEntry {
	pub filename: String,
	pub file_type: FileType,
}

pub struct DirectoryContentResult {
	pub entries: Vec<DirectoryContentEntry>,
	// add maybe stuff as total size (including the directory used space)...
}


pub trait IntegrityError {
	// todo: i dont like the idea of "is_recoverable"
	//       everything is, sometimes it's trivial, other times it requires taking a decision
	//       maybe change this to "is_usable" for when the state is inconsistent but still works
	fn is_recoverable(&self) -> bool;
	fn to_string(&self) -> String;
}

pub struct IntegrityResult {
	pub errors: Vec<Box<dyn IntegrityError>>,
}

impl IntegrityResult {
	pub fn is_ok(&self) -> bool {
		self.errors.len() == 0
	}

	pub fn is_recoverable(&self) -> bool {
		for e in &self.errors {
			if !e.is_recoverable() {
				return false;
			}
		}

		return true;
	}
}