use crate::hash::*;
use std::cmp::min;
use std::collections::{HashMap, HashSet};

pub const DEFAULT_BLOCK_SIZE: usize = 16384;

fn div_up(num: usize, den: usize) -> usize {
	(num + den - 1) / den
}

#[derive(Clone)]
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

#[derive(Clone)]
pub struct CopyCmd {
	source: u64,
	target: u64,
	size: u32,
}

pub struct PatchCommands {
	pub base: Vec<CopyCmd>,
	pub other: Vec<CopyCmd>,
}

fn compute_copy_size(cmds: &Vec<CopyCmd>) -> usize {
	let mut result: usize = 0;
	for cmd in cmds {
		result += cmd.size as usize;
	}
	result
}

impl PatchCommands {
	pub fn new() -> Self {
		Self {
			base: Vec::new(),
			other: Vec::new(),
		}
	}
	pub fn need_bytes_from_base(&self) -> usize {
		compute_copy_size(&self.base)
	}
	pub fn need_bytes_from_other(&self) -> usize {
		compute_copy_size(&self.other)
	}
}

pub fn compute_diff(input: &[u8], other_blocks: &Vec<Block>, block_size: usize) -> PatchCommands {
	let mut weak_hash_set: HashSet<u32> = HashSet::new();
	let mut base_block_hash_map: HashMap<Hash128, u64> = HashMap::new();
	let mut other_block_hash_map: HashMap<Hash128, u64> = HashMap::new();
	for block in other_blocks {
		weak_hash_set.insert(block.hash_weak);
		other_block_hash_map.insert(block.hash_strong, block.offset);
	}
	let find_base_block =
		|block_begin: usize, block_end: usize, block_hash_weak: u32| -> Option<Block> {
			if weak_hash_set.contains(&block_hash_weak) {
				let block_slice = &input[block_begin..block_end];
				let block_hash_strong = compute_hash_strong(block_slice);
				if other_block_hash_map.contains_key(&block_hash_strong) {
					let block = Block {
						offset: block_begin as u64,
						size: (block_end - block_begin) as u32,
						hash_weak: block_hash_weak,
						hash_strong: block_hash_strong,
					};
					return Some(block);
				}
			}
			return None;
		};
	let mut rolling_hash = RollingHash::new();
	let mut window_begin: usize = 0;
	let mut window_end: usize = window_begin;
	let mut num_matching_bytes: u64 = 0;
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

		match find_base_block(window_begin, window_end, rolling_hash.get()) {
			Some(base_block) => {
				window_begin = window_end;
				window_end = window_begin;
				rolling_hash = RollingHash::new();
				num_matching_bytes += base_block.size as u64;
				base_block_hash_map.insert(base_block.hash_strong, base_block.offset);
			}
			None => {
				rolling_hash.sub(input[window_begin]);
				window_begin += 1;
			}
		}
	}
	let mut patch_commands = PatchCommands::new();
	for other_block in other_blocks {
		match base_block_hash_map.get(&other_block.hash_strong) {
			Some(&base_offset) => {
				patch_commands.base.push(CopyCmd {
					source: base_offset,
					target: other_block.offset,
					size: other_block.size,
				});
			}
			None => {
				patch_commands.other.push(CopyCmd {
					source: other_block.offset,
					target: other_block.offset,
					size: other_block.size,
				});
			}
		}
	}
	assert_eq!(
		num_matching_bytes as usize,
		patch_commands.need_bytes_from_base()
	);
	patch_commands
}

pub struct Patch {
	pub data: Vec<u8>,
	pub base: Vec<CopyCmd>,
	pub other: Vec<CopyCmd>,
	pub other_size: u64,
}

pub fn build_patch(other_data: &[u8], patch_commands: &PatchCommands) -> Patch {
	let mut result = Patch {
		data: Vec::new(),
		base: patch_commands.base.clone(),
		other: Vec::new(),
		other_size: other_data.len() as u64,
	};
	for cmd in &patch_commands.other {
		let patch_copy_cmd = CopyCmd {
			source: result.data.len() as u64,
			target: cmd.target,
			size: cmd.size,
		};
		let slice_begin = cmd.source as usize;
		let slice_end = cmd.source as usize + cmd.size as usize;
		let slice = &other_data[slice_begin..slice_end];
		result.data.extend(slice.iter().cloned());
		result.other.push(patch_copy_cmd);
	}
	result
}

pub fn apply_patch(base_data: &[u8], patch: &Patch) -> Vec<u8> {
	let mut result: Vec<u8> = Vec::new();
	println!("Other size: {}", patch.other_size);
	result.resize(patch.other_size as usize, 0);
	for cmd in &patch.base {
		let source_slice = &base_data[cmd.source as usize..cmd.source as usize + cmd.size as usize];
		result[cmd.target as usize..cmd.target as usize + cmd.size as usize]
			.copy_from_slice(source_slice);
	}
	for cmd in &patch.other {
		let source_slice =
			&patch.data[cmd.source as usize..cmd.source as usize + cmd.size as usize];
		result[cmd.target as usize..cmd.target as usize + cmd.size as usize]
			.copy_from_slice(source_slice);
	}
	result
}
