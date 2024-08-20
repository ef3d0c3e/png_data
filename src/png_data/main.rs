#![feature(isqrt)]
mod header;

use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::process::ExitCode;

use crc::Crc;
use getopts::Matches;
use getopts::Options;
use header::Decode;
use header::Encode;
use header::Header;
use png::BitDepth;
use png::ColorType;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn print_usage(program: &str, opts: Options) {
	let brief = format!(
		"Usage: {0} -(e|z|d) [FILE [-o OUTPUT]] [opts]
		Encode: {0} -e file.tar -l rgb8 -o out.png -c \"(.tar)\"
		Info:   {0} -z out.png # (.tar)
		Decode: {0} -d out.png -o file.tar",
		program
	);
	print!("{}", opts.usage(&brief));
}

fn print_version() {
	print!(
		r#"png_data (c) ef3d0c3e -- Pass data as PNGs
Copyright (c) 2024
png_data is licensed under the GNU Affero General Public License version 3 (AGPLv3),
under the terms of the Free Software Foundation <https://www.gnu.org/licenses/agpl-3.0.en.html>.

This program is free software; you may modify and redistribute it.
There is NO WARRANTY, to the extent permitted by law."#
	);
}

fn str_to_layout(layout: &str) -> Result<(ColorType, BitDepth), String> {
	let split = layout
		.char_indices()
		.find(|(_, c)| c.is_ascii_digit())
		.ok_or(format!("Unable to find number for layout's bit depth"))?
		.0;
	match layout.split_at(split) {
		("rgb", bits) => match bits {
			"8" => Ok((ColorType::Rgb, BitDepth::Eight)),
			"16" => Ok((ColorType::Rgb, BitDepth::Sixteen)),
			_ => Err(format!("Color type rgb cannot have bit depth: {bits}")),
		},
		("rgba", bits) => match bits {
			"8" => Ok((ColorType::Rgba, BitDepth::Eight)),
			"16" => Ok((ColorType::Rgba, BitDepth::Sixteen)),
			_ => Err(format!("Color type rgba cannot have bit depth: {bits}")),
		},
		("g", bits) => match bits {
			"1" => Ok((ColorType::Grayscale, BitDepth::One)),
			"2" => Ok((ColorType::Grayscale, BitDepth::Two)),
			"4" => Ok((ColorType::Grayscale, BitDepth::Four)),
			"8" => Ok((ColorType::Grayscale, BitDepth::Eight)),
			"16" => Ok((ColorType::Grayscale, BitDepth::Sixteen)),
			_ => Err(format!(
				"Color type grayscale cannot have bit depth: {bits}"
			)),
		},
		("ga", bits) => match bits {
			"1" => Ok((ColorType::GrayscaleAlpha, BitDepth::One)),
			"2" => Ok((ColorType::GrayscaleAlpha, BitDepth::Two)),
			"4" => Ok((ColorType::GrayscaleAlpha, BitDepth::Four)),
			"8" => Ok((ColorType::GrayscaleAlpha, BitDepth::Eight)),
			"16" => Ok((ColorType::GrayscaleAlpha, BitDepth::Sixteen)),
			_ => Err(format!(
				"Color type grayscale alpha cannot have bit depth: {bits}"
			)),
		},
		_ => Err(format!("Uknown layout: {layout}")),
	}
}

fn bits_per_pixel(colors: ColorType, depth: BitDepth) -> u8 {
	match colors {
		ColorType::Rgb => depth as u8 * 3,
		ColorType::Rgba => depth as u8 * 4,
		ColorType::Grayscale => depth as u8,
		ColorType::GrayscaleAlpha => depth as u8 * 2,
		_ => panic!("Unsupported color type: {colors:#?}"),
	}
}

fn best_layout(size: u64, bits_per_pixel: u8) -> (u32, u32) {
	let sz = (size * 8).div_ceil(bits_per_pixel as u64);
	let width = sz.isqrt();
	(width as u32, sz.div_ceil(width) as u32)
}

fn encode(input: String, output: String, layout: String, matches: Matches) -> Result<(), String> {
	let layout = str_to_layout(layout.as_str())?;
	let comment = matches.opt_str("c");

	// Input file data
	let input_data = std::fs::read(&input)
		.map_err(|err| format!("Failed to read input file `{input}`: {err}"))?;

	// Header
	let header = Header::new(header::Version::VERSION_1, input_data.as_slice(), comment)?;
	let mut data = vec![];
	header.encode(&mut data);

	eprintln!("=== HEADER ===");
	eprintln!("Version: {:#?}", header.version);
	eprintln!(
		"Comment: {}",
		header.comment.as_ref().map_or("", |c| c.as_str())
	);
	eprintln!("Data: {}bytes CRC[{:X}]", header.data_len, header.data_crc);
	eprintln!("==============");

	let bits_per_pixel = bits_per_pixel(layout.0, layout.1);
	let (width, height) = best_layout(
		(data.len() + input_data.len()) as u64,
		bits_per_pixel
	);

	// Encode
	let output_file = File::create(&output)
		.map_err(|err| format!("Failed to open output file `{output}`: {err}"))?;
	let ref mut w = BufWriter::new(output_file);
	let mut encoder = png::Encoder::new(w, width, height);
	encoder.set_color(layout.0);
	encoder.set_depth(layout.1);
	encoder.set_compression(png::Compression::Best);
	let mut writer = encoder
		.write_header()
		.map_err(|err| format!("Failed to write png header: {err}"))?;

	// Image byte length
	let byte_len = ((width as usize) * (height as usize) * (bits_per_pixel as usize)).div_ceil(8);
	data.reserve(byte_len);

	data.extend_from_slice(input_data.as_slice());

	// Fill with random data
	let mut rng = ChaCha8Rng::from_entropy();
	while data.len() < byte_len {
		data.push(rng.gen::<u8>())
	}

	writer
		.write_image_data(&data)
		.map_err(|err| format!("Failed to write image data: {err}"))?;
	println!("File written to `{output}`");

	Ok(())
}

fn decode_header(input: String, _matches: Matches) -> Result<(), String> {
	// Input file data
	let decoder = png::Decoder::new(
		File::open(&input).map_err(|err| format!("Failed to read input file `{input}`: {err}"))?,
	);
	let mut reader = decoder
		.read_info()
		.map_err(|err| format!("Failed to read png info for `{input}`: {err}"))?;
	let mut data = vec![0; reader.output_buffer_size()];
	let info = reader
		.next_frame(data.as_mut_slice())
		.map_err(|err| format!("Failed to read png info for `{input}`: {err}"))?;
	
	data.resize(info.buffer_size(), 0);
	

	let mut it = data.iter().enumerate().map(|(idx, byte)| (idx, *byte));
	let header = Header::decode(&mut it).map_err(|err| format!("Failed to decode header: {err}"))?;
	eprintln!("=== HEADER ===");
	eprintln!("Version: {:#?}", header.version);
	eprintln!(
		"Comment: {}",
		header.comment.as_ref().map_or("", |c| c.as_str())
	);
	eprintln!("Data: {}bytes CRC[{:X}]", header.data_len, header.data_crc);
	eprintln!("==============");

	Ok(())
}

fn decode(input: String, output: String, _matches: Matches) -> Result<(), String> {
	// Input file data
	let decoder = png::Decoder::new(
		File::open(&input).map_err(|err| format!("Failed to read input file `{input}`: {err}"))?,
	);
	let mut reader = decoder
		.read_info()
		.map_err(|err| format!("Failed to read png info for `{input}`: {err}"))?;
	let mut data = vec![0; reader.output_buffer_size()];
	let info = reader
		.next_frame(data.as_mut_slice())
		.map_err(|err| format!("Failed to read png info for `{input}`: {err}"))?;
	
	data.resize(info.buffer_size(), 0);
	

	let mut it = data.iter().enumerate().map(|(idx, byte)| (idx, *byte));
	let header = 
	{
		//let mut temp_it = std::mem::take(&mut it);
		Header::decode(&mut it).map_err(|err| format!("Failed to decode header: {err}"))?
	};
	eprintln!("=== HEADER ===");
	eprintln!("Version: {:#?}", header.version);
	eprintln!(
		"Comment: {}",
		header.comment.as_ref().map_or("", |c| c.as_str())
	);
	eprintln!("Data: {}bytes CRC[{:X}]", header.data_len, header.data_crc);
	eprintln!("==============");

	// Check crc
	let data_start = it.next().ok_or(format!("Failed to get data start"))?.0;
	let file_data = &data[data_start..data_start+header.data_len as usize];
	let crc = Crc::<u32>::new(&crc::CRC_32_CKSUM).checksum(file_data);
	if crc != header.data_crc {
		Err(format!("Data CRC[{crc:X}] does not match header CRC[{:X}]", header.data_crc))?;
	}

	std::fs::write(&output, file_data).map_err(|err| format!("Failed to write to output file `{output}`: {err}"))?;
	println!("File written to `{output}`");

	Ok(())
}

fn main() -> ExitCode {
	let args: Vec<String> = env::args().collect();
	let program = args[0].clone();

	let mut opts = Options::new();
	opts.optopt("e", "encode", "Embed file", "FILE");
	opts.optopt("d", "decode", "Decode mode", "FILE");
	opts.optopt("z", "info", "Read header", "FILE");
	opts.optopt("l", "layout", "Png image layout", "TXT");
	opts.optopt("o", "output", "Output file", "PATH");
	opts.optopt("c", "comment", "Header comment", "TXT");
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

	// Check options
	if matches.opt_present("e") as usize
		+ matches.opt_present("d") as usize
		+ matches.opt_present("z") as usize
		> 1
	{
		eprintln!("Specify either `-e(--encode)`, `-z(--info)` or `-d(--decode)`");
		return ExitCode::FAILURE;
	}

	if let Some(input_file) = matches.opt_str("e") {
		let layout = match matches.opt_str("l") {
			None => {
				eprintln!("Missing required png layout (-l|--layout) option");
				return ExitCode::FAILURE;
			}
			Some(layout) => layout,
		};

		let output_file = match matches.opt_str("o") {
			None => {
				eprintln!("Missing required output (-o|--output) option");
				return ExitCode::FAILURE;
			}
			Some(output_file) => output_file,
		};

		if let Err(e) = encode(input_file, output_file, layout, matches) {
			eprintln!("{e}");
			return ExitCode::FAILURE;
		}
	} else if let Some(input_file) = matches.opt_str("z") {
		if let Err(e) = decode_header(input_file, matches) {
			eprintln!("{e}");
			return ExitCode::FAILURE;
		}
	} else if let Some(input_file) = matches.opt_str("d") {
		let output_file = match matches.opt_str("o") {
			None => {
				eprintln!("Missing required output (-o|--output) option");
				return ExitCode::FAILURE;
			}
			Some(output_file) => output_file,
		};

		if let Err(e) = decode(input_file, output_file, matches) {
			eprintln!("{e}");
			return ExitCode::FAILURE;
		}
	} else {
		print_usage(&program, opts);
		return ExitCode::SUCCESS;
	}
	ExitCode::SUCCESS
}
