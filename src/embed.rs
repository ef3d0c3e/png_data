use std::fmt::Formatter;
use std::str::FromStr;

use bitvec::prelude::*;
use bitvec::vec::BitVec;

use crate::block::BlockMode;
use crate::image::ImageInfo;

pub enum EmbedAlgorithm {
	Lo(u8),
}

impl EmbedAlgorithm {
	pub fn max_size(&self, blockmode: &BlockMode, info: &Box<dyn ImageInfo>) -> usize {
		let blocks_num = info.size() / blockmode.len;

		match self {
			EmbedAlgorithm::Lo(bits) => {
				(((blockmode.len - blockmode.crc_len)*blocks_num) as f64 * (*bits as f64) / 8f64).floor() as usize
			}
		}
	}

	pub fn next_block(&self, original_data: &mut [u8], embed_data: &BitVec<u8>, mut embed_offset: usize, blockmode: &BlockMode) -> usize {
		match self {
			EmbedAlgorithm::Lo(bits) => {
				let mask = (1<<bits) -1;

				fn bits_to_byte(slice: &BitSlice<u8>, bits: u8) -> u8
				{
					let mut result : u8 = 0;
					for i in 0..bits
					{
						result |= (slice[i as usize] as u8) << i;
					}
					result
				}

				for i in 0..(blockmode.len-blockmode.crc_len)
				{
					let hi = std::cmp::min(embed_offset+*bits as usize, embed_data.len())-embed_offset;
					let embed = bits_to_byte(embed_data.get(embed_offset..embed_offset+hi).unwrap(), hi as u8);

					original_data[i] &= !mask;
					original_data[i] |= embed;
					
					embed_offset += hi;
				}
			}
		}
		
		embed_offset
	}

	pub fn read_block(&self, encoded_data: &[u8], incoming: &mut BitVec<u8>, blockmode: &BlockMode) {
		match self {
			EmbedAlgorithm::Lo(bits) => {
				fn push(vec: &mut BitVec<u8>, bits: u8, b: u8)
				{
					for i in 0..bits
					{
						vec.push((b >> i) & 0b1 == 0b1)
					}
				}

				let mut i = 0;
				let start = incoming.len();
				while incoming.len()-start < (blockmode.len-blockmode.crc_len)*8
				{
					push(incoming, *bits, encoded_data[i]);
					i += 1;
				}

				// TODO: Read CRC and verify
			}
		}
	}
}

impl core::fmt::Display for EmbedAlgorithm {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			EmbedAlgorithm::Lo(bits) => write!(f, "Lo({bits})"),
		}
	}
}

impl FromStr for EmbedAlgorithm {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let (dig_pos, _) = s
			.char_indices()
			.find(|(_, c)| c.is_ascii_digit())
			.ok_or(format!("Unknown algorithm: {s}"))?;

		let (first, second) = s.split_at(dig_pos);
		match first {
			"lo" => {
				let value = second.parse::<u8>().map_err(|err| {
					format!("Failed to convert `{second}` to a number of bits: {err}")
				})?;
				// TODO: We can allow more than 8 bits, depending on the image's bit depth
				if value > 8 || value == 0 {
					Err(format!(
						"Cannot specify {value} bits for `lo` method, must be within [1, 8]"
					))
				} else {
					Ok(EmbedAlgorithm::Lo(value))
				}
			}
			_ => Err(format!("Unknown algorithm: {s}")),
		}
	}
}
