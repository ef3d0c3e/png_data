pub mod block;
pub mod embed;
pub mod header;
pub mod image;

use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::process::ExitCode;
use std::str::FromStr;

use bitvec::vec::BitVec;
use block::BlockMode;
use block::BlockPlacement;
use embed::EmbedAlgorithm;
use getopts::Matches;
use getopts::Options;
use header::Header;
use image::ImageInfo;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rand::Rng;
use rand::prelude::SliceRandom;
use bitvec::prelude::*;

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

	fn encode(&self, w: &mut BufWriter<Box<dyn Write>>, data: Vec<u8>) {
		let mut encoder = png::Encoder::new(w, self.width(), self.height());
		encoder.set_color(self.color_type);
		encoder.set_depth(self.bit_depth);
		let mut writer = encoder.write_header().unwrap();
		writer.write_image_data(data.as_slice()).unwrap();
		println!("Ok");
	}
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

fn decode(image: String, matches: Matches, header_only: bool) -> Result<(), String> {
	let algorithm = get_algorithm(matches.opt_str("l"))?;
	let crc = false;

	let (data, info) = decode_image(image)?;
	let blockmode = BlockMode::from_length(info.size(), crc);
	let seed = derive_seed(
		matches
			.opt_str("s")
			.unwrap_or(format!("{}x{}", info.width(), info.height()))
			.as_str(),
	)?;

	println!("Blockmode: {blockmode}");

	// Read header
	let mut read_data = BitVec::<u8>::new();
	let mut data_pos = 0;
	while read_data.len() < 9*8
	{
		data_pos = algorithm.read_block(&data, data_pos, &mut read_data, &blockmode);
	}

	let (version, blockmode, data_len, comment_len) = Header::from_data(read_data.as_bitslice());
	// Read header comment
	while read_data.len() < (9+comment_len as usize)*8
	{
		data_pos = algorithm.read_block(&data, data_pos, &mut read_data, &blockmode);
	}

	// Extract comment:
	let comment = String::from_utf8_lossy(
		&read_data.as_raw_slice()[9..(9+comment_len as usize)]
	);

	println!("=== HEADER ===");
	println!("Version : {version}");
	println!("Data Len: {data_len}");
	println!("Comment : `{comment}`");
	println!("==============");

	fn read_byte(slice: &bitvec::slice::BitSlice<u8>) -> u8
	{
		let mut result = 0;
		for i in 0..8
		{
			result |= (slice[i as usize] as u8) << i;
		}
		result
	}

	let data_start = 9+comment_len as usize;
	while read_data.len() < (data_start + data_len as usize)*8
	{
		data_pos = algorithm.read_block(&data, data_pos, &mut read_data, &blockmode);
	}

	for i in 60..80
	{
		let b = read_byte(&read_data[(data_start+i)*8..(data_start+1+i)*8]);
		println!("{i} : {b:08b} ({})", b as char);
	}



	let mut outfile = File::create("decode.png").unwrap();
	outfile.write(
		&read_data.as_raw_slice()[data_start..data_start+data_len as usize]
	).unwrap();


	Ok(())
}

fn encode(image: String, matches: Matches) -> Result<Vec<u8>, String> {
	let algorithm = get_algorithm(matches.opt_str("l"))?;
	let crc = false;
	let embed_file = matches
		.opt_str("i")
		.ok_or(format!("Embed file is required"))?;

	let (mut data, info) = decode_image(image)?;
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

	let mut rand = ChaCha8Rng::from_seed(seed);
	//let placement = BlockPlacement::new(data.as_mut_slice(), blockmode.len, &algorithm, embed_data.len(), &mut rand)?;

	//return Ok(vec![]);

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
	
	// Blocks to write
	let blocks_num = ((header_data.len()+embed_data.len()) as f64 / (header.blockmode.len-header.blockmode.crc_len) as f64).ceil() as usize;

	// Get data
	let mut bv = BitVec::<u8>::from_vec(header_data);
	bv.extend_from_raw_slice(embed_data.as_slice());
	// zero-padding
	while bv.len()/8 < blocks_num*header.blockmode.len
	{
		for i in 0..8
		{
			bv.push(false);
		}
	}

	// Shuffle the blocks
	//let mut rand = ChaCha8Rng::from_seed(seed);
	//let mut blocks_pos = (0..blocks_num).collect::<Vec<_>>();
	//blocks_pos.shuffle(&mut rand);


	println!("-------------");
	println!("Writing:     {blocks_num}x{} [{}] blocks", header.blockmode.len, header.blockmode.crc_len);
	println!("Data length: {} bytes", bv.len()/8);
	println!("-------------");

	//for i in 0..9*4 {
	//	let b = data[i] & 0b1111;
	//	println!("{b:b}");
	//}
	println!("=====");

	// TODO: make sure the rounding error keep this offset safe
	// i.e that two blocks can't overlap
	//let coffset = data.len() / (blocks_num+1);


	let mut embed_offset = 0;
	let mut data_pos = 0;
	for i in 0 .. blocks_num
	{
		println!("block: {i}/{embed_offset}/{data_pos}");
		(data_pos, embed_offset) = algorithm.next_block(
			&mut data.as_mut_slice(),
			data_pos,
			&bv,
			embed_offset,
			&header.blockmode);
	}
	println!("{}", bv.len());


	for i in 10..80 {
		let b = (data[i*2] & 0b1111) | ((data[i*2+1] & 0b1111) << 4);
		println!("{i}: {b:08b}, {}", b as char);
		fn read_byte(slice: &bitvec::slice::BitSlice<u8>) -> u8
		{
			let mut result = 0;
			for i in 0..8
			{
				result |= (slice[i as usize] as u8) << i;
			}
			result
		}
		println!("{i}+ {b:08b}, {}", read_byte(&bv[i*8..(i+1)*8]) as char);
	}
	let outfile = File::create("out.png").unwrap();
	let ref mut w = BufWriter::new(Box::new(outfile) as Box<dyn Write>);
	info.encode(w, data);

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

	if matches.opt_present("z") {
		match decode(input, matches, true) {
			Ok(_) => todo!(""),
			Err(e) => {
				eprintln!("{e}");
				return ExitCode::FAILURE;
			}
		}
	} else if matches.opt_present("e") {
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
