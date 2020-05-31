use serde::{Deserialize, Serialize};

pub struct RollingHash {
	a: u16,
	b: u16,
	count: usize,
}

impl RollingHash {
	pub fn new() -> Self {
		RollingHash {
			a: 0,
			b: 0,
			count: 0,
		}
	}
	pub fn count(&self) -> usize {
		return self.count;
	}
	pub fn update(&mut self, input: &[u8]) {
		for x in input {
			self.add(*x);
		}
	}
	pub fn get(&self) -> u32 {
		(self.a as u32) | ((self.b as u32) << 16)
	}
	pub fn add(&mut self, x: u8) {
		self.a = self.a.wrapping_add((x.wrapping_add(31)) as u16);
		self.b = self.b.wrapping_add(self.a);
		self.count += 1;
	}
	pub fn sub(&mut self, x: u8) {
		let x2 = (x.wrapping_add(31)) as u16;
		self.a = self.a.wrapping_sub(x2);
		self.b = self.b.wrapping_sub((self.count * (x2 as usize)) as u16);
		self.count -= 1;
	}
}

#[derive(Clone, Copy, Hash, Debug, Deserialize, Serialize)]
pub struct Hash128([u8; 16]);

impl Hash128 {
	fn new_from_blake3(hash: &blake3::Hash) -> Self {
		let mut bytes: [u8; 16] = [0; 16];
		bytes.copy_from_slice(&hash.as_bytes()[0..16]);
		Self(bytes)
	}
	pub fn as_bytes(&self) -> &[u8; 16] {
		&self.0
	}
}

pub fn compute_hash_strong(input: &[u8]) -> Hash128 {
	let mut hasher_blake3 = blake3::Hasher::new();
	hasher_blake3.update(input);
	Hash128::new_from_blake3(&hasher_blake3.finalize())
}

pub fn compute_hash_weak(input: &[u8]) -> u32 {
	let mut hash_rolling = RollingHash::new();
	hash_rolling.update(&input);
	hash_rolling.get()
}

impl PartialEq for Hash128 {
	fn eq(&self, other: &Self) -> bool {
		constant_time_eq::constant_time_eq_16(&self.0, &other.0)
	}
}

impl Eq for Hash128 {}
