//! Borland Pascal 7 System.Random (ETLAUNCH 1168:1513 / 1168:1570). The game logic draws every
//! random number from this one LCG, which is what makes recorded demos replayable.

pub struct PascalRandom {
    pub seed: u32,
    /// Calls so far, to compare against the demo's CHECK windows.
    calls: u32,
}

impl PascalRandom {
    pub fn new(seed: u32) -> Self {
        PascalRandom { seed, calls: 0 }
    }

    fn next(&mut self) -> u32 {
        self.calls += 1;
        self.seed = self.seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        self.seed
    }

    /// Random(n): 0 <= result < n.
    pub fn random(&mut self, n: u16) -> i32 {
        ((self.next() as u64 * n as u64) >> 32) as i32
    }

    pub fn calls(&self) -> u32 {
        self.calls
    }
}

/// Number of LCG steps from `from` to `to`, if it is at most `limit`.
pub fn steps_between(from: u32, to: u32, limit: u32) -> Option<u32> {
    let mut s = from;
    for n in 0..=limit {
        if s == to {
            return Some(n);
        }
        s = s.wrapping_mul(0x0808_8405).wrapping_add(1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_first_demo_checkpoints() {
        // BROYEUR.REC: CHECK[2] is 30 calls after seed 0, CHECK[3] 35 more.
        assert_eq!(steps_between(0, 0x476e79ea, 100), Some(30));
        assert_eq!(steps_between(0x476e79ea, 0x828d26c1, 100), Some(35));
    }
}
