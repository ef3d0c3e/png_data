use bitvec::slice::BitSlice;
use bitvec::vec::BitVec;
use rand::prelude::SliceRandom;
use rand::Rng;

use crate::embed::EmbedAlgorithm;

/// Gets the best blocksize (i.e. that minimize remaining space) for a certain data length.
/// The blocksize is a number in range [16, 65536]
pub fn best_blocksize(len: usize) -> usize {
	let mut best_remainder = len;
	let mut best_p = 0;
	for p in 4..16 {
		let remainder = len % (1 << p);
		if remainder <= best_remainder {
			best_remainder = remainder;
			best_p = p;
		}
	}

	1 << best_p
}

/// Struct to hold the positions of data blocks
#[derive(Debug)]
pub struct BlockPlacement<'a> {
	algorithm: &'a EmbedAlgorithm,
	data: &'a mut [u8],
	block_size: usize,
	pub blocks: Vec<usize>,
}

impl<'a> BlockPlacement<'a> {
	// Attempts to create a new block placement
	//
	// # Errors
	//
	// Will fail if the data is too small to hold all the blocks
	pub fn new<R>(
		algorithm: &'a EmbedAlgorithm,
		data: &'a mut [u8],
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

	// Embeds the data into the original image
	pub fn write_embed(&mut self, embed: &BitSlice<u8>) {
		assert_eq!(embed.len() % 8, 0);

		fn bits_to_byte(slice: &BitSlice<u8>, bits: u8) -> u8 {
			let mut result: u8 = 0;
			for i in 0..bits {
				result |= (slice[i as usize] as u8) << i;
			}
			result
		}

		let mut index = 0;
		match self.algorithm {
			EmbedAlgorithm::Lo(bits) => {
				for block in &self.blocks {
					for i in 0..self.block_size {
						let pos = block * self.block_size + i;
						let hi = std::cmp::min(*bits as usize, embed.len() - index);

						self.data[pos] &= !((1 << hi) - 1);
						self.data[pos] |= bits_to_byte(&embed[index..], hi as u8);

						index += hi;
					}
				}
			}
		}
	}
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
	// Iterator over the current block
	block_it: Option<BlockIterator<'a>>,
}

impl<'a> BlockPlacementIterator<'a> {
	/// Creates a new embed iterator
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

		let first_block_pos = blocks[0] * block_size;
		let first_block = &data[first_block_pos..first_block_pos + block_size];

		Self {
			algorithm,
			data,
			block_size,
			index: 0,
			blocks,
			block_it: Some(BlockIterator::new(Block(algorithm, first_block), None)),
		}
	}
}

impl<'a> Iterator for BlockPlacementIterator<'a> {
	type Item = u8;

	/// Gets the next embedded byte in the image
	///
	/// # Note
	///
	/// Even when the [`next()`] is Some(..), if the iterator is past the embed's length, it will
	/// return garbage data.
	fn next(&mut self) -> Option<Self::Item> {
		self.block_it.as_ref()?;

		if let Some(byte) = self.block_it.as_mut().unwrap().next() {
			Some(byte)
		} else {
			self.index += 1;
			// Get next block
			if self.index == self.blocks.len() {
				return None;
			}

			let block_pos = self.blocks[self.index] * self.block_size;
			let block = &self.data[block_pos..block_pos + self.block_size];
			self.block_it = Some(BlockIterator::new(
				Block(self.algorithm, block),
				self.block_it.take(),
			));

			self.next()
		}
	}
}

// Block of data in the resulting image
#[derive(Debug)]
pub struct Block<'a>(&'a EmbedAlgorithm, &'a [u8]);

// Iterator to read embedded data inside a block
pub struct BlockIterator<'a> {
	// Block of the iterator
	block: Block<'a>,

	// Byte position in [`data`]
	index: usize,

	// Remainder, i.e. bits that have been read and will be part of the `next` byte
	remainder: BitVec<u8>,
}

impl<'a> BlockIterator<'a> {
	pub fn new(block: Block<'a>, previous: Option<BlockIterator>) -> Self {
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
	fn test_write() {
		let algorithm = EmbedAlgorithm::Lo(2);

		let mut data = vec![0u8; 8];
		let embed = vec![0xFF, 0xFF];

		let embed_bits = BitVec::<u8>::from_slice(embed.as_slice());
		let mut rand = ChaCha8Rng::from_seed([1u8; 32]);
		let mut placement = BlockPlacement::new::<_>(
			&algorithm,
			data.as_mut_slice(),
			4,
			embed_bits.len() / 8,
			&mut rand,
		)
		.unwrap();
		placement.write_embed(embed_bits.as_bitslice());

		assert_eq!(data, vec![0b00000011; 8]);
	}

	#[test]
	fn block_iterator() {
		let algorithm = EmbedAlgorithm::Lo(3);
		let data = vec![
			0b10111000, 0b11111001, 0b01101010, 0b00111011, 0b11011100, 0b11100110, 0b01100111,
			0b01100000,
		];

		let block = Block(&algorithm, &data);
		let mut it = BlockIterator::new(block, None);

		assert_eq!(it.next(), Some(0b10_001_000));
		assert_eq!(it.next(), Some(0b0100_0110));
		assert_eq!(it.next(), Some(0b0001_1111));
	}

	#[test]
	fn blockplacement_iterator() {
		let algorithm = EmbedAlgorithm::Lo(4);
		let data = vec![
			0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
			0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b,
			0x1c, 0x1d, 0x1e, 0x1f,
		];

		let mut rand = ChaCha8Rng::from_seed([1u8; 32]);
		let mut it = BlockPlacementIterator::new::<_>(&algorithm, &data, 4, &mut rand);

		let mut rand = ChaCha8Rng::from_seed([1u8; 32]);
		let mut positions = (0..8).collect::<Vec<_>>();
		positions.shuffle(&mut rand);

		for i in 0..data.len() / 2 {
			let byte = it.next().unwrap();
			// TODO...
			//assert_eq!(byte, data[positions[i/4]*4+(i%4)]);
		}
	}
}
