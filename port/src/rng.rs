//! Borland Pascal 7 System.Random (ETLAUNCH 1168:1513 / 1168:1570). The game logic draws every
//! random number from this one LCG, which is what makes recorded demos replayable.

pub struct PascalRandom {
    pub seed: u32,
    /// Calls so far, to compare against the demo's CHECK windows.
    calls: u32,
    /// Debug (env ET_RNG=frame): print every call with its Rust caller.
    pub log: bool,
    /// Debug (env ET_RNG_TSV=file): every call as `frame<TAB>n<TAB>result<TAB>caller`.
    pub dump: Option<std::io::BufWriter<std::fs::File>>,
    pub frame: u32,
}

impl PascalRandom {
    pub fn new(seed: u32) -> Self {
        PascalRandom {
            seed,
            calls: 0,
            log: false,
            dump: std::env::var("ET_RNG_TSV").ok().map(|p| std::io::BufWriter::new(std::fs::File::create(p).expect("ET_RNG_TSV"))),
            frame: 0,
        }
    }

    fn next(&mut self) -> u32 {
        self.calls += 1;
        self.seed = self.seed.wrapping_mul(0x0808_8405).wrapping_add(1);
        self.seed
    }

    /// Random(n): 0 <= result < n.
    pub fn random(&mut self, n: u16) -> i32 {
        let r = ((self.next() as u64 * n as u64) >> 32) as i32;
        if let Some(w) = self.dump.as_mut() {
            use std::io::Write;
            let bt = std::backtrace::Backtrace::force_capture().to_string();
            let caller = bt
                .lines()
                .filter(|l| l.contains("eatthis::") && !l.contains("eatthis::rng::"))
                .map(|l| l.trim().trim_start_matches(|c: char| c.is_ascii_digit() || c == ':' || c == ' '))
                .nth(0)
                .unwrap_or("");
            let _ = writeln!(w, "{}\t{n}\t{r}\t{caller}", self.frame);
        }
        if self.log {
            let bt = std::backtrace::Backtrace::force_capture().to_string();
            let caller = bt
                .lines()
                .filter(|l| l.contains("eatthis::") && !l.contains("eatthis::rng::"))
                .map(|l| l.trim().trim_start_matches(|c: char| c.is_ascii_digit() || c == ':' || c == ' '))
                .take(2)
                .collect::<Vec<_>>()
                .join(" <- ");
            eprintln!("RNG Random({n}) = {r}  {caller}");
        }
        r
    }

    /// Runs `f` with RandSeed set to `seed`, then puts the seed (and the call count the CHECK
    /// windows compare) back: the original does that around drawing code.
    pub fn scoped<R>(&mut self, seed: u32, f: impl FnOnce(&mut Self) -> R) -> R {
        let (s, c) = (self.seed, self.calls);
        self.seed = seed;
        let out = f(self);
        self.seed = s;
        self.calls = c;
        out
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
