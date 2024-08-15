pub trait ImageInfo {
	fn width(&self) -> u32;
	fn height(&self) -> u32;
	fn size(&self) -> usize;
}
