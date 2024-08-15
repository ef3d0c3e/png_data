#![feature(core_intrinsics)]
pub mod block;
pub mod embed;
pub mod header;
pub mod image;

use std::env;
use std::fs::File;
use std::process::ExitCode;
use std::str::FromStr;

use bitvec::slice::BitSlice;
use bitvec::vec::BitVec;
use block::BlockMode;
use embed::EmbedAlgorithm;
use getopts::Matches;
use getopts::Options;
use header::Header;
use image::ImageInfo;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rand::Rng;
use rand::prelude::SliceRandom;

fn print_usage(program: &str, opts: Options) {
	let brief = format!(
		"Usage: {0} -(e|d|i) FILE [opts]
		Encode: {0} -e file.tar -l rgba8 -c \"(.tar) my archive\" > out.png
		Decode: {0} -d out.png > file.tar
		Info:   {0} -i out.png # (.tar) my archive",
		program
	);
	print!("{}", opts.usage(&brief));
}

fn print_version() {
	print!(
		"png_data -- Embed data into PNG\n
Public domain\n"
	);
}

impl ImageInfo for png::OutputInfo {
	fn width(&self) -> u32 { self.width }

	fn height(&self) -> u32 { self.height }

	fn size(&self) -> usize { self.buffer_size() }
}

fn get_algorithm(s: Option<String>) -> Result<EmbedAlgorithm, String> {
	if let Some(s) = &s {
		EmbedAlgorithm::from_str(s.as_str())
	} else {
		Err("Missing required algorithm parameter".into())
	}
}

fn get_blockmode(s: Option<String>) -> Result<BlockMode, String> {
	if let Some(s) = &s {
		BlockMode::from_str(s)
	} else {
		Err("Missing requires blockmode parameter".into())
	}
}

fn decode_image(image: String) -> Result<(Vec<u8>, Box<dyn ImageInfo>), String> {
	match image.split_at(image.find('.').unwrap_or(0)).1 {
		".png" => {
			let decoder = png::Decoder::new(
				File::open(&image).map_err(|err| format!("Failed to read `{image}`: {err}"))?,
			);
			let mut reader = decoder
				.read_info()
				.map_err(|err| format!("Failed to read png info for `{image}`: {err}"))?;
			let mut result = Vec::with_capacity(reader.output_buffer_size());
			result.resize(reader.output_buffer_size(), 0);
			let info = reader
				.next_frame(result.as_mut_slice())
				.map_err(|err| format!("Failed to read png info for `{image}`: {err}"))?;
			result.resize(info.buffer_size(), 0);

			Ok((result, Box::new(info)))
		}
		_ => Err(format!("Unable get image type for {image}")),
	}
}

fn derive_seed(seed: &str) -> Result<[u8; 32], String> { 
	let mut result = [0u8; 32];
	argon2::Argon2::default().hash_password_into(seed.as_bytes(), b"SEED SALT", &mut result)
		.map_err(|err| format!("Failed to derive seed `{seed}`: {err}"))?;
	Ok(result)
}

fn encode(image: String, matches: Matches) -> Result<Vec<u8>, String> {
	let algorithm = get_algorithm(matches.opt_str("l"))?;
	let crc = false;
	let embed_file = matches
		.opt_str("i")
		.ok_or(format!("Embed file is required"))?;

	let (data, info) = decode_image(image)?;
	let blockmode = BlockMode::from_length(info.size(), crc);
	let seed = derive_seed(
		matches
			.opt_str("s")
			.unwrap_or(format!("{}x{}", info.width(), info.height()))
			.as_str(),
	)?;
	let max_size = algorithm.max_size(&blockmode, &info);

	let embed_data = std::fs::read(&embed_file)
		.map_err(|err| format!("Failed to read embed file `{embed_file}`: {err}"))?;

	// Get header
	let header = Header {
		blockmode,
		comment: matches.opt_str("c"),
	};
	let header_data = header.to_data(1, embed_data.len() as u32);

	// Check length
	if embed_data.len() + header_data.len() > max_size {
		Err(format!(
			"Cannot embed {}bytes into {}bytes using the {algorithm} algorithm with blockmode {}. Max embeddable size: {max_size}bytes",
			embed_data.len()+header_data.len(),
			data.len(),
			header.blockmode,
		))?;
	}

	// Shuffle the blocks
	let mut rand = ChaCha8Rng::from_seed(seed);
	let blocks_num = info.size() / (header.blockmode.len-header.blockmode.crc_len);

	let mut blocks_pos = (0..blocks_num).collect::<Vec<_>>();
	blocks_pos.shuffle(&mut rand);


	let mut bv = BitVec::<u8>::from_vec(header_data);
	bv.extend_from_raw_slice(embed_data.as_slice());

	let mut embed_offset = 0;
	for i in 0 .. blocks_num
	{
		println!("{:#?}", bv.len());
		let (block, mut new_offset) = algorithm.next_block(
			&data.as_slice()[i*header.blockmode.len..],
			&bv,
			embed_offset,
			&header.blockmode);
		new_offset += header.blockmode.crc_len*8;

		embed_offset = new_offset;
	}
	algorithm.next_block(data.as_slice(), &bv, 48, &header.blockmode);
	// TODO: WRITE CRC

	println!("Ok");

	Ok(vec![])
}

fn main() -> ExitCode {
	let args: Vec<String> = env::args().collect();
	let program = args[0].clone();

	let mut opts = Options::new();
	opts.optopt("i", "input", "Input file", "PATH");
	opts.optflag("e", "encode", "Encode file");
	opts.optflag("d", "decode", "Decode mode");
	opts.optopt("c", "comment", "Header comment", "TXT");
	opts.optopt("s", "seed", "Force seed", "TXT");
	opts.optflag("", "no-crc", "Disables CRC");
	opts.optflag("z", "info", "Read information");
	opts.optopt("l", "algorithm", "Embed algorithm", "lo3");
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
	if matches.free.is_empty() {
		print_usage(&program, opts);
		return ExitCode::FAILURE;
	}

	let input = matches.free[0].clone();

	if matches.opt_present("e") {
		match encode(input, matches) {
			Ok(_) => todo!(""),
			Err(e) => {
				eprintln!("{e}");
				return ExitCode::FAILURE;
			}
		}
	}

	ExitCode::SUCCESS
}
