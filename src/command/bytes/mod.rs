//! VISCA frame encoding: the command byte catalogue and the single frame
//! writer every encoder uses.
//!
//! One address-byte convention holds throughout: a command or inquiry is an
//! address-free *body*, and [`FrameWriter`] adds the camera's address byte and
//! the terminator when it writes the frame.

mod frame;

pub mod constants;

pub use frame::{frame_len, write_frame};
pub(crate) use frame::{nibbles, step_command_encoder, FrameWriter, Step};

/// VISCA command terminator byte.
pub const VISCA_TERMINATOR: u8 = 0xFF;

/// Concatenates body parts into one fixed-size array at compile time.
///
/// Constants that share a register (a command family and its inquiry, or two
/// command families on one opcode) are written once and assembled here, so
/// the shared bytes cannot drift. The length is checked during constant
/// evaluation: a wrong `N` fails to compile.
pub(crate) const fn concat<const N: usize>(parts: &[&[u8]]) -> [u8; N] {
    let mut out = [0u8; N];
    let mut len = 0;
    let mut part = 0;
    while part < parts.len() {
        let bytes = parts[part];
        let mut index = 0;
        while index < bytes.len() {
            assert!(
                len < N,
                "concatenated body is longer than its declared length"
            );
            out[len] = bytes[index];
            len += 1;
            index += 1;
        }
        part += 1;
    }
    assert!(
        len == N,
        "concatenated body is shorter than its declared length"
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_assembles_parts_in_order() {
        const JOINED: [u8; 5] = concat(&[&[0x01], &[0x7E, 0x01, 0x0A], &[0x00]]);
        assert_eq!(JOINED, [0x01, 0x7E, 0x01, 0x0A, 0x00]);
    }
}
