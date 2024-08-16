use bitvec::slice::BitSlice;

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
		let comment_len = self.comment.as_ref().map(|c| c.len() as u16).unwrap_or(0 as u16);
		header.extend_from_slice(comment_len.to_le_bytes().as_slice());

		// Comment
		if let Some(comment) = &self.comment {
			header.extend_from_slice(comment.as_bytes());
		}

		header
	}

	pub fn from_data(slice: &BitSlice<u8>) -> (u16, BlockMode, u32, u16) {
		fn read_byte(slice: &bitvec::slice::BitSlice<u8>) -> u8
		{
			let mut result = 0;
			for i in 0..8
			{
				result |= (slice[i as usize] as u8) << i;
			}
			result
		}

		let version = ((read_byte(&slice[8..16]) as u16) << 8) | (read_byte(&slice[0..8]) as u16);
		let blockmode = BlockMode::from_byte(read_byte(&slice[16..24]));
		let len = ((read_byte(&slice[48..56]) as u32) << 24)
				| ((read_byte(&slice[40..48]) as u32) << 16)
				| ((read_byte(&slice[32..40]) as u32) << 8)
				| (read_byte(&slice[24..32]) as u32);
		let comment_len = ((read_byte(&slice[64..72]) as u16) << 8) | (read_byte(&slice[56..64]) as u16);


		(version, blockmode, len, comment_len)
	}
}
