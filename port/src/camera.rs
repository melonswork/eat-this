//! The scrolling camera: where the view sits relative to Leon (`FUN_1088_0a3b` + `FUN_1088_087b` +
//! `FUN_1088_0c82`). The camera trails a target that follows the hero with a dead zone of ±40 px
//! horizontally and ±30 vertically, moves at most 15 px per frame, and stops at the map's edge (the
//! `0xa75f` TileFunctionAt returns outside the map) and at "stop scrolling" tiles (function 7100).
//!
//! Not ported: the mouse look (DAT_4e4c/4e4e), the hero code's own nudges of the screen offset
//! (hanging, climbing, DAT_4e34/36 changes in HeroUpdate), the shake (DAT_45c2) and the scripted
//! camera positions (DAT_4ee0).

use crate::world::{OUTSIDE, World};

const SCREEN: (i32, i32) = (320, 200);
const STOP_SCROLLING: u16 = 7100;

pub struct Camera {
    pub x: i32,
    pub y: i32,
    /// Where on screen the hero rests (DAT_4e34/4e36): drifts back to (0xa0, 0x7d) by 1 px a frame.
    offset: (i32, i32),
    /// The point the camera is heading for (DAT_4e38/4e3a).
    target: (i32, i32),
}

/// Rounds `v` to the tile edge on the side the camera is moving from, with the original's 16-bit
/// wrap (`(v as u16 >> 4) + (d < 0)) * 16 - probe`).
fn snap(probe: i32, d: i32) -> i32 {
    let v = (probe + d) as i16 as u16;
    ((i32::from(v >> 4) + i32::from(d < 0)) * 16 - probe) as i16 as i32
}

impl Camera {
    /// LevelLoad's `FUN_1088_0c20`: the camera starts on its target, held inside the map.
    pub fn new(world: &World, hero: (i32, i32)) -> Camera {
        let offset = (0xa0, 0x7d);
        let target = (hero.0 - offset.0, hero.1 - offset.1);
        let (mw, mh) = world.pixel_size();
        Camera {
            x: target.0.clamp(0, (mw - SCREEN.0).max(0)),
            y: target.1.clamp(0, (mh - SCREEN.1).max(0)),
            offset,
            target,
        }
    }

    /// One frame, after the hero has moved.
    pub fn update(&mut self, world: &World, hero: (i32, i32)) {
        // FUN_1088_0a3b(1): the target keeps within a dead zone around the hero's resting point.
        let follow = |hero: i32, off: i32, target: i32, zone: i32| {
            let want = hero - off;
            let d = want - target;
            if d.abs() > zone { want - d.signum() * zone } else { target }
        };
        self.target.0 = follow(hero.0, self.offset.0, self.target.0, 0x28);
        self.target.1 = follow(hero.1, self.offset.1, self.target.1, 0x1e);
        self.offset.0 += (0xa0 - self.offset.0).signum();
        self.offset.1 += (0x7d - self.offset.1).signum();
        let mut dx = self.target.0 - self.x;
        let mut dy = self.target.1 - self.y;

        // FUN_1088_087b: at most 15 px a frame, and not into stop tiles or off the map.
        let stops = |f: u16| f == STOP_SCROLLING || f == OUTSIDE;
        if dx != 0 {
            let probe = if dx < 1 { self.x } else { self.x + SCREEN.0 };
            dx = dx.clamp(-15, 15);
            if stops(world.tile_function_at(hero.1, probe + dx)) {
                dx = snap(probe, dx);
            }
        }
        if dy != 0 {
            let probe = if dy < 1 { self.y } else { self.y + SCREEN.1 };
            dy = dy.clamp(-15, 15);
            if stops(world.tile_function_at(probe + dy, hero.0)) {
                dy = snap(probe, dy);
            }
        }
        self.x += dx;
        self.y += dy;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_stops_at_the_tile_edge() {
        // moving left: round down to a tile edge and add a tile, as the original does
        assert_eq!(snap(100, -3), 112 - 100);
        // moving right: round down
        assert_eq!(snap(420, 5), 416 - 420);
        // off the left edge of the map: back to 0
        assert_eq!(snap(4, -9), -4);
    }

    /// Feeds the original's hero positions (tools/dumpframe.py --camera-trace) to the camera and
    /// compares with the camera the original drew with. Needs the CD and the trace:
    ///   cargo test --release -- --ignored camera_follows
    #[test]
    #[ignore]
    fn camera_follows_the_original() {
        let map = "../work/cd/BIN/MAPS/CITY01.MAP";
        let trace = std::fs::read_to_string("../work/frames/CITY01/cam.tsv").unwrap();
        let world = World::load(std::path::Path::new(map), std::path::Path::new("../work/cd/BIN/TILES")).unwrap();
        let rows: Vec<Vec<i32>> = trace
            .lines()
            .map(|l| l.split(' ').map(|v| v.parse().unwrap()).collect())
            .filter(|r: &Vec<i32>| r[0] >= 1)
            .collect();
        let mut cam = Camera::new(&world, (rows[0][3], rows[0][4]));
        let (mut bad, mut first) = (0, None);
        for (i, r) in rows.iter().enumerate() {
            cam.update(&world, (r[3], r[4]));
            if (cam.x, cam.y) != (r[1], r[2]) {
                bad += 1;
                first.get_or_insert((i, (cam.x, cam.y), (r[1], r[2])));
                if bad < 40 { println!("frame {} hero {:?} ours {:?} original {:?}", r[0], (r[3], r[4]), (cam.x, cam.y), (r[1], r[2])); }
            }
        }
        println!("{bad} of {} frames differ; first {first:?}", rows.len());
        assert!(bad * 10 < rows.len());
    }
}
