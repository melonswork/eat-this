//! Recorded demos (.REC): per-frame inputs plus RandSeed every 35 frames. See re/demo.md.

use crate::archive::Archive;
use std::{io, path::Path};

pub const CHECK_PERIOD: u32 = 35;
pub const INPUTS: usize = 9;

pub struct Demo {
    pub frames: u32,
    keys: [Vec<i8>; INPUTS], // already past the priming frame
    mouse: Vec<(i16, i16)>,
    pub checks: Vec<u32>, // checks[k] = RandSeed at the input read of frame 35k + 1 (1-based); [0] after level setup
}

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

impl Demo {
    pub fn load(path: &Path) -> io::Result<Demo> {
        let a = Archive::open(path)?;
        let get = |n: &str| {
            a.get(n)
                .ok_or_else(|| bad(format!("{}: no {n}", path.display())))
        };
        let h = get("REC HEADER")?;
        let frames = u32::from_le_bytes(h[0..4].try_into().unwrap());
        let version = u16::from_le_bytes([h[6], h[7]]);
        if version != 3 {
            eprintln!("warning: demo version {version}, the game only plays version 3 correctly");
        }
        let mut keys: [Vec<i8>; INPUTS] = Default::default();
        for (i, k) in keys.iter_mut().enumerate() {
            let raw = get(&format!("KEY #{i}"))?;
            for run in raw.chunks_exact(3) {
                let extra = u16::from_le_bytes([run[0], run[1]]) as usize;
                k.extend(std::iter::repeat_n(run[2] as i8, extra + 1));
            }
            if k.len() != frames as usize + 1 {
                return Err(bad(format!(
                    "KEY #{i}: {} frames, expected {}",
                    k.len(),
                    frames + 1
                )));
            }
            k.remove(0);
        }
        let mut mouse: Vec<(i16, i16)> = Vec::new();
        for e in get("MOUSE")?.chunks_exact(4) {
            let (a, b) = (
                u16::from_le_bytes([e[0], e[1]]),
                i16::from_le_bytes([e[2], e[3]]),
            );
            if a == 0x8001 {
                let last = *mouse.last().unwrap_or(&(0, 0));
                mouse.extend(std::iter::repeat_n(last, b as usize));
            } else {
                mouse.push((a as i16, b));
            }
        }
        mouse.resize(frames as usize, *mouse.last().unwrap_or(&(0, 0)));
        let checks = get("CHECK")?
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        Ok(Demo {
            frames,
            keys,
            mouse,
            checks,
        })
    }

    /// gInput[0..9] and the mouse for game frame `f` (0-based).
    pub fn input(&self, f: u32) -> ([i8; INPUTS], (i16, i16)) {
        let mut k = [0i8; INPUTS];
        for (i, v) in k.iter_mut().enumerate() {
            *v = self.keys[i][f as usize];
        }
        (k, self.mouse[f as usize])
    }
}
