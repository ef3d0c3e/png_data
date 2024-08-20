mod block;
mod embed;
mod ent;
mod header;
mod image;

use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::process::ExitCode;
use std::str::FromStr;

use bitvec::prelude::*;
use block::best_blocksize;
use block::BlockPlacement;
use block::BlockPlacementIterator;
use crc::Crc;
use embed::EmbedAlgorithm;
use ent::EntropyGenerator;
use getopts::Matches;
use getopts::Options;
use header::Decode;
use header::Encode;
use header::Header;
use image::ImageInfo;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn print_usage(program: &str, opts: Options) {
	let brief = format!(
		"Usage: {0} -l ALGORITHM -(e|z|d) [EMBED] FILE -o OUTPUT [opts]
		Encode: {0} -l lo3 -e embed.jpg input.png -o out.png -c \"Embedded JPEG file\"
		Info:   {0} -l lo3 out.png # Embedded JPEG file
		Decode: {0} -l lo3 -d out.png -o decoded.jpg",
		program
	);
	print!("{}", opts.usage(&brief));
}

fn print_version() {
	print!(
		r#"png_embed (c) ef3d0c3e -- Embed data into PNGs
Copyright (c) 2024
png_embed is licensed under the GNU Affero General Public License version 3 (AGPLv3),
under the terms of the Free Software Foundation <https://www.gnu.org/licenses/agpl-3.0.en.html>.

This program is free software; you may modify and redistribute it.
There is NO WARRANTY, to the extent permitted by law."#
	);
}

impl ImageInfo for png::OutputInfo {
	fn width(&self) -> u32 { self.width }

	fn height(&self) -> u32 { self.height }

	fn size(&self) -> usize { self.buffer_size() }

	fn encode(&self, w: &mut BufWriter<Box<dyn Write>>, data: Vec<u8>) {
		let mut encoder = png::Encoder::new(w, self.width(), self.height());
		encoder.set_color(self.color_type);
		encoder.set_depth(self.bit_depth);
		let mut writer = encoder.write_header().unwrap();
		writer.write_image_data(data.as_slice()).unwrap();
	}
}

fn decode_image(image: &str) -> Result<(Vec<u8>, Box<dyn ImageInfo>), String> {
	match image.split_at(image.find('.').unwrap_or(0)).1 {
		".png" => {
			let decoder = png::Decoder::new(
				File::open(image).map_err(|err| format!("Failed to read `{image}`: {err}"))?,
			);
			let mut reader = decoder
				.read_info()
				.map_err(|err| format!("Failed to read png info for `{image}`: {err}"))?;
			let mut result = vec![0; reader.output_buffer_size()];
			let info = reader
				.next_frame(result.as_mut_slice())
				.map_err(|err| format!("Failed to read png info for `{image}`: {err}"))?;
			result.resize(info.buffer_size(), 0);

			Ok((result, Box::new(info)))
		}
		_ => Err(format!("Unable get image type for {image}")),
	}
}

// Derives the seed from a given string.
// Currently using Argon with salt: `png_data embed`
fn derive_seed(seed: &str) -> Result<[u8; 32], String> {
	let mut result = [0u8; 32];
	argon2::Argon2::default()
		.hash_password_into(seed.as_bytes(), b"png_data embed", &mut result)
		.map_err(|err| format!("Failed to derive seed `{seed}`: {err}"))?;
	Ok(result)
}

fn encode(
	input: String,
	embed: String,
	output: String,
	algorithm: String,
	matches: Matches,
) -> Result<(), String> {
	let algorithm = EmbedAlgorithm::from_str(algorithm.as_str())?;

	let (mut data, info) = decode_image(input.as_str())?;
	let block_size = best_blocksize(info.size());
	let seed = derive_seed(
		matches
			.opt_str("s")
			.unwrap_or(format!("{}x{}", info.width(), info.height()))
			.as_str(),
	)?;
	let comment = matches.opt_str("c");

	// Data
	let embed_file_data = std::fs::read(&embed)
		.map_err(|err| format!("Failed to read embed file `{embed}`: {err}"))?;

	// Header
	let header = Header::new(
		header::Version::VERSION_1,
		embed_file_data.as_slice(),
		comment,
	)?;

	// Result
	let mut embed_data = vec![];
	header.encode(&mut embed_data);
	embed_data.extend(embed_file_data);

	eprintln!("=== HEADER ===");
	eprintln!("Version: {:#?}", header.version);
	eprintln!(
		"Comment: {}",
		header.comment.as_ref().map_or("", |c| c.as_str())
	);
	eprintln!("Data: {}bytes CRC[{:X}]", header.data_len, header.data_crc);
	eprintln!("Block: {block_size}bytes");

	let mut rand = ChaCha8Rng::from_seed(seed);
	let mut placement = BlockPlacement::new(
		&algorithm,
		data.as_mut_slice(),
		block_size,
		embed_data.len(),
		&mut rand,
	)?;

	eprintln!("Required blocks: {}", placement.blocks.len());
	eprintln!("==============");

	placement.write_embed(embed_data.as_slice().view_bits::<Lsb0>());
	if matches.opt_present("n") {
		let ent = entropy::shannon_entropy(&embed_data);
		println!("Payload entropy: {ent}\nFilling image remainder with random data...");
		placement.fill_unused(EntropyGenerator::new(
			ent as f64,
			ChaCha8Rng::from_entropy(),
		))
	}

	let outfile = File::create(&output).unwrap();
	let w = &mut BufWriter::new(Box::new(outfile) as Box<dyn Write>);
	info.encode(w, data);

	Ok(())
}

fn decode_header(input: String, algorithm: String, matches: Matches) -> Result<(), String> {
	let algorithm = EmbedAlgorithm::from_str(algorithm.as_str())?;

	let (data, info) = decode_image(input.as_str())?;
	let block_size = best_blocksize(info.size());
	let seed = derive_seed(
		matches
			.opt_str("s")
			.unwrap_or(format!("{}x{}", info.width(), info.height()))
			.as_str(),
	)?;

	let mut rand = ChaCha8Rng::from_seed(seed);
	let mut it = BlockPlacementIterator::new(&algorithm, data.as_slice(), block_size, &mut rand);

	let header = Header::decode(&mut it)?;

	eprintln!("=== HEADER ===");
	eprintln!("Version: {:#?}", header.version);
	eprintln!(
		"Comment: \"{}\"",
		header.comment.as_ref().map_or("", |c| c.as_str())
	);
	eprintln!("Data: {}bytes CRC[{:X}]", header.data_len, header.data_crc);
	eprintln!("==============");

	Ok(())
}

fn decode(
	input: String,
	output: String,
	algorithm: String,
	matches: Matches,
) -> Result<(), String> {
	let algorithm = EmbedAlgorithm::from_str(algorithm.as_str())?;

	let (data, info) = decode_image(input.as_str())?;
	let block_size = best_blocksize(info.size());
	let seed = derive_seed(
		matches
			.opt_str("s")
			.unwrap_or(format!("{}x{}", info.width(), info.height()))
			.as_str(),
	)?;

	let mut rand = ChaCha8Rng::from_seed(seed);
	let mut it = BlockPlacementIterator::new(&algorithm, data.as_slice(), block_size, &mut rand);

	let header = Header::decode(&mut it)?;

	let mut data = Vec::with_capacity(header.data_len as usize);
	while data.len() < header.data_len as usize {
		data.push(
			it.next()
				.ok_or(format!("Failed to read data byte at {}", data.len()))?,
		);
	}

	// Check CRC
	let data_crc = Crc::<u32>::new(&crc::CRC_32_CKSUM).checksum(data.as_slice());
	if data_crc != header.data_crc {
		Err(format!(
			"Data CRC do not match: HEADER={:X} GOT={data_crc:X}",
			header.data_crc
		))?;
	}

	let outfile = File::create(&output)
		.map_err(|e| format!("Failed to create output file `{output}`: {e}"))?;
	let w = &mut BufWriter::new(Box::new(outfile) as Box<dyn Write>);
	w.write_all(data.as_slice())
		.map_err(|e| format!("Failed to write to output file `{output}`: {e}"))?;

	eprintln!("File written to `{output}`");

	Ok(())
}

fn main() -> ExitCode {
	let args: Vec<String> = env::args().collect();
	let program = args[0].clone();

	let mut opts = Options::new();
	opts.optopt("e", "embed", "Embed file", "PATH");
	opts.optopt("o", "output", "Output file", "PATH");
	opts.optflag("d", "decode", "Decode mode");
	opts.optopt("c", "comment", "Header comment", "TXT");
	opts.optopt(
		"s",
		"seed",
		"Force a seed, defaults to \"{width}x{height}\"",
		"TXT",
	);
	opts.optflag("z", "info", "Read header");
	opts.optopt("l", "algorithm", "Embed algorithm", "lo3");
	opts.optflag(
		"n",
		"entropy",
		"Attempts to hide payload by modifying the file's entropy",
	);
	opts.optflag("h", "help", "Print this help menu");
	opts.optflag("v", "version", "Print program version and licenses");

	let matches = match opts.parse(&args[1..]) {
		Ok(m) => m,
		Err(f) => {
			panic!("{}", f.to_string())
		}
	};
	if matches.opt_present("v") {
		print_version();
		return ExitCode::SUCCESS;
	}
	if matches.opt_present("h") {
		print_usage(&program, opts);
		return ExitCode::SUCCESS;
	}

	// Get input file
	if matches.free.is_empty() {
		eprintln!("Missing input file");
		print_usage(&program, opts);
		return ExitCode::FAILURE;
	}
	let input_file = matches.free[0].clone();

	// Check options
	if matches.opt_present("e") as usize
		+ matches.opt_present("d") as usize
		+ matches.opt_present("z") as usize
		> 1
	{
		eprintln!("Specify either `-e(--embed)`, `-z(--info)` or `-d(--decode)`");
		return ExitCode::FAILURE;
	} else if !matches.opt_present("l") {
		eprintln!("Missing algorithm name");
		return ExitCode::FAILURE;
	}

	// Get algorithm
	let algorithm = matches.opt_str("l").unwrap();

	if matches.opt_present("e") {
		let embed_file = matches.opt_str("e").unwrap();
		if !matches.opt_present("o") {
			eprintln!("Missing -o(utput) file");
			return ExitCode::FAILURE;
		}
		let output_file = matches.opt_str("o").unwrap();

		if let Err(e) = encode(input_file, embed_file, output_file, algorithm, matches) {
			eprintln!("{e}");
			return ExitCode::FAILURE;
		}
	} else if matches.opt_present("z") {
		if let Err(e) = decode_header(input_file, algorithm, matches) {
			eprintln!("{e}");
			return ExitCode::FAILURE;
		}
	} else if matches.opt_present("d") {
		if !matches.opt_present("o") {
			eprintln!("Missing -o(utput) file");
			return ExitCode::FAILURE;
		}
		let output_file = matches.opt_str("o").unwrap();

		if let Err(e) = decode(input_file, output_file, algorithm, matches) {
			eprintln!("{e}");
			return ExitCode::FAILURE;
		}
	} else {
		print_usage(&program, opts);
		return ExitCode::FAILURE;
	}

	ExitCode::SUCCESS
}
