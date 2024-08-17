use std::fmt::Formatter;
use std::str::FromStr;

use bitvec::slice::BitSlice;
use bitvec::vec::BitVec;
use rand::prelude::SliceRandom;
use rand::Rng;

use crate::embed::EmbedAlgorithm;

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
			crc_len: (crc * len) as usize,
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

#[derive(Debug)]
pub struct BlockPlacement<'a> {
	algorithm: &'a EmbedAlgorithm,
	data: &'a mut [u8],
	block_size: usize,
	blocks: Vec<usize>,
}

impl<'a> BlockPlacement<'a> {
	// Attempts to create a new block placement
	//
	// # Errors
	//
	// Will fail if the data is too small to hold all the blocks
	pub fn new<R>(
		data: &'a mut [u8],
		algorithm: &'a EmbedAlgorithm,
		block_size: usize,
		embed_size: usize,
		rng: &mut R,
	) -> Result<Self, String>
	where
		R: Rng + ?Sized,
	{
		// Total size of the embed (crc included)
		let embedded_size = algorithm.embedded_size(embed_size);

		// Maximum number of blocks
		let max_blocks = data.len() / block_size;

		// Number of blocks
		let blocks_num = (embedded_size as f64 / block_size as f64).ceil() as usize;

		if blocks_num > max_blocks {
			return Err(format!(
				"Too many blocks required: {blocks_num}, maximum: {max_blocks}"
			));
		}

		// Blocks in the resulting data
		let mut blocks = (0..max_blocks).collect::<Vec<_>>();

		// Shuffle the block order
		blocks.shuffle(rng);

		// Only keep the first blocks_num blocks
		blocks.resize(blocks_num, 0);

		Ok(Self {
			algorithm,
			data,
			block_size,
			blocks,
		})
	}

	pub fn write_embed(&mut self) 
}

// Iterator over blocks in the resulting image
pub struct BlockPlacementIterator<'a> {
	algorithm: &'a EmbedAlgorithm,
	data: &'a [u8],
	block_size: usize,

	// Block index
	index: usize,
	// Position of the blocks
	blocks: Vec<usize>,
}

impl<'a> BlockPlacementIterator<'a> {
	pub fn new<R: Rng + ?Sized>(
		algorithm: &'a EmbedAlgorithm,
		data: &'a [u8],
		block_size: usize,
		rng: &mut R,
	) -> Self {
		// Maximum number of blocks
		let max_blocks = data.len() / block_size;

		// Blocks
		let mut blocks = (0..max_blocks).collect::<Vec<_>>();

		// Shuffle the block order
		blocks.shuffle(rng);

		Self {
			algorithm,
			data,
			block_size,
			index: 0,
			blocks,
		}
	}
}

impl<'a> Iterator for BlockPlacementIterator<'a> {
	type Item = Block<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index == self.blocks.len() {
			return None;
		}

		let pos = self.blocks[self.index] * self.block_size;
		let slice = &self.data[pos..pos + self.block_size];
		self.index += 1;

		Some(Block(self.algorithm, slice))
	}
}

// Block of data in the resulting image
#[derive(Debug)]
pub struct Block<'a>(&'a EmbedAlgorithm, &'a [u8]);

// Iterator to read embedded data inside a block
pub struct BlockIterator<'a> {
	// Block of the iterator
	block: &'a Block<'a>,

	// Byte position in [`data`]
	index: usize,

	// Remainder, i.e. bits that have been read and will be part of the `next` byte
	remainder: BitVec<u8>,
}

impl<'a> BlockIterator<'a> {
	pub fn new(block: &'a Block, previous: Option<BlockIterator>) -> Self {
		if let Some(previous) = previous {
			Self {
				block,
				index: 0,
				remainder: previous.remainder,
			}
		} else {
			Self {
				block,
				index: 0,
				remainder: BitVec::with_capacity(7),
			}
		}
	}
}

impl<'a> Iterator for BlockIterator<'a> {
	type Item = u8;

	fn next(&mut self) -> Option<Self::Item> {
		let mut byte = 0u8;

		// Read remainder
		let mut bit_idx = 0;
		for bit in &self.remainder {
			byte |= (*bit as u8) << bit_idx;
			bit_idx += 1;
		}
		self.remainder.clear();

		match self.block.0 {
			EmbedAlgorithm::Lo(bits) => {
				while bit_idx < 8 {
					// End of data
					if self.index == self.block.1.len() {
						return None;
					}

					// Read next byte
					let next = self.block.1[self.index];
					self.index += 1;

					for i in 0..*bits {
						if bit_idx < 8 {
							// Prepend bit to result
							byte |= ((next >> i) & 0b1) << bit_idx;
						} else {
							// Append bit to remainder
							self.remainder.push((next >> i) & 0b1 == 0b1)
						}

						bit_idx += 1;
					}
				}

				Some(byte)
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use rand::SeedableRng;
	use rand_chacha::ChaCha8Rng;

	use super::*;

	#[test]
	fn block_iterator() {
		let algorithm = EmbedAlgorithm::Lo(3);
		let data = vec![
			0b10111000, 0b11111001, 0b01101010, 0b00111011, 0b11011100, 0b11100110, 0b01100111,
			0b01100000,
		];

		let block = Block(&algorithm, &data);
		let mut it = BlockIterator::new(&block, None);

		assert_eq!(it.next(), Some(0b10_001_000));
		assert_eq!(it.next(), Some(0b0_100_011_0));
		assert_eq!(it.next(), Some(0b000_111_11));
	}

	#[test]
	fn blockplacement_iterator() {
		let algorithm = EmbedAlgorithm::Lo(4);
		let mut data = vec![
			0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
			0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
			0x1c, 0x1d, 0x1e, 0x1f,
		];

		let mut rand = ChaCha8Rng::from_seed([1u8; 32]);
		let mut it = BlockPlacementIterator::new::<_>(&algorithm, &data, 4, &mut rand);

		let mut rand = ChaCha8Rng::from_seed([1u8; 32]);
		let mut positions = (0..8).collect::<Vec<_>>();
		positions.shuffle(&mut rand);

		for i in 0..8 {
			let block = it.next().unwrap();
			assert_eq!(block.1[0] / 4, positions[i]);
		}
	}
}
