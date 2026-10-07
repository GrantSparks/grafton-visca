//! VISCA reply frames for scripted and simulated cameras.
//!
//! This is the one place the test infrastructure spells out reply bytes: the
//! testkit's [`helpers`](crate::testing::testkit::helpers) and
//! `ViscaCameraSimulator` build their frames here, and the integration suite
//! compiles this same file into `tests/common/fake_camera.rs` through
//! `#[path]`, because that suite must also run without `test-utils`. The file
//! therefore names nothing through `crate::`; the including module supplies
//! `VISCA_TERMINATOR`.
//!
//! The frames are written out byte by byte from the VISCA reply grammar rather
//! than produced by the crate's encoders, so a fake camera built from them is
//! an independent oracle for the decoders under test.
//!
//! Reply grammar (camera address 1, so every reply starts `0x90`):
//!
//! | Reply | Frame |
//! |---|---|
//! | ACK | `90 4s FF` |
//! | Completion | `90 5s FF` |
//! | Inquiry data | `90 50 <payload> FF` |
//! | Error | `90 6s ee FF` |
//!
//! `s` is the command socket and `ee` the error code. Syntax Error (`02`) and
//! Command Buffer Full (`03`) answer a command that was never accepted, so
//! they carry no socket: their one frame shape is `90 60 ee FF`.

#![allow(clippy::expect_used, clippy::panic)]

use super::VISCA_TERMINATOR;

/// Size of the Sony VISCA-over-IP envelope header.
pub const SONY_HEADER_LEN: usize = 8;

/// Error code: the message could not be parsed.
pub const SYNTAX_ERROR: u8 = 0x02;
/// Error code: the camera has no room to accept the command.
pub const COMMAND_BUFFER_FULL: u8 = 0x03;
/// Error code: the command on the socket was cancelled.
pub const COMMAND_CANCELED: u8 = 0x04;
/// Error code: the cancel named a socket that holds no command.
pub const NO_SOCKET: u8 = 0x05;
/// Error code: the command cannot run in the camera's current state.
pub const COMMAND_NOT_EXECUTABLE: u8 = 0x41;

/// Reply address byte of individual camera `camera` (1 to 7): `z0` with
/// `z = 8 + camera`, so camera 1 replies from `90` and camera 2 from `A0`.
#[must_use]
pub const fn reply_address(camera: u8) -> u8 {
    0x80 | ((camera & 0x07) << 4)
}

/// ACK for `socket` from camera `camera`: `z0 4s FF` (see [`reply_address`]).
#[must_use]
pub fn ack_from(camera: u8, socket: u8) -> Vec<u8> {
    vec![
        reply_address(camera),
        0x40 | (socket & 0x0F),
        VISCA_TERMINATOR,
    ]
}

/// ACK for `socket`: `90 4s FF`.
#[must_use]
pub fn ack(socket: u8) -> Vec<u8> {
    ack_from(1, socket)
}

/// Completion for `socket`: `90 5s FF`.
#[must_use]
pub fn complete(socket: u8) -> Vec<u8> {
    vec![0x90, 0x50 | (socket & 0x0F), VISCA_TERMINATOR]
}

/// Inquiry data reply: `90 50 <payload> FF`.
#[must_use]
pub fn inquiry_reply(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 3);
    frame.extend_from_slice(&[0x90, 0x50]);
    frame.extend_from_slice(payload);
    frame.push(VISCA_TERMINATOR);
    frame
}

/// Error reply for `socket` with error `code`: `90 6s ee FF`.
#[must_use]
pub fn error(socket: u8, code: u8) -> Vec<u8> {
    vec![0x90, 0x60 | (socket & 0x0F), code, VISCA_TERMINATOR]
}

/// Syntax Error: `90 60 02 FF`. The message was never accepted, so the frame
/// carries no socket.
#[must_use]
pub fn syntax_error() -> Vec<u8> {
    error(0, SYNTAX_ERROR)
}

/// Command Buffer Full: `90 60 03 FF`.
///
/// This is the one Buffer Full shape the test infrastructure emits. The camera
/// refused the command before assigning a socket, so the frame carries none.
#[must_use]
pub fn buffer_full() -> Vec<u8> {
    error(0, COMMAND_BUFFER_FULL)
}

/// Command Canceled for `socket`: `90 6s 04 FF`.
#[must_use]
pub fn canceled(socket: u8) -> Vec<u8> {
    error(socket, COMMAND_CANCELED)
}

/// No Socket for the socket a cancel named: `90 6s 05 FF`.
#[must_use]
pub fn no_socket(socket: u8) -> Vec<u8> {
    error(socket, NO_SOCKET)
}

/// Command Not Executable for `socket`: `90 6s 41 FF`.
#[must_use]
pub fn not_executable(socket: u8) -> Vec<u8> {
    error(socket, COMMAND_NOT_EXECUTABLE)
}

/// Wraps a VISCA reply in the Sony envelope: `01 11`, the payload length as a
/// big-endian `u16`, the big-endian `u32` sequence, then the payload.
///
/// # Panics
///
/// Panics if `payload` is longer than `u16::MAX` bytes, which no VISCA reply
/// can be.
#[must_use]
pub fn sony_reply(sequence: u32, payload: &[u8]) -> Vec<u8> {
    let length = u16::try_from(payload.len()).expect("a VISCA reply fits a u16 length");
    let mut frame = Vec::with_capacity(SONY_HEADER_LEN + payload.len());
    frame.extend_from_slice(&[0x01, 0x11]);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&sequence.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

/// The Sony payload types, as the crate's envelope defines them: VISCA
/// command, inquiry, reply and device setting; control command and reply.
pub const SONY_PAYLOAD_TYPES: [[u8; 2]; 6] = [
    [0x01, 0x00],
    [0x01, 0x10],
    [0x01, 0x11],
    [0x01, 0x20],
    [0x02, 0x00],
    [0x02, 0x01],
];

/// Splits a Sony-enveloped message into its sequence number and VISCA
/// payload, or `None` for a raw VISCA message.
///
/// A raw VISCA message starts with its address byte (`8x` to `Fx`); a Sony
/// envelope starts with a payload type whose first byte is `01` or `02`.
///
/// # Panics
///
/// Panics when `frame` starts like an envelope but is not a well-formed one:
/// a header shorter than [`SONY_HEADER_LEN`] bytes, a payload type outside
/// [`SONY_PAYLOAD_TYPES`], or a length field that differs from the payload
/// length. A malformed envelope is a framing defect the test must see, so it
/// is never mistaken for raw VISCA or stripped silently.
#[must_use]
pub fn sony_split(frame: &[u8]) -> Option<(u32, &[u8])> {
    if !matches!(frame.first(), Some(0x01 | 0x02)) {
        return None;
    }
    let Some((header, payload)) = frame.split_first_chunk::<SONY_HEADER_LEN>() else {
        panic!("truncated Sony envelope header: {frame:02X?}");
    };
    let payload_type = [header[0], header[1]];
    assert!(
        SONY_PAYLOAD_TYPES.contains(&payload_type),
        "unknown Sony payload type {payload_type:02X?} in {frame:02X?}"
    );
    let length = usize::from(u16::from_be_bytes([header[2], header[3]]));
    assert_eq!(
        length,
        payload.len(),
        "Sony length field disagrees with the payload in {frame:02X?}"
    );
    let sequence = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
    Some((sequence, payload))
}

/// The sequence number of a Sony-enveloped message, or `None` for a raw
/// VISCA message (one that starts with its address byte, `8x` to `Fx`).
///
/// # Panics
///
/// Panics when `frame` starts like an envelope (`01` or `02`) but is not a
/// well-formed one: a header shorter than eight bytes, an unknown payload
/// type, or a length field that differs from the payload length.
#[must_use]
pub fn sony_sequence(frame: &[u8]) -> Option<u32> {
    sony_split(frame).map(|(sequence, _)| sequence)
}

/// The VISCA message inside `frame`: the payload of a Sony envelope, or the
/// frame itself when it is raw VISCA.
///
/// # Panics
///
/// Panics on a malformed envelope, as [`sony_split`] does.
#[must_use]
pub fn visca_payload(frame: &[u8]) -> &[u8] {
    sony_split(frame).map_or(frame, |(_, payload)| payload)
}

/// Wraps `reply` in the framing of `request`: the Sony envelope echoing the
/// request's sequence number when the request was enveloped, raw otherwise.
///
/// # Panics
///
/// Panics on a malformed request envelope, as [`sony_split`] does.
#[must_use]
pub fn reply_like(request: &[u8], reply: &[u8]) -> Vec<u8> {
    match sony_sequence(request) {
        Some(sequence) => sony_reply(sequence, reply),
        None => reply.to_vec(),
    }
}
