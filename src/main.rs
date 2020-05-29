extern crate blake3;
extern crate clap;
extern crate memmap;

use anyhow::Result;
use clap::{App, Arg, SubCommand};
use memmap::MmapOptions;
use std::fs::File;
use std::time::Instant;

mod rolling_hash;
use rolling_hash::RollingHash;

mod patchy;
use patchy::{Block, Hash128};

fn size_mb(size: usize) -> f64 {
	let mb = (1 << 20) as f64;
	return (size as f64) / mb;
}

fn hash_file(filename: &str) -> Result<()> {
	let file = File::open(filename)?;
	let mmap = unsafe { MmapOptions::new().map(&file)? };
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
	let blocks = patchy::compute_blocks(&mmap, 16384);
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
		.get_matches();

	if let Some(matches) = matches.subcommand_matches("hash") {
		let input = matches.value_of("INPUT").unwrap();
		println!("Hashing '{}'", input);
		match hash_file(input) {
			Ok(_) => println!("Success"),
			Err(_) => println!("Failed"),
		}
	}
}
