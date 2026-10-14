//! Image Line Archive File (1995) and its LZW compression. See tools/ilaf.py and tools/lzw.py.

use std::{fs, io, path::Path};

const MAGIC: &[u8] = b"Image Line Archive File (1995)\x1a";

pub struct Archive {
    #[allow(dead_code)]
    pub description: String,
    entries: Vec<(String, u8, Vec<u8>)>,
}

fn pstring(buf: &[u8], at: usize) -> String {
    let len = buf[at] as usize;
    buf[at + 1..at + 1 + len]
        .iter()
        .map(|&b| b as char)
        .collect()
}

fn u16_at(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([buf[at], buf[at + 1]])
}

fn u32_at(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(buf[at..at + 4].try_into().unwrap())
}

impl Archive {
    pub fn open(path: &Path) -> io::Result<Archive> {
        let data = fs::read(path)?;
        Archive::parse(&data).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: not an Image Line archive", path.display()),
            )
        })
    }

    pub fn parse(data: &[u8]) -> Option<Archive> {
        if !data.starts_with(MAGIC) {
            return None;
        }
        let dir = u32_at(data, 0x1f) as usize;
        let count = u16_at(data, 0x25) as usize;
        let entries = (0..count)
            .map(|i| {
                let at = dir + i * 22;
                let (off, size) = (u32_at(data, at) as usize, u32_at(data, at + 4) as usize);
                (
                    pstring(data, at + 9),
                    data[at + 8],
                    data[off..off + size].to_vec(),
                )
            })
            .collect();
        Some(Archive {
            description: pstring(data, 0x29),
            entries,
        })
    }

    /// The unpacked contents of an entry, or None if there is no such entry.
    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        let (_, flags, data) = self.entries.iter().find(|(n, _, _)| n == name)?;
        Some(if *flags & 1 != 0 {
            lzw_decompress(data)
        } else {
            data.clone()
        })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(n, _, _)| n.as_str())
    }
}

/// GIF-style LZW: 9..12 bit codes LSB-first, 0x100 clear, 0x101 end. ETLAUNCH 1108:00d6.
pub fn lzw_decompress(data: &[u8]) -> Vec<u8> {
    if data.len() < 4 {
        return Vec::new();
    }
    let size = u32_at(data, 0) as usize;
    let src = &data[4..];
    let nbits = src.len() * 8;
    let read = |pos: usize, width: usize| -> usize {
        let mut v = 0usize;
        for i in 0..width {
            let bit = pos + i;
            v |= ((src[bit >> 3] >> (bit & 7)) as usize & 1) << i;
        }
        v
    };
    let mut out = Vec::with_capacity(size);
    let mut table: Vec<(usize, u8, usize)> = Vec::new(); // (start in out, first byte, len) per code
    let (mut width, mut limit, mut pos) = (9, 0x200, 0);
    let mut prev: Option<(usize, usize)> = None; // (start, len) of the previous code's output
    while pos + width <= nbits {
        let code = read(pos, width);
        pos += width;
        match code {
            0x101 => break,
            0x100 => {
                table = (0..=255).map(|b| (usize::MAX, b as u8, 1)).collect();
                table.extend([(0, 0, 0), (0, 0, 0)]);
                (width, limit, prev) = (9, 0x200, None);
                continue;
            }
            _ => {}
        }
        let start = out.len();
        if code < 256 {
            out.push(code as u8);
        } else if code < table.len() {
            let (s, _, len) = table[code];
            out.extend_from_within(s..s + len);
        } else if code == table.len() && prev.is_some() {
            let (s, len) = prev.unwrap();
            out.extend_from_within(s..s + len);
            out.push(out[s]);
        } else {
            panic!("bad LZW code {code:#x}");
        }
        let len = out.len() - start;
        if let Some((ps, plen)) = prev {
            if table.len() < 0x1000 {
                // prev + first byte of this output: contiguous in `out` because this output follows prev
                table.push((ps, 0, plen + 1));
                if table.len() >= limit && width < 12 {
                    width += 1;
                    limit <<= 1;
                }
            }
        }
        prev = Some((start, len));
    }
    assert_eq!(out.len(), size, "LZW size mismatch");
    out
}
