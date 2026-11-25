#![forbid(unsafe_code)]

const FNV1A_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A_PRIME: u64 = 0x0100_0000_01b3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fnv1a64 {
    state: u64,
}

impl Fnv1a64 {
    pub const fn new() -> Fnv1a64 {
        Fnv1a64 {
            state: FNV1A_OFFSET_BASIS,
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= u64::from(byte);
            self.state = self.state.wrapping_mul(FNV1A_PRIME);
        }
    }

    pub fn update_u64(&mut self, value: u64) {
        self.update(&value.to_le_bytes());
    }

    pub const fn finish(&self) -> u64 {
        self.state
    }
}

impl Default for Fnv1a64 {
    fn default() -> Fnv1a64 {
        Fnv1a64::new()
    }
}

pub fn fnv1a_64(text: &str) -> u64 {
    let mut hasher = Fnv1a64::new();
    hasher.update(text.as_bytes());
    hasher.finish()
}
