use crate::hash::*;
use std::cmp::min;
use std::collections::HashSet;

pub const DEFAULT_BLOCK_SIZE: usize = 16384;

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

		result.push(Block {
			offset: block_begin as u64,
			size: (block_end - block_begin) as u32,
			hash_weak: compute_hash_weak(block_slice),
			hash_strong: compute_hash_strong(block_slice),
		});
	}
	result
}

pub fn compute_diff(input: &[u8], other_blocks: &Vec<Block>, block_size: usize) -> usize {

	let mut matching_bytes : usize = 0;

	let mut weak_hash_set : HashSet<u32> = HashSet::new();
	let mut strong_hash_set : HashSet<Hash128> = HashSet::new();

	for block in other_blocks {
		weak_hash_set.insert(block.hash_weak);
		strong_hash_set.insert(block.hash_strong);
	}

	let should_accept_block =
		|block_begin: usize, block_end: usize, block_hash_weak: u32| -> bool { 
			if weak_hash_set.contains(&block_hash_weak) {
				let block_slice = &input[block_begin..block_end];
				let block_hash_strong = compute_hash_strong(block_slice);
				if strong_hash_set.contains(&block_hash_strong) {
					return true;
				}
			}
			return false;
		};

	let mut rolling_hash = RollingHash::new();
	let mut window_begin: usize = 0;
	let mut window_end: usize = window_begin;

	loop {
		let remaining_len = input.len() - window_begin;
		if remaining_len == 0 {
			break;
		}

		let this_window_size: usize = min(remaining_len, block_size);
		while rolling_hash.count() < this_window_size {
			rolling_hash.add(input[window_end]);
			window_end += 1;
		}

		if should_accept_block(window_begin, window_end, rolling_hash.get()) {
			window_begin = window_end;
			window_end = window_begin;
			rolling_hash = RollingHash::new();
			matching_bytes += block_size;
			continue;
		}

		rolling_hash.sub(input[window_begin]);
		window_begin += 1;
	}

	input.len() - matching_bytes
}
