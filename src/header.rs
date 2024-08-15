use crate::block::BlockMode;

pub struct Header {
	pub blockmode: BlockMode,
	pub comment: Option<String>,
}

impl Header {
	pub fn to_data(&self, version: u16, embed_len: u32) -> Vec<u8> {
		let mut header = vec![];

		// Version
		header.extend_from_slice(version.to_le_bytes().as_slice());

		// TODO: IV+Cipherinfo
		// Blockmode
		header.push(self.blockmode.to_data().to_le());

		// Data len
		header.extend_from_slice(embed_len.to_le_bytes().as_slice());

		// Comment len
		let comment_len = self.comment.as_ref().map(|c| c.len()).unwrap_or(0);
		header.extend_from_slice(comment_len.to_le_bytes().as_slice());

		// Comment
		if let Some(comment) = &self.comment {
			header.extend_from_slice(comment.as_bytes());
		}

		header
	}
}
