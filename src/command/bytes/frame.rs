//! The crate's single VISCA frame writer.

use super::VISCA_TERMINATOR;
use crate::{error::Error, CameraId};

/// Writes one VISCA frame — address byte, body, terminator — directly into a
/// caller-owned buffer.
///
/// Every encoder builds its frame through this type, so the address byte, the
/// terminator and the [`Error::BufferTooSmall`] report each have exactly one
/// implementation. Command bodies never carry the address byte: the address is
/// a property of the frame being written, not of the command, which is why
/// every constant in [`super::constants`] and every built-in inquiry row is an
/// address-free body.
///
/// A write that does not fit is counted rather than performed, so
/// [`Self::finish`] reports the exact length the frame needs. The bytes of a
/// frame that did not fit are unspecified; only an `Ok` length describes a
/// complete frame.
#[must_use = "a frame is only complete once `finish` appends its terminator"]
#[derive(Debug)]
pub(crate) struct FrameWriter<'a> {
    buffer: &'a mut [u8],
    len: usize,
}

impl<'a> FrameWriter<'a> {
    /// Starts a frame addressed to `camera_id`.
    pub(crate) fn new(camera_id: CameraId, buffer: &'a mut [u8]) -> Self {
        Self { buffer, len: 0 }.byte(camera_id.to_address_byte())
    }

    /// Appends one body byte.
    pub(crate) fn byte(mut self, byte: u8) -> Self {
        if let Some(slot) = self.buffer.get_mut(self.len) {
            *slot = byte;
        }
        self.len += 1;
        self
    }

    /// Appends body bytes.
    pub(crate) fn bytes(self, bytes: &[u8]) -> Self {
        bytes.iter().fold(self, |frame, &byte| frame.byte(byte))
    }

    /// Appends the low `N` nibbles of `value` as nibble bytes; see
    /// [`nibbles`].
    pub(crate) fn nibbles<const N: usize>(self, value: impl Into<u32>) -> Self {
        self.bytes(&nibbles::<N>(value.into()))
    }

    /// Appends the terminator and returns the frame length.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BufferTooSmall`] with the full frame length when the
    /// buffer cannot hold the terminated frame.
    pub(crate) fn finish(self) -> Result<usize, Error> {
        let frame = self.byte(VISCA_TERMINATOR);
        if frame.len <= frame.buffer.len() {
            Ok(frame.len)
        } else {
            Err(Error::buffer_too_small(frame.len, frame.buffer.len()))
        }
    }
}

/// The length of the frame that carries `body`: the address byte, the body
/// and the terminator.
///
/// Public only so the `ViscaInquiry` derive's expansion can name it.
#[doc(hidden)]
pub const fn frame_len(body: &[u8]) -> usize {
    body.len() + 2
}

/// Splits the low `N` nibbles of `value` into nibble bytes, most significant
/// first: `nibbles::<2>(0xA5)` is `[0x0A, 0x05]`.
///
/// This is the crate's one VISCA nibble encoder; frame writing and macro
/// parameters both use it.
pub(crate) const fn nibbles<const N: usize>(value: u32) -> [u8; N] {
    let mut out = [0u8; N];
    let mut index = 0;
    while index < N {
        let shift = 4 * (N - 1 - index);
        out[index] = ((value >> shift) & 0x0F) as u8;
        index += 1;
    }
    out
}

/// Writes `address, parts…, FF` for `camera_id` into `buffer`.
///
/// This is the frame writer behind the public `visca_command!` macro and the
/// `ViscaInquiry` derive; it is public only so their expansions can name it.
///
/// # Errors
///
/// Returns [`Error::BufferTooSmall`] when `buffer` cannot hold the frame.
#[doc(hidden)]
pub fn write_frame(
    camera_id: CameraId,
    parts: &[&[u8]],
    buffer: &mut [u8],
) -> Result<usize, Error> {
    parts
        .iter()
        .fold(FrameWriter::new(camera_id, buffer), |frame, part| {
            frame.bytes(part)
        })
        .finish()
}

/// The relative-step operand shared by every VISCA control family that steps
/// a value or drives a motor.
///
/// One control prefix followed by `00`, `02` or `03` resets (or stops), steps
/// up (tele, far) or steps down (wide, near). Drive families additionally
/// encode a variable speed as `2p` (up) or `3p` (down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// `00`: reset the value, or stop the drive.
    Reset = 0x00,
    /// `02`: step up, tele or far.
    Up = 0x02,
    /// `03`: step down, wide or near.
    Down = 0x03,
}

impl Step {
    /// The step's wire byte.
    pub(crate) const fn byte(self) -> u8 {
        self as u8
    }

    /// The variable-speed drive byte `2p` (up) or `3p` (down).
    ///
    /// `speed` occupies the low nibble; the speed types bound it to `0..=7`.
    pub(crate) const fn at_speed(self, speed: u8) -> u8 {
        ((self as u8) << 4) | (speed & 0x0F)
    }
}

/// Implements [`WireEncode`](crate::command::encode::WireEncode) for a public
/// `Reset`/`Up`/`Down`/set enum.
///
/// The step variants write `control` followed by the [`Step`] byte; the set
/// variant writes `direct` followed by its parameter through the named
/// [`FrameWriter`] method. The enum keeps its own variant names. Further arms
/// may follow as `pattern => |frame| expression`.
macro_rules! step_command_encoder {
    (
        $ty:ident {
            control: $control:expr,
            direct: $direct:expr,
            $set:ident $value:tt => $method:ident::<$count:literal>($param:expr)
            $(, $pattern:pat => |$frame:ident| $other:expr)* $(,)?
        }
    ) => {
        impl $crate::command::encode::WireEncode for $ty {
            fn write_into(
                &self,
                camera_id: $crate::CameraId,
                buffer: &mut [u8],
            ) -> ::core::result::Result<usize, $crate::Error> {
                use $crate::command::bytes::{FrameWriter, Step};

                let frame = FrameWriter::new(camera_id, buffer);
                match self {
                    Self::Reset => frame.bytes(&$control).byte(Step::Reset.byte()),
                    Self::Up => frame.bytes(&$control).byte(Step::Up.byte()),
                    Self::Down => frame.bytes(&$control).byte(Step::Down.byte()),
                    Self::$set $value => frame.bytes(&$direct).$method::<$count>($param),
                    $($pattern => {
                        let $frame = frame;
                        $other
                    })*
                }
                .finish()
            }
        }
    };
}

pub(crate) use step_command_encoder;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn frame(write: impl FnOnce(FrameWriter<'_>) -> FrameWriter<'_>) -> Vec<u8> {
        let mut buffer = [0u8; 32];
        let len = write(FrameWriter::new(CameraId::CAMERA_2, &mut buffer))
            .finish()
            .unwrap();
        buffer[..len].to_vec()
    }

    #[test]
    fn frames_carry_the_address_body_and_one_terminator() {
        assert_eq!(
            frame(|f| f.bytes(&[0x01, 0x04, 0x00]).byte(0x02)),
            [0x82, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn a_data_byte_equal_to_the_terminator_is_still_terminated() {
        assert_eq!(
            frame(|f| f.bytes(&[0x01, 0x04, 0x3F, 0x02]).byte(0xFF)),
            [0x82, 0x01, 0x04, 0x3F, 0x02, 0xFF, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn nibble_writer_splits_most_significant_first() {
        assert_eq!(frame(|f| f.nibbles::<2>(0xA5_u8)), [0x82, 0x0A, 0x05, 0xFF]);
        assert_eq!(
            frame(|f| f.nibbles::<4>(0x1234_u16)),
            [0x82, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );
        assert_eq!(
            frame(|f| f.nibbles::<5>(0xFFF7_5A8C_u32)),
            [0x82, 0x07, 0x05, 0x0A, 0x08, 0x0C, 0xFF]
        );
        assert_eq!(nibbles::<2>(0x1F), [0x01, 0x0F]);
    }

    #[test]
    fn a_short_buffer_reports_the_whole_frame_length() {
        for actual in 0..6 {
            let mut buffer = [0u8; 8];
            let result = write_frame(
                CameraId::CAMERA_1,
                &[&[0x01, 0x04], &[0x00, 0x02]],
                &mut buffer[..actual],
            );
            assert!(
                matches!(
                    result,
                    Err(Error::BufferTooSmall { required: 6, actual: reported })
                        if reported == actual
                ),
                "{actual}: {result:?}"
            );
        }
    }

    #[test]
    fn write_frame_concatenates_parts() {
        let mut buffer = [0u8; 6];
        assert_eq!(
            write_frame(
                CameraId::CAMERA_1,
                &[&[0x01, 0x04], &[0x00, 0x02]],
                &mut buffer
            )
            .unwrap(),
            6
        );
        assert_eq!(buffer, [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    }

    #[test]
    fn step_bytes_and_drive_speeds_share_one_operand() {
        assert_eq!(
            [Step::Reset.byte(), Step::Up.byte(), Step::Down.byte()],
            [0x00, 0x02, 0x03]
        );
        assert_eq!(Step::Up.at_speed(7), 0x27);
        assert_eq!(Step::Down.at_speed(0), 0x30);
    }
}
