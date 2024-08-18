use std::fmt::Formatter;
use std::str::FromStr;

/// Algorithm to embed data
#[derive(Debug)]
pub enum EmbedAlgorithm {
	Lo(u8),
}

impl EmbedAlgorithm {
	/// Get the size of the data (in bytes) once embedded by the algorithm
	pub fn embedded_size(&self, size: usize) -> usize {
		match self {
			EmbedAlgorithm::Lo(bits) => ((size * 8) as f64 / *bits as f64).ceil() as usize,
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
				if value > 7 || value == 0 {
					Err(format!(
						"Cannot specify {value} bits for `lo` method, must be within [1, 7]"
					))
				} else {
					Ok(EmbedAlgorithm::Lo(value))
				}
			}
			_ => Err(format!("Unknown algorithm: {s}")),
		}
	}
}
