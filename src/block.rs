use std::fmt::Formatter;
use std::str::FromStr;

/// Block mode for embedded data
#[derive(Debug)]
pub struct BlockMode {
	pub len: usize,
	pub crc_len: usize,
}

impl BlockMode {
	/// Gets the best [`BlockMode`] and the remainder
	pub fn from_length(len: usize, crc: bool) -> Self {
		let mut best_remainder = len;
		let mut best_p = 0;
		for p in 4..16 {
			let remainder = len % (1 << p);
			if remainder <= best_remainder {
				best_remainder = remainder;
				best_p = p;
			}
		}

		Self {
			len: 1 << best_p,
			crc_len: (best_p / 4) * crc as usize,
		}
	}

	pub fn to_data(&self) -> u8 {
		((self.crc_len != 0) as u8) | ((u8::leading_zeros(self.len as u8) + 1) << 1) as u8
	}

	pub fn from_byte(byte: u8) -> BlockMode {
		let crc = byte & 0b1;
		let len = byte >> 1;

		Self {
			len: 1usize << len,
			crc_len: (crc * len) as usize
		}
	}
}

impl core::fmt::Display for BlockMode {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "(len: {}, crc_len: {})", self.len, self.crc_len)
	}
}

impl FromStr for BlockMode {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let size = s
			.parse::<usize>()
			.map_err(|err| format!("Failed to parse `{}` as block size: {err}", s))?;

		if size < 6 || size > 16 {
			Err(format!(
				"Invalid block size specified: `{size}` expected value within [6; 16]"
			))?;
		}

		Ok(BlockMode {
			len: 1 << size,
			crc_len: size,
		})
	}
}
