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
	/// Get the size of the data (in bytes) once embedded by the algorithm
	pub fn embedded_size(&self, size: usize) -> usize {
		match self {
			EmbedAlgorithm::Lo(bits) => {
				((size * 8) as f64 / *bits as f64).ceil() as usize
			}
		}
	}

	pub fn max_size(&self, blockmode: &BlockMode, info: &Box<dyn ImageInfo>) -> usize {
		let blocks_num = info.size() / blockmode.len;

		match self {
			EmbedAlgorithm::Lo(bits) => {
				(((blockmode.len - blockmode.crc_len)*blocks_num) as f64 * (*bits as f64) / 8f64).floor() as usize
			}
		}
	}

	pub fn next_block(&self, original_data: &mut [u8], mut data_pos: usize, embed_data: &BitVec<u8>, mut embed_offset: usize, blockmode: &BlockMode) -> (usize, usize) {
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

				let start = embed_offset;
				while embed_offset-start < (blockmode.len-blockmode.crc_len)*8
				{
					let hi = std::cmp::min(*bits as usize, embed_data.len() - embed_offset);
					let embed = bits_to_byte(embed_data.get(embed_offset..embed_offset+hi).unwrap(), hi as u8);

					original_data[data_pos] &= !mask;
					original_data[data_pos] |= embed;
					
					data_pos += 1;
					embed_offset += hi;
				}

				// TODO: WRITE CRC
			}
		}
		
		(data_pos, embed_offset)
	}

	pub fn read_block(&self, encoded_data: &[u8], mut data_pos: usize, incoming: &mut BitVec<u8>, blockmode: &BlockMode) -> usize {
		match self {
			EmbedAlgorithm::Lo(bits) => {
				fn push(vec: &mut BitVec<u8>, bits: u8, b: u8)
				{
					for i in 0..bits
					{
						vec.push((b >> i) & 0b1 == 0b1)
					}
				}

				let start = incoming.len();
				while incoming.len()-start < (blockmode.len-blockmode.crc_len)*8
				{
					push(incoming, *bits, encoded_data[data_pos]);
					data_pos += 1;
				}

				// TODO: Read CRC and verify
			}
		}

		data_pos
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
