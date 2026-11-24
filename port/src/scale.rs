//! Scale2x (AdvMAME2x), applied twice: pixel-art upscaling by 4 on the CPU.

fn twice(src: &[u8], w: usize, h: usize, out: &mut Vec<u8>) {
    out.resize(w * h * 12, 0);
    let px = |x: usize, y: usize| -> [u8; 3] { src[(y * w + x) * 3..(y * w + x) * 3 + 3].try_into().unwrap() };
    for y in 0..h {
        for x in 0..w {
            let e = px(x, y);
            let b = px(x, y.saturating_sub(1));
            let n = px(x, (y + 1).min(h - 1));
            let d = px(x.saturating_sub(1), y);
            let f = px((x + 1).min(w - 1), y);
            let q = if b != n && d != f {
                [if d == b { d } else { e }, if b == f { f } else { e }, if d == n { d } else { e }, if n == f { f } else { e }]
            } else {
                [e; 4]
            };
            for (k, c) in q.iter().enumerate() {
                let at = ((2 * y + k / 2) * 2 * w + 2 * x + k % 2) * 3;
                out[at..at + 3].copy_from_slice(c);
            }
        }
    }
}

pub fn scale4(src: &[u8], w: usize, h: usize, tmp: &mut Vec<u8>, out: &mut Vec<u8>) {
    twice(src, w, h, tmp);
    twice(tmp, 2 * w, 2 * h, out);
}
