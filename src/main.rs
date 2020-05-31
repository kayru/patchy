use anyhow::{Context, Result};
use clap::{App, AppSettings, Arg, SubCommand};
use memmap::MmapOptions;
use serde::{Deserialize, Serialize};
use std::cmp::{max, min};
use std::fs::File;
use std::io::prelude::*;
use std::time::Instant;

mod hash;
use hash::*;

mod patchy;
use patchy::*;

const BLOCK_SIZE_BOUNDS_LOG2: (i32, i32) = (6, 24);
const DEFAULT_BLOCK_SIZE_LOG2: i32 = 11; // experimentally found to be the best value for smallest patch size

const COMPRESSION_LEVEL_BOUNDS: (i32, i32) = (1, 22);
const DEFAULT_COMPRESSION_LEVEL: i32 = 15;

fn size_mb(size: usize) -> f64 {
	let mb = (1 << 20) as f64;
	(size as f64) / mb
}

fn compress(data: &[u8], level: i32) -> std::io::Result<Vec<u8>> {
	zstd::stream::encode_all(data, level)
}

fn decompress(data: &[u8]) -> std::io::Result<Vec<u8>> {
	zstd::stream::decode_all(data)
}

const PATCH_FILE_ID: [u8; 8] = *b"!patchy!";
const PATCH_FILE_VERSION: u32 = 1;
#[derive(Serialize, Deserialize)]
struct PatchWithHeader {
	id: [u8; 8],
	version: u32,
	base_hash: Hash128,
	other_hash: Hash128,
	patch: Patch,
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
	let blocks = compute_blocks(&mmap, DEFAULT_BLOCK_SIZE);
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

fn diff_files(
	base_filename: &str,
	other_filename: &str,
	patch_filename: Option<&str>,
	block_size: usize,
	compression_level: i32,
) -> Result<()> {
	let base_file = File::open(base_filename).context("Can't open BASE input file")?;
	let base_mmap = unsafe {
		MmapOptions::new()
			.map(&base_file)
			.context("Can't memory map input file")?
	};
	println!(
		"Base size: {:.2} MB ({} bytes)",
		size_mb(base_mmap.len()),
		base_mmap.len()
	);

	let other_file = File::open(other_filename).context("Can't open OTHER input file")?;
	let other_mmap = unsafe {
		MmapOptions::new()
			.map(&other_file)
			.context("Can't memory map input file")?
	};
	println!(
		"Other size: {:.2} MB ({} bytes)",
		size_mb(other_mmap.len()),
		other_mmap.len()
	);

	println!("Using block size: {}", block_size);

	println!("Computing blocks hashes for '{}'", other_filename);
	let other_blocks = compute_blocks(&other_mmap, block_size);

	println!("Computing diff");
	let patch_commands = compute_diff(&base_mmap, &other_blocks, block_size);

	if patch_commands.is_synchronized() {
		println!("Patch is not required");
		return Ok(());
	}

	println!(
		"Diff size: {:.2} MB",
		size_mb(patch_commands.need_bytes_from_other())
	);

	println!(
		"Need from BASE: {:.2} MB ({} blocks), from OTHER: {:.2} MB ({} blocks)",
		size_mb(patch_commands.need_bytes_from_base()),
		patch_commands.base.len(),
		size_mb(patch_commands.need_bytes_from_other()),
		patch_commands.other.len()
	);

	let patch = build_patch(&other_mmap, &patch_commands);
	println!("Patch commands: {}", patch.base.len() + patch.other.len());

	println!("Verifying patch");
	let patched_base = apply_patch(&base_mmap, &patch);
	assert_eq!(patched_base.len(), other_mmap.len());

	let other_hash = compute_hash_strong(&other_mmap);
	let patched_base_hash = compute_hash_strong(&patched_base);
	assert_eq!(other_hash, patched_base_hash);
	drop(patched_base);

	println!("Serializing patch");
	let patch_with_header = PatchWithHeader {
		id: PATCH_FILE_ID,
		version: PATCH_FILE_VERSION,
		base_hash: compute_hash_strong(&base_mmap),
		other_hash,
		patch,
	};
	let patch_serialized: Vec<u8> = bincode::serialize(&patch_with_header)?;
	println!(
		"Serialized uncompressed size: {:.2} MB",
		size_mb(patch_serialized.len())
	);

	println!("Compressing patch (zstd level {})", compression_level);
	let patch_compressed = compress(&patch_serialized, compression_level)?;

	println!("Compressed size: {:.2} MB", size_mb(patch_compressed.len()));

	if let Some(patch_filename) = patch_filename {
		println!("Writing patch to '{}'", patch_filename);
		let mut patch_file: std::fs::File =
			File::create(patch_filename).context("Can't open PATCH output file")?;
		patch_file.write_all(&patch_compressed)?;
	}

	Ok(())
}

fn patch_file(
	base_filename: &str,
	patch_filename: &str,
	output_filename: Option<&str>,
) -> Result<()> {
	let base_file = File::open(base_filename).context("Can't open BASE file")?;
	let base_mmap = unsafe {
		MmapOptions::new()
			.map(&base_file)
			.context("Can't memory map input file")?
	};
	let patch_file = File::open(patch_filename).context("Can't open PATCH file")?;
	let patch_mmap = unsafe {
		MmapOptions::new()
			.map(&patch_file)
			.context("Can't memory map patch file")?
	};
	let patch_decompressed = decompress(&patch_mmap)?;
	let patch_with_header: PatchWithHeader = bincode::deserialize(&patch_decompressed)?;
	assert_eq!(patch_with_header.id, PATCH_FILE_ID);
	assert_eq!(patch_with_header.version, PATCH_FILE_VERSION);

	println!("Verifying base file");
	let base_hash = compute_hash_strong(&base_mmap);
	assert_eq!(base_hash, patch_with_header.base_hash);

	println!("Applying patch");
	let patched_base = apply_patch(&base_mmap, &patch_with_header.patch);

	println!("Verifying result file");
	let patched_base_hash = compute_hash_strong(&patched_base);
	assert_eq!(patched_base_hash, patch_with_header.other_hash);

	if let Some(output_filename) = output_filename {
		println!("Writing output to '{}'", output_filename);
		let mut patch_file: std::fs::File =
			File::create(output_filename).context("Can't open OUTPUT file")?;
		patch_file.write_all(&patched_base)?;
	}
	Ok(())
}

fn clamp_parameter(name: &str, v: i32, bounds: (i32, i32)) -> i32 {
	let clamped = min(max(bounds.0, v), bounds.1);
	if v != clamped {
		println!(
			"{} ({}) is outside of expected range [{}..{}] and was clamped to {}",
			name, v, bounds.0, bounds.1, clamped
		)
	}
	clamped
}

fn dispatch_command(matches: clap::ArgMatches) -> Result<()> {
	if let Some(matches) = matches.subcommand_matches("hash") {
		let input = matches.value_of("INPUT").unwrap();
		println!("Hashing '{}'", input);
		return hash_file(input);
	} else if let Some(matches) = matches.subcommand_matches("patch") {
		let base = matches.value_of("BASE").unwrap();
		let patch = matches.value_of("PATCH").unwrap();
		let output = matches.value_of("OUTPUT");
		println!("Patching '{}' using '{}'", base, patch);
		return patch_file(base, patch, output);
	} else if let Some(matches) = matches.subcommand_matches("diff") {
		let base = matches.value_of("BASE").unwrap();
		let other = matches.value_of("OTHER").unwrap();
		let patch = matches.value_of("PATCH");
		let block_size = match matches.value_of("block") {
			Some(block_str) => {
				let block_size_log2 = block_str
					.parse::<i32>()
					.context("Couldn't parse block size parameter into integer")?;
				1 << clamp_parameter("Block size", block_size_log2, BLOCK_SIZE_BOUNDS_LOG2)
			}
			None => 1 << DEFAULT_BLOCK_SIZE_LOG2,
		};
		let compression_level = match matches.value_of("level") {
			Some(level_str) => {
				let level = level_str
					.parse::<i32>()
					.context("Couldn't parse compression level parameter into integer")?;
				clamp_parameter("Compression level", level, COMPRESSION_LEVEL_BOUNDS)
			}
			None => DEFAULT_COMPRESSION_LEVEL,
		};
		println!("Diffing '{}' and '{}'", base, other);
		return diff_files(base, other, patch, block_size, compression_level);
	}
	Ok(())
}

fn main() {
	match dispatch_command(
		App::new("Patchy")
			.version(env!("CARGO_PKG_VERSION"))
			.about("Binary patching tool")
			.setting(AppSettings::SubcommandRequiredElseHelp)
			.subcommand(
				SubCommand::with_name("hash")
					.about("Computes block hash for a file")
					.arg(Arg::with_name("INPUT").required(true).help("Input file")),
			)
			.subcommand(
				SubCommand::with_name("patch")
					.about("Applies a patch created by 'diff' command")
					.arg(Arg::with_name("BASE").required(true).help("Base file"))
					.arg(Arg::with_name("PATCH").required(true).help("Patch file"))
					.arg(Arg::with_name("OUTPUT").help("Output file")),
			)
			.subcommand(
				SubCommand::with_name("diff")
					.about("Computes binary difference between files and writes patch file to disk")
					.arg(
						Arg::with_name("level")
							.short("l")
							.takes_value(true)
							.help(&format!(
								"Compression level [{}..{}], default = {}",
								COMPRESSION_LEVEL_BOUNDS.0,
								COMPRESSION_LEVEL_BOUNDS.1,
								DEFAULT_COMPRESSION_LEVEL
							)),
					)
					.arg(
						Arg::with_name("block")
							.short("b")
							.takes_value(true)
							.help(&format!(
								"Patch block size as log2(bytes) [{}..{}], default = {} ({} bytes)",
								BLOCK_SIZE_BOUNDS_LOG2.0,
								BLOCK_SIZE_BOUNDS_LOG2.1,
								DEFAULT_BLOCK_SIZE_LOG2,
								1 << DEFAULT_BLOCK_SIZE_LOG2
							)),
					)
					.arg(Arg::with_name("BASE").required(true).help("Base file"))
					.arg(Arg::with_name("OTHER").required(true).help("Other file"))
					.arg(Arg::with_name("PATCH").help("Output patch file")),
			)
			.get_matches(),
	) {
		Ok(_) => println!("Success"),
		Err(e) => println!("Failed: {:?}", e),
	}
}
