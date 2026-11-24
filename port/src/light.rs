//! Extras: glow and light. The renderer's glow mask (`render::GLOW`) says which pixels were drawn by
//! fire, explosions, shots and lights; their own bright colours bleed into the pixels around them
//! (glow) and light up what is near (light). Works on the finished RGB frame.

fn blur(src: &mut Vec<f32>, w: usize, h: usize, r: usize) {
    // Two box blurs each way: close to a Gaussian.
    let mut tmp = vec![0f32; src.len()];
    let k = 1.0 / (2 * r + 1) as f32;
    for _ in 0..2 {
        for c in 0..3 {
            for y in 0..h {
                let at = |x: usize| (y * w + x) * 3 + c;
                let mut sum: f32 = (0..=r.min(w - 1)).map(|x| src[at(x)]).sum();
                for x in 0..w {
                    tmp[at(x)] = sum * k;
                    if x + r + 1 < w {
                        sum += src[at(x + r + 1)];
                    }
                    if x >= r {
                        sum -= src[at(x - r)];
                    }
                }
            }
            for x in 0..w {
                let at = |y: usize| (y * w + x) * 3 + c;
                let mut sum: f32 = (0..=r.min(h - 1)).map(|y| tmp[at(y)]).sum();
                for y in 0..h {
                    src[at(y)] = sum * k;
                    if y + r + 1 < h {
                        sum += tmp[at(y + r + 1)];
                    }
                    if y >= r {
                        sum -= tmp[at(y - r)];
                    }
                }
            }
        }
    }
}

/// `glow` and `light` are percentages; `rgb` is w x h.
pub fn apply(rgb: &mut [u8], w: usize, h: usize, mask: &[u8], glow: i32, light: i32) {
    if (glow <= 0 && light <= 0) || mask.len() != w * h || !mask.iter().any(|&m| m > 0) {
        return;
    }
    let mut emit = vec![0f32; w * h * 3];
    for (i, &m) in mask.iter().enumerate() {
        if m == 0 {
            continue;
        }
        let px = &rgb[i * 3..i * 3 + 3];
        let top = *px.iter().max().unwrap() as f32 / 255.0;
        // Only what is bright shines: smoke and dark metal in the same sprites don't.
        let t = ((top - 0.35) / 0.4).clamp(0.0, 1.0);
        let k = m as f32 / 255.0 * t * t * (3.0 - 2.0 * t);
        for c in 0..3 {
            emit[i * 3 + c] = px[c] as f32 * k;
        }
    }
    let mut lit = emit.clone();
    blur(&mut emit, w, h, 3);
    // The light reaches further: blurred at a quarter of the size.
    let (qw, qh) = (w.div_ceil(4), h.div_ceil(4));
    let mut q = vec![0f32; qw * qh * 3];
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                q[((y / 4) * qw + x / 4) * 3 + c] += lit[(y * w + x) * 3 + c] / 16.0;
            }
        }
    }
    blur(&mut q, qw, qh, 5);
    for y in 0..h {
        let fy = ((y as f32 + 0.5) / 4.0 - 0.5).clamp(0.0, (qh - 1) as f32);
        let (y0, ty) = (fy as usize, fy.fract());
        let y1 = (y0 + 1).min(qh - 1);
        for x in 0..w {
            let fx = ((x as f32 + 0.5) / 4.0 - 0.5).clamp(0.0, (qw - 1) as f32);
            let (x0, tx) = (fx as usize, fx.fract());
            let x1 = (x0 + 1).min(qw - 1);
            for c in 0..3 {
                let g = |x: usize, y: usize| q[(y * qw + x) * 3 + c];
                let a = g(x0, y0) + (g(x1, y0) - g(x0, y0)) * tx;
                let b = g(x0, y1) + (g(x1, y1) - g(x0, y1)) * tx;
                lit[(y * w + x) * 3 + c] = a + (b - a) * ty;
            }
        }
    }
    let (kg, kl) = (glow as f32 / 100.0 * 1.4, light as f32 / 100.0 * 7.0 / 255.0);
    for (i, v) in rgb.iter_mut().enumerate() {
        let s = *v as f32;
        // The light multiplies what it falls on; the glow adds on top.
        *v = (s * (1.0 + lit[i] * kl).min(3.0) + emit[i] * kg).min(255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dark_mask_changes_nothing_and_fire_lights_its_surroundings() {
        let (w, h) = (64, 40);
        let mut rgb = vec![40u8; w * h * 3];
        let before = rgb.clone();
        let mut mask = vec![0u8; w * h];
        super::apply(&mut rgb, w, h, &mask, 100, 100);
        assert_eq!(rgb, before);
        for y in 18..22 {
            for x in 30..34 {
                mask[y * w + x] = 255;
                rgb[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&[255, 200, 80]);
            }
        }
        super::apply(&mut rgb, w, h, &mask, 100, 100);
        let near = (20 * w + 38) * 3;
        assert!(rgb[near] > 40 && rgb[near] > rgb[near + 2]);
        assert_eq!(rgb[0], 40);
    }
}
