//! `java.util.Random` (48-bit LCG), so seeded climate rolls match the original's sequence.

const MULT: u64 = 0x5DEECE66D;
const MASK: u64 = (1 << 48) - 1;

#[derive(Clone, Debug)]
pub struct JavaRandom { seed: u64 }

impl JavaRandom {
    pub fn new(seed: i64) -> Self { Self { seed: (seed as u64 ^ MULT) & MASK } }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(MULT).wrapping_add(0xB)) & MASK;
        (self.seed >> (48 - bits)) as i64 as i32
    }

    pub fn next_int(&mut self) -> i32 { self.next(32) }

    /// `nextInt(bound)`, bound > 0.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if bound & -bound == bound { return ((bound as i64 * self.next(31) as i64) >> 31) as i32; }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 { return val; }
        }
    }

    pub fn next_float(&mut self) -> f32 { self.next(24) as f32 / (1 << 24) as f32 }

    pub fn next_double(&mut self) -> f64 { (((self.next(26) as i64) << 27) + self.next(27) as i64) as f64 / (1u64 << 53) as f64 }
}
