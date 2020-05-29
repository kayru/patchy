use crate::rolling_hash::RollingHash;
use std::cmp::min;

pub struct Hash128([u8; 16]);

impl Hash128 {
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
	}
}

fn div_up(num: usize, den: usize) -> usize {
	(num + den - 1) / den
}

pub struct Block {
	pub offset: u64,
	pub size: u32,
	pub hash_weak: u32,
	pub hash_strong: Hash128,
}

pub fn compute_blocks(input: &[u8], block_size: usize) -> Vec<Block> {
	let mut result: Vec<Block> = Vec::new();
	let num_blocks = div_up(input.len(), block_size);
	result.reserve(num_blocks);

	for i in 0..num_blocks {
		let block_begin = i * block_size;
		let block_end = min((i + 1) * block_size, input.len());

		let block_slice = &input[block_begin..block_end];

		let mut hasher_blake3 = blake3::Hasher::new();
		hasher_blake3.update(&block_slice);

		let mut hash_strong_bytes: [u8; 16] = [0; 16];
		hash_strong_bytes.copy_from_slice(&hasher_blake3.finalize().as_bytes()[0..16]);

		let mut hash_rolling = RollingHash::new();
		hash_rolling.update(&block_slice);
		result.push(Block {
			offset: block_begin as u64,
			size: (block_end - block_begin) as u32,
			hash_weak: hash_rolling.get(),
			hash_strong: Hash128(hash_strong_bytes),
		});
	}
	result
}
