use std::io::BufWriter;
use std::io::Write;

pub trait ImageInfo {
	fn width(&self) -> u32;
	fn height(&self) -> u32;
	fn size(&self) -> usize;
	fn encode(&self, w: &mut BufWriter<Box<dyn Write>>, data: Vec<u8>);
}
