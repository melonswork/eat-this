//! IFF pictures (`BIN/PICTURES/*.LBM`): FORM PBM (chunky) and FORM ILBM (planar), ByteRun1 or
//! uncompressed. See `Picture` in tools/formats.py. The game itself only reads 320x200 PBM
//! (`FUN_10f0_0b6d`); the planar path is for the editor's other files.

use crate::palette::Pal6;
use std::{io, path::Path};

pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>, // width * height indexed
    /// CMAP shifted down to 6 bits, as the game keeps it.
    pub palette: Pal6,
}

fn bad(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, why.to_string())
}

fn be16(b: &[u8], at: usize) -> usize {
    u16::from_be_bytes([b[at], b[at + 1]]) as usize
}

/// One scan line of ByteRun1: control c < 128 copies c + 1 bytes, c > 128 repeats the next byte
/// 257 - c times, 128 does nothing.
fn byte_run1(src: &[u8], pos: &mut usize, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n);
    while out.len() < n && *pos < src.len() {
        let c = src[*pos];
        *pos += 1;
        if c < 128 {
            let end = (*pos + c as usize + 1).min(src.len());
            out.extend_from_slice(&src[*pos..end]);
            *pos = end;
        } else if c > 128 {
            if let Some(&v) = src.get(*pos) {
                out.extend(std::iter::repeat_n(v, 257 - c as usize));
            }
            *pos += 1;
        }
    }
    out.resize(n, 0);
    out
}

impl Picture {
    pub fn load(path: &Path) -> io::Result<Picture> {
        Picture::parse(&std::fs::read(path)?)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))
    }

    pub fn parse(d: &[u8]) -> io::Result<Picture> {
        if d.len() < 12 || &d[..4] != b"FORM" {
            return Err(bad("not an IFF file"));
        }
        let planar = match &d[8..12] {
            b"PBM " => false,
            b"ILBM" => true,
            _ => return Err(bad("not a PBM or ILBM picture")),
        };
        let end = (8 + u32::from_be_bytes(d[4..8].try_into().unwrap()) as usize).min(d.len());
        let (mut w, mut h, mut planes, mut masking, mut compression) = (0, 0, 0, 0, 0);
        let (mut cmap, mut body): (&[u8], &[u8]) = (&[], &[]);
        let mut at = 12;
        while at + 8 <= end {
            let size = u32::from_be_bytes(d[at + 4..at + 8].try_into().unwrap()) as usize;
            let chunk = &d[(at + 8).min(d.len())..(at + 8 + size).min(d.len())];
            match &d[at..at + 4] {
                b"BMHD" if chunk.len() >= 12 => {
                    (w, h) = (be16(chunk, 0), be16(chunk, 2));
                    (planes, masking, compression) = (chunk[8] as usize, chunk[9], chunk[10]);
                }
                b"CMAP" => cmap = chunk,
                b"BODY" => body = chunk,
                _ => {}
            }
            at += 8 + size + (size & 1); // chunks are padded to an even length
        }
        if w == 0 || h == 0 || body.is_empty() {
            return Err(bad("missing BMHD or BODY"));
        }
        let stride = w.div_ceil(16) * 2;
        let row_len = if planar {
            stride * (planes + usize::from(masking == 1))
        } else {
            w
        };
        let mut pixels = Vec::with_capacity(w * h);
        let mut pos = 0;
        for _ in 0..h {
            let row = if compression == 1 {
                byte_run1(body, &mut pos, row_len)
            } else {
                let mut r = body[pos.min(body.len())..(pos + row_len).min(body.len())].to_vec();
                r.resize(row_len, 0);
                pos += row_len;
                r
            };
            if planar {
                for x in 0..w {
                    let bit = 7 - (x & 7);
                    pixels.push(
                        (0..planes)
                            .fold(0u8, |v, p| v | (row[p * stride + (x >> 3)] >> bit & 1) << p),
                    );
                }
            } else {
                pixels.extend_from_slice(&row[..w]);
            }
        }
        let mut palette = [[0u8; 3]; 256];
        for (c, rgb) in palette.iter_mut().zip(cmap.chunks_exact(3)) {
            *c = [rgb[0] >> 2, rgb[1] >> 2, rgb[2] >> 2];
        }
        Ok(Picture {
            width: w,
            height: h,
            pixels,
            palette,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_run1_literal_and_repeat() {
        // copy 3 bytes, repeat 0x07 four times, a no-op, then literal 1 byte
        let src = [2, 1, 2, 3, 253, 7, 128, 0, 9];
        let mut pos = 0;
        assert_eq!(byte_run1(&src, &mut pos, 8), [1, 2, 3, 7, 7, 7, 7, 9]);
    }

    #[test]
    fn planar_bits_are_combined() {
        // 16x1, 2 planes, no compression: plane 0 = 1000..., plane 1 = 0100...
        let mut f = b"FORM".to_vec();
        let mut body = Vec::new();
        body.extend_from_slice(b"ILBM");
        let mut bmhd = vec![0, 16, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0];
        bmhd.resize(20, 0);
        body.extend_from_slice(b"BMHD");
        body.extend_from_slice(&20u32.to_be_bytes());
        body.extend_from_slice(&bmhd);
        body.extend_from_slice(b"BODY");
        body.extend_from_slice(&4u32.to_be_bytes());
        body.extend_from_slice(&[0x80, 0x00, 0x40, 0x00]);
        f.extend_from_slice(&(body.len() as u32).to_be_bytes());
        f.extend_from_slice(&body);
        let p = Picture::parse(&f).unwrap();
        assert_eq!((p.width, p.height), (16, 1));
        assert_eq!(&p.pixels[..3], [1, 2, 0]);
    }
}
