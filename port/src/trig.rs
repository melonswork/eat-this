//! The game's angles: 1/16 degree, 0x1680 to the turn, 0 pointing right and growing
//! counterclockwise (screen y grows down, so "up" is 0x5a0).

use std::sync::LazyLock;

pub const TURN: i32 = 0x1680;

/// 1170:6bb8: round(sin(i / 16 deg) * 0x4000), checked against the original's table.
static SIN: LazyLock<Vec<i32>> = LazyLock::new(|| {
    (0..TURN)
        .map(|i| ((i as f64 / 16.0).to_radians().sin() * 16384.0).round() as i32)
        .collect()
});

pub fn sin(a: i32) -> i32 {
    SIN[a.rem_euclid(TURN) as usize]
}

/// `sin(a + 90 deg)`, as the original indexes it: `(a + 0x5a0) % 0x1680`.
pub fn cos(a: i32) -> i32 {
    sin(a + 0x5a0)
}

/// Atan2 (10f8:0036): the angle of (dx, dy) in screen coordinates. The x87 code divides
/// arctan(dx / dy) by pi / 2880 and rounds to nearest even.
pub fn atan2(dy: i32, dx: i32) -> i32 {
    let a = || ((dx as f64 / dy as f64).atan() / (std::f64::consts::PI / 2880.0)).round_ties_even() as i32;
    if dy < 0 {
        a() + 0x5a0
    } else if dy > 0 {
        (a() + 0x10e0) % TURN
    } else if dx < 0 {
        0xb40
    } else {
        0
    }
}
