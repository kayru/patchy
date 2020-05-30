extern crate blake3;
extern crate clap;
extern crate memmap;

use anyhow::{Context, Result};
use clap::{App, Arg, SubCommand};
use memmap::MmapOptions;
use std::fs::File;
use std::time::Instant;

mod hash;
use hash::{RollingHash, compute_hash_strong};

mod patchy;

fn size_mb(size: usize) -> f64 {
	let mb = (1 << 20) as f64;
	return (size as f64) / mb;
}

fn hash_file(filename: &str) -> Result<()> {
	let file = File::open(filename).context("Can't open input file")?;
	let mmap = unsafe {
		MmapOptions::new()
			.map(&file)
			.context("Can't memory map input file")?
	};
	println!("File size {}", mmap.len());

	let time_begin_blake3 = Instant::now();
	let mut hasher_blake3 = blake3::Hasher::new();
	hasher_blake3.update(&mmap);
	let hash = hasher_blake3.finalize();
	let duration_blake3 = Instant::now() - time_begin_blake3;

	println!(
		"Finished in {} sec, {} MB/sec",
		duration_blake3.as_secs_f32(),
		size_mb(mmap.len()) / duration_blake3.as_secs_f64()
	);
	println!("Hash blake3: {}", hash.to_hex());

	let time_begin_rolling = Instant::now();
	let mut hash_rolling = RollingHash::new();
	hash_rolling.update(&mmap);
	let duration_rolling = Instant::now() - time_begin_rolling;
	println!(
		"Finished in {} sec, {} MB/sec",
		duration_rolling.as_secs_f32(),
		size_mb(mmap.len()) / duration_rolling.as_secs_f64()
	);
	println!("Hash rolling: {}", hash_rolling.get());

	let time_begin_blocks = Instant::now();
	let blocks = patchy::compute_blocks(&mmap, patchy::DEFAULT_BLOCK_SIZE);
	let duration_blocks = Instant::now() - time_begin_blocks;
	println!(
		"Finished computing blocks in {} sec, {} MB/sec",
		duration_blocks.as_secs_f32(),
		size_mb(mmap.len()) / duration_blocks.as_secs_f64()
	);
	println!("Blocks: {}", blocks.len());

	hasher_blake3.reset();
	for block in blocks {
		hasher_blake3.update(&(block.offset.to_le_bytes()));
		hasher_blake3.update(&(block.size.to_le_bytes()));
		hasher_blake3.update(&(block.hash_weak.to_le_bytes()));
		hasher_blake3.update(block.hash_strong.as_bytes());
	}
	println!("Hash of blocks: {}", hasher_blake3.finalize().to_hex());

	Ok(())
}

fn diff_files(base_filename: &str, other_filename: &str) -> Result<()> {
	let base_file = File::open(base_filename).context("Can't open BASE input file")?;
	let base_mmap = unsafe {
		MmapOptions::new()
			.map(&base_file)
			.context("Can't memory map input file")?
	};
	println!("Base size {} MB", size_mb(base_mmap.len()));

	let other_file = File::open(other_filename).context("Can't open OTHER input file")?;
	let other_mmap = unsafe {
		MmapOptions::new()
			.map(&other_file)
			.context("Can't memory map input file")?
	};
	println!("Other size {} MB", size_mb(other_mmap.len()));

	println!("Computing blocks for '{}'", other_filename);
	let other_blocks = patchy::compute_blocks(&other_mmap, patchy::DEFAULT_BLOCK_SIZE);

	println!("Computing diff");
	let patch_commands = patchy::compute_diff(&base_mmap, &other_blocks, patchy::DEFAULT_BLOCK_SIZE);

	println!("Diff size {} MB", size_mb(patch_commands.need_bytes_from_other()));
	println!("Blocks from BASE: {}, from OTHER: {}", patch_commands.base.len(), patch_commands.other.len());

	println!("Verifying patch");
	let patch = patchy::build_patch(&other_mmap, &patch_commands);
	let patched_base = patchy::apply_patch(&base_mmap, &patch);
	assert_eq!(patched_base.len(), other_mmap.len());

	let other_hash = compute_hash_strong(&other_mmap);
	let patched_base_hash = compute_hash_strong(&patched_base);
	assert_eq!(other_hash, patched_base_hash);

	Ok(())
}

fn main() {
	let matches = App::new("Patchy")
		.version("0.0.1")
		.about("Binary patching tool")
		.subcommand(
			SubCommand::with_name("hash")
				.about("Computes block hash for a file")
				.arg(
					Arg::with_name("INPUT")
						.index(1)
						.required(true)
						.help("Input file"),
				),
		)
		.subcommand(
			SubCommand::with_name("diff")
				.about("Computes binary difference between files")
				.arg(
					Arg::with_name("BASE")
						.index(1)
						.required(true)
						.help("Base file"),
				)
				.arg(
					Arg::with_name("OTHER")
						.index(2)
						.required(true)
						.help("Other file"),
				),
		)
		.get_matches();

	if let Some(matches) = matches.subcommand_matches("hash") {
		let input = matches.value_of("INPUT").unwrap();
		println!("Hashing '{}'", input);
		match hash_file(input) {
			Ok(_) => println!("Success"),
			Err(e) => println!("Failed: {:?}", e),
		}
	} else if let Some(matches) = matches.subcommand_matches("diff") {
		let base = matches.value_of("BASE").unwrap();
		let other = matches.value_of("OTHER").unwrap();
		println!("Diffing '{}' and '{}'", base, other);
		match diff_files(base, other) {
			Ok(_) => println!("Success"),
			Err(e) => println!("Failed: {:?}", e),
		}
	}
}
