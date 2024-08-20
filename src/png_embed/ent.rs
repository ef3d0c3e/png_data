use rand::distributions::WeightedIndex;
use rand::prelude::Distribution;
use rand::Rng;

pub struct EntropyGenerator<R>
where
	R: Rng,
{
	rng: R,
	dist: WeightedIndex<f64>,
}

/// Genrates random bytes with a set entropy
impl<R: Rng> EntropyGenerator<R> {
	// FIXME: Bad entropy
	pub fn new(entropy: f64, rng: R) -> Self {
		// FIXME: Does not work for entropy below 1.0
		let n = (2.0f64.powf(entropy)).round() as usize;

		let mut probabilities = std::iter::repeat(1.0f64).take(n).collect::<Vec<_>>();
		let sum = probabilities.iter().sum::<f64>();
		probabilities.iter_mut().for_each(|p| *p /= sum);

		let dist = WeightedIndex::new(&probabilities).unwrap();

		Self { rng, dist }
	}

	pub fn next(&mut self) -> u8 { self.dist.sample(&mut self.rng) as u8 }
}

#[cfg(test)]
mod tests {
	use entropy::shannon_entropy;
	use rand::SeedableRng;
	use rand_chacha::ChaCha8Rng;

	use super::*;

	#[test]
	fn test_entropy() {
		for i in 1..8 {
			let mut gen = EntropyGenerator::new(i as f64, ChaCha8Rng::from_entropy());

			let mut data = Vec::with_capacity(1024);
			for _ in 0..1024 {
				data.push(gen.next());
			}

			assert!((shannon_entropy(data) - i as f32).abs() < 0.2);
		}
	}
}
