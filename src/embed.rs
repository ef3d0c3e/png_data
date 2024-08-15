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

	pub fn next_block(&self, original_data: &[u8], embed_data: &BitVec<u8>, mut embed_offset: usize, blockmode: &BlockMode) -> (Vec<u8>, usize) {
		let mut result = Vec::<u8>::with_capacity(blockmode.len);

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
					let embed = bits_to_byte(embed_data.get(embed_offset..embed_offset+*bits as usize).unwrap(), *bits);
					let b = original_data[i];

					result.push((b & !mask) | embed);
					
					embed_offset += *bits as usize;
				}
			}
		}
		

		(result, embed_offset)
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
