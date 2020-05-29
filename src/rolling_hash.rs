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
	pub fn update(&mut self, input: &[u8]) {
		for x in input {
			self.add(*x);
		}
	}
	pub fn get(&self) -> u32 {
		(self.a as u32) | ((self.b as u32) << 16)
	}
	pub fn add(&mut self, x: u8) {
		self.a += (x + 31) as u16;
		self.b += self.a;
		self.count += 1;
	}
	pub fn sub(&mut self, x: u8) {
		let x2 = (x + 31) as u16;
		self.a -= x2;
		self.b -= (self.count * (x2 as usize)) as u16;
	}
}
