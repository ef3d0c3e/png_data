mod header;

use std::env;
use std::process::ExitCode;

use getopts::Matches;
use getopts::Options;

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

fn best_layout(size: u32, bits_per_pixel: u8) -> (u32, u32) {
	let sz : f64 = size as f64 / bits_per_pixel as f64;
	let width = sz.sqrt().floor();
	(width as u32, (sz / width as f64).ceil() as u32)
}

fn encode(input: String, output: String, layout: String, matches: Matches) -> Result<(), String> {
	Ok(())
}

fn main() -> ExitCode {
	let args: Vec<String> = env::args().collect();
	let program = args[0].clone();

	let mut opts = Options::new();
	opts.optopt("e", "encode", "Embed file", "PATH");
	opts.optopt("d", "decode", "Decode mode", "PATH");
	opts.optflag("z", "info", "Read header");
	opts.optopt("l", "layout", "Png image layout", "TXT");
	opts.optopt("p", "password", "Data password", "TXT");
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
	}
	ExitCode::SUCCESS
}
