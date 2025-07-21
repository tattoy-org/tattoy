//! Generally useful shared code.

/// The official Tattoy blue;
pub const TATTOY_BLUE: &str = "#0034a1";

#[cfg(not(target_os = "windows"))]
/// The Unix newline
pub const NEWLINE: &str = "\n";

#[cfg(target_os = "windows")]
/// The Windows newline
pub const NEWLINE: &str = "\r\n";

/// Reset any OSC colour codes
pub const RESET_COLOUR: &str = "\x1b[m";

/// OSC code to clear the terminal screen.
pub const CLEAR_SCREEN: &str = "\x1b[2J";

/// OSC code to reset the terminal screen.
pub const RESET_SCREEN: &str = "\x1bc";

/// The escape character.
pub const ESCAPE: &str = "\x1b";

/// The string terminator character.
pub const STRING_TERMINATOR: &str = "\x1c";

/// The bell character.
pub const BELL: &str = "\x07";

/// Smoothly transition between 2 values.
pub(crate) fn smoothstep(edge0: f32, edge1: f32, mut x: f32) -> f32 {
    x = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    x * x * 2.0f32.mul_add(-x, 3.0)
}

/// A simple hash function.
pub(crate) fn simple_hash(input: &[u8]) -> u64 {
    let mut hash: u64 = 0;
    for byte in input {
        let byte_u64 = u64::from(*byte);
        let shifted = safe_add(hash << 5u8, hash);
        hash = safe_add(shifted, byte_u64);
    }
    hash
}

/// Safely add 2 `u64`s by wrapping on overflow.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::integer_division_remainder_used,
    reason = "u64 always fits into u128"
)]
const fn safe_add(left: u64, right: u64) -> u64 {
    match left.checked_add(right) {
        Some(result) => result,
        None => {
            let wrapped_result = (left as u128 + right as u128) % (u64::MAX as u128 + 1);
            wrapped_result as u64
        }
    }
}
