//! What is drawn behind the tile planes. Each level script picks one of the original's background
//! routines with `BackgroundProc=N` (a table of function pointers at 1170:189e; the routines live in
//! segment 1090). Ported so far, checked against the original's frame buffer (tools/dumpframe.py):
//!
//! * 0, the city (CITY01): `FUN_1090_0797`.
//! * 4, the forest (FOREST02..04): `FUN_1090_1629`, its first layer only.
//!
//! Every other number falls back to the level's picture scrolled at 1/16 of the camera. That is a
//! guess, not what those routines do.

use crate::{H, W, blit, data::SpriteGroup, picture::Picture};

pub struct Background {
    pub proc_: u32,
    pub picture: Option<Picture>,
    /// The script's LoadSpriteGroup slots, in order (None: blank or missing).
    pub groups: Vec<Option<SpriteGroup>>,
    /// The level's PalMix table, for the translucent draw mode.
    pub mix: Option<Vec<u8>>,
}

/// Row `y` of the picture copied to the screen row with the picture shifted left by `shift`
/// (the original's two `FUN_1118_02db` memcpys).
fn copy_row(fb: &mut [u8], pic: &Picture, y: usize, shift: usize) {
    if y >= pic.height || pic.width != W as usize {
        return;
    }
    let shift = shift % pic.width;
    let src = &pic.pixels[y * pic.width..(y + 1) * pic.width];
    let dst = &mut fb[y * W as usize..(y + 1) * W as usize];
    dst[..pic.width - shift].copy_from_slice(&src[shift..]);
    dst[pic.width - shift..].copy_from_slice(&src[..shift]);
}

impl Background {
    fn sprite(&self, slot: usize, n: usize, fb: &mut [u8], x: i32, y: i32) {
        let Some(Some(g)) = self.groups.get(slot) else {
            return;
        };
        let Some(Some(s)) = g.sprites.get(n) else {
            return;
        };
        blit(
            fb,
            &s.pixels,
            s.width,
            s.height,
            x + s.hotspot.0,
            y + s.hotspot.1,
            false,
        );
    }

    /// Draw mode 0x30: each pixel is blended with what is under it through the PalMix table.
    fn sprite_mixed(&self, slot: usize, n: usize, fb: &mut [u8], x: i32, y: i32) {
        let (Some(mix), Some(Some(g))) = (&self.mix, self.groups.get(slot)) else {
            return self.sprite(slot, n, fb, x, y);
        };
        let Some(Some(s)) = g.sprites.get(n) else {
            return;
        };
        for sy in 0..s.height {
            for sx in 0..s.width {
                let src = s.pixels[(sy * s.width + sx) as usize];
                let (px, py) = (x + s.hotspot.0 + sx, y + s.hotspot.1 + sy);
                if src != 0 && (0..W).contains(&px) && (0..H).contains(&py) {
                    let at = (py * W + px) as usize;
                    fb[at] = mix[src as usize * 256 + fb[at] as usize];
                }
            }
        }
    }

    /// `frame` is the level frame counter as the original's draw sees it (frames completed before
    /// this one), (cam_x, cam_y) the camera.
    pub fn draw(&self, fb: &mut [u8], cam: (i32, i32), frame: u32) {
        match self.proc_ {
            0 => self.city(fb, cam, frame),
            3 => self.sea(fb, cam, frame),
            6 => {
                // FUN_1090_1a06 (GRINDER): the second group's backdrop at 1/4 of the camera,
                // tiled down. (Extra map 8 goes over it; main.rs draws that.)
                let y = (-cam.1 / 4) % H;
                self.sprite(1, 0, fb, 0, y);
                self.sprite(1, 0, fb, 0, y + H);
            }
            4 => self.forest(fb, cam),
            _ => {
                if let Some(p) = &self.picture {
                    for y in 0..H as usize {
                        copy_row(fb, p, y, (cam.0 / 16).rem_euclid(W) as usize);
                    }
                }
            }
        }
    }

    /// `FUN_1090_0797`. The sky (a 320x120 sprite that is rows 0..119 of the picture) drifts left
    /// one pixel per 8 frames; the two rows at the horizon follow the camera at 1/16; the water
    /// below scrolls faster with every row, in fixed point (`acc / 248`); then the skyline strip
    /// and its reflection (draw mode 0x30: through the PalMix table) ride at the horizon's speed. Not
    /// drawn: the little screen on a building (`DAT_4d9c`).
    fn city(&self, fb: &mut [u8], cam: (i32, i32), frame: u32) {
        let sky = -((frame >> 3) as i32 % W);
        self.sprite(0, 1, fb, sky, 0);
        self.sprite(0, 1, fb, sky + W, 0);
        let Some(pic) = &self.picture else { return };
        let horizon = ((cam.0 + 0x640) / 16 % W) as usize;
        for y in [0x7a, 0x7b] {
            copy_row(fb, pic, y, horizon);
        }
        let a = i64::from(cam.0) + i64::from(frame / 3);
        let mut acc = a * 248 / 16;
        for y in 0x7c..H as usize {
            copy_row(fb, pic, y, (acc / 248 % i64::from(W)) as usize);
            acc += a;
        }
        let s = horizon as i32;
        self.sprite(0, 0, fb, -s, 0);
        self.sprite(0, 0, fb, W - s, 0);
        self.sprite_mixed(0, 2, fb, -s, 0);
        self.sprite_mixed(0, 2, fb, W - s, 0);
    }

    /// `FUN_1090_147b` (HYDRO): the first LoadSpriteGroup's sprites at 1/6 of the camera: while
    /// the horizon (0x4b - camY / 6) is on screen, clouds drifting left one pixel per 3 frames
    /// and the far shore, then the sea from the horizon down, tiled. (Extra map 8 at 1/3 of the
    /// camera goes over it; main.rs draws that.)
    fn sea(&self, fb: &mut [u8], cam: (i32, i32), frame: u32) {
        let mut y = 0x4b - cam.1 / 6;
        let x0 = -((cam.0 / 6) % W);
        if y > 0 {
            let c = -((frame / 3) as i32 % W);
            self.sprite(0, 2, fb, c, y);
            self.sprite(0, 2, fb, c + W, y);
            self.sprite(0, 1, fb, x0, y);
            self.sprite(0, 1, fb, x0 + W, y);
        }
        if y < -199 {
            y %= 200;
        }
        for (dx, dy) in [(0, 0), (W, 0), (0, H), (W, H)] {
            self.sprite(0, 0, fb, x0 + dx, y + dy);
        }
    }

    /// `FUN_1090_1629`: a 320x200 sprite (fourth LoadSpriteGroup, sprite 0) tiled at 1/16 of the
    /// camera. Not ported: the two parallax tile layers (extra maps 8 and 9) drawn over it at 1/2
    /// and 1/4 speed.
    fn forest(&self, fb: &mut [u8], cam: (i32, i32)) {
        let x = -((cam.0 >> 4) % W);
        let y = -((cam.1 >> 4) % H);
        for (dx, dy) in [(0, 0), (W, 0), (0, H), (W, H)] {
            self.sprite(3, 0, fb, x + dx, y + dy);
        }
    }
}
