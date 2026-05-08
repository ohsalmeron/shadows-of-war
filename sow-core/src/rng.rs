use wyrand::WyRand;

pub trait NextIntExt {
    /// Generates a random integer between min (inclusive) and max (exclusive).
    fn next_int(&mut self, min: i32, max: i32) -> i32;

    /// Returns true with probability 1/odds.
    fn chance(&mut self, odds: u32) -> bool;
}

impl NextIntExt for WyRand {
    #[inline]
    fn next_int(&mut self, min: i32, max: i32) -> i32 {
        let range = (max - min).max(1) as u64;
        min + (self.rand() % range) as i32
    }

    #[inline]
    fn chance(&mut self, odds: u32) -> bool {
        if odds == 0 {
            false
        } else {
            (self.rand() % odds as u64) == 0
        }
    }
}
