use crc::Crc;

pub trait Encode {
	/// Encode the data into a vector
	fn encode(&self, vec: &mut Vec<u8>);
}

pub trait Decode {
	type Type;

	/// Decode the data from an iterator
	fn decode<I>(it: &mut I) -> Result<Self::Type, String>
	where
		I: Iterator<Item = (usize, u8)>;
}

/// The program's version.
/// Used for compatibility reasons.
#[repr(u16)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub enum Version {
	VERSION_1,
}

impl TryFrom<u16> for Version {
	type Error = String;

	fn try_from(value: u16) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Version::VERSION_1),
			ver => Err(format!("Unknown version: {ver}")),
		}
	}
}

#[derive(Debug)]
pub struct Header {
	pub version: Version,
	pub data_len: u32,
	pub data_crc: u32,
	pub comment: Option<String>,
}

impl Header {
	/// Construct a new header from the embedded data
	pub fn new(version: Version, data: &[u8], comment: Option<String>) -> Result<Self, String> {
		if data.len() > u32::MAX as usize {
			return Err(format!(
				"Embedded data length: {} is greater than maximum {}",
				data.len(),
				u32::MAX
			));
		} else if let Some(len) = comment.as_ref().map(|c| c.len()) {
			if len > u16::MAX as usize {
				return Err(format!(
					"Embedded comment is too long, maximum length: {}, got {len}",
					u16::MAX
				));
			}
		}

		Ok(Self {
			version,
			data_len: data.len() as u32,
			data_crc: Crc::<u32>::new(&crc::CRC_32_CKSUM).checksum(data),
			comment,
		})
	}
}

impl Encode for Header {
	fn encode(&self, vec: &mut Vec<u8>) {
		// Version
		vec.extend_from_slice((self.version as u16).to_le_bytes().as_slice());

		// Data Len
		vec.extend_from_slice(self.data_len.to_le_bytes().as_slice());

		// Data CRC
		vec.extend_from_slice(self.data_crc.to_le_bytes().as_slice());

		// Comment length
		let comment_length = self.comment.as_ref().map_or(0u16, |c| c.len() as u16);
		vec.extend_from_slice(comment_length.to_le_bytes().as_slice());

		// Comment
		if let Some(comment) = &self.comment {
			vec.extend_from_slice(comment.as_bytes());
		}
	}
}

impl Decode for Header {
	type Type = Header;

	fn decode<I>(it: &mut I) -> Result<Self::Type, String>
	where
		I: Iterator<Item = (usize, u8)>,
	{
		let mut count = 0;
		let mut next = || -> Result<u8, String> {
			let result = it
				.next()
				.ok_or(format!("Failed to get byte at index: {count}"));
			count += 1;
			result.map(|(_, b)| b)
		};

		let version = u16::from_le_bytes([next()?, next()?]);
		let data_len = u32::from_le_bytes([next()?, next()?, next()?, next()?]);
		let data_crc = u32::from_le_bytes([next()?, next()?, next()?, next()?]);
		let comment_length = u16::from_le_bytes([next()?, next()?]);

		let comment = if comment_length != 0 {
			let mut comment_data = Vec::with_capacity(comment_length as usize);
			for _ in 0..comment_length {
				comment_data.push(next()?);
			}

			Some(
				String::from_utf8(comment_data)
					.map_err(|e| format!("Failed to retrieve comment: {e}"))?,
			)
		} else {
			None
		};

		Ok(Header {
			version: Version::try_from(version)?,
			data_len,
			data_crc,
			comment,
		})
	}
}

/*
#[derive(Debug)]
pub struct HeaderCrypt {
	pub version: Version,
	pub nonce: Vec<u8>,
	pub data_len: u32,
	pub data_crc: u32,
	pub comment: Option<String>,
}

impl HeaderCrypt {
	/// Construct a new header from the embedded data
	pub fn new(version: Version, nonce: Vec<u8>, data: &[u8], comment: Option<String>) -> Result<Self, String> {
		if data.len() > u32::MAX as usize {
			return Err(format!(
				"Embedded data length: {} is greater than maximum {}",
				data.len(),
				u32::MAX
			));
		} else if let Some(len) = comment.as_ref().map(|c| c.len()) {
			if len > u16::MAX as usize {
				return Err(format!(
					"Embedded comment is too long, maximum length: {}, got {len}",
					u16::MAX
				));
			}
		}

		Ok(Self {
			version,
			nonce,
			data_len: data.len() as u32,
			data_crc: Crc::<u32>::new(&crc::CRC_32_CKSUM).checksum(data),
			comment,
		})
	}
}
*/
