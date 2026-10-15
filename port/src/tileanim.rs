//! Animated tiles. A TILE INFOS record is 16 bytes:
//!
//! | byte | meaning |
//! |---|---|
//! | 0 | draw kind: 0 transparent (colour 0 skipped), 2 opaque, 4 and 255 empty, 6/8 special |
//! | 1 | animation type, 0 = none (TILES.TXT's functions 100..105: "Begin-end" ... "Blink") |
//! | 2 | A: number of frames (blink: the "on" threshold) |
//! | 3 | B: game frames per animation frame (blink: the period) |
//! | 4 | C: first tile of the animation, in the same group (a static tile holds its own number) |
//! | 5 | D: phase, added to the frame counter |
//! | 6.. | functions 0..4 (u16 each) |
//!
//! `FUN_10f0_4a77` / `FUN_10f0_4676` (the tile draw) call the type's function from the table at
//! 1170:1cdc (`FUN_10f0_41cf`, 421f, 4293, 42ec, 4346, 43a9) with the record's bytes and the level
//! frame counter, and draw the tile it returns instead. Drawing only: nothing here is game state.

/// The tile (within the tile's own group) to show at level frame `frame`.
pub fn tile(info: &[u8], tile: u8, frame: u32) -> u8 {
    let kind = info[1];
    let (a, b, c, d) = (
        i64::from(info[2]),
        i64::from(info[3]),
        i64::from(info[4]),
        i64::from(info[5]),
    );
    if kind == 0 || kind > 6 || b == 0 {
        return tile; // the original would divide by zero on b == 0; no such tile is on the CD
    }
    let t = i64::from(frame) + d;
    let r = match kind {
        1 if a > 0 => c + t / b % a,
        2 if a > 1 => c + (t / b % (2 * (a - 1)) - (a - 1)).abs(),
        3 if a > 0 => a + c - 1 - t / b % a,
        4 if a > 0 => c + t * 4 / b % a,
        5 if a > 0 => a + c - 1 - t * 4 / b % a,
        6 => c + i64::from(a < t % b),
        _ => return tile,
    };
    r as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: u8, a: u8, b: u8, c: u8, d: u8) -> [u8; 16] {
        let mut r = [0; 16];
        r[1..6].copy_from_slice(&[kind, a, b, c, d]);
        r
    }

    #[test]
    fn begin_end_loops() {
        let r = rec(1, 3, 8, 5, 0); // SV008 tile 6: frames 5..7, 8 game frames each
        let seq: Vec<u8> = [0, 7, 8, 16, 23, 24, 25].iter().map(|&f| tile(&r, 6, f)).collect();
        assert_eq!(seq, [5, 5, 6, 7, 7, 5, 5]);
    }

    #[test]
    fn begin_end_begin_bounces() {
        let r = rec(2, 3, 1, 10, 0);
        let seq: Vec<u8> = (0..8).map(|f| tile(&r, 10, f)).collect();
        assert_eq!(seq, [12, 11, 10, 11, 12, 11, 10, 11]);
    }

    #[test]
    fn reversed_and_blink() {
        let r = rec(3, 3, 2, 5, 0);
        assert_eq!([0, 2, 4, 6].map(|f| tile(&r, 5, f)), [7, 6, 5, 7]);
        let blink = rec(6, 4, 10, 20, 0); // on while frame % 10 > 4
        assert_eq!([0, 4, 5, 9].map(|f| tile(&blink, 20, f)), [20, 20, 21, 21]);
    }

    #[test]
    fn static_tiles_stay() {
        assert_eq!(tile(&rec(0, 0, 0, 7, 0), 7, 1234), 7);
    }
}
