//! Behavioural coverage for the exported VISCA terminator.
//!
//! The library publishes `VISCA_TERMINATOR` as part of its stable surface, so
//! two facts have to stay true together:
//!
//! 1. the exported constant is the protocol's terminator (`0xFF`), and
//! 2. every frame the library produces ends with *that constant*, exactly once.
//!
//! These tests drive the production encode path -- `Request::write_into`, the
//! same call the owner uses before handing bytes to a transport -- and compare
//! the encoded bytes against the imported constant rather than a literal. A
//! terminator written as a parallel literal somewhere in the encoders therefore
//! cannot silently diverge: change the constant and the frames stop matching,
//! change an encoder's last byte and the frames stop matching. The raw and
//! envelope tests extend the same property to the two other places a frame's
//! terminator is load-bearing: raw-frame admission and transport framing.
//!
//! This replaces an earlier source-text scanner. Scanning `src/` for `0xFF`
//! cannot distinguish a hardcoded terminator from the many legitimate uses
//! (response fixtures, simulator scripts, framer comparisons, error codes, the
//! constant's own definition), so the scanner survived only by exempting them
//! all and asserted nothing about behaviour.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use bytes::BytesMut;
use grafton_visca::{
    command::{
        CommandKind, PowerInquiry, PowerOn, PowerStandby, TallyGreenInquiry, ZoomPositionInquiry,
        VISCA_TERMINATOR,
    },
    raw,
    request::builtin::{
        FocusInfinity, FocusStop, IrisReset, PanTiltHome, PanTiltReset, PresetSet, PushAfPress,
        ZoomDrive, ZoomStop,
    },
    transport::{AddressingMode, Envelope, RawVisca, SonyEncapsulated},
    CameraId, Coarse, ControlClass, Error, PresetNumber, Request, RetryClass, TimeoutClass,
    ZoomSpeed,
};

/// Sony's encapsulation header, which precedes the VISCA payload.
const SONY_HEADER_SIZE: usize = 8;

/// Encodes one request through the production `Request::write_into` path.
///
/// The buffer is deliberately generous: exact `MAX_SIZE` accounting is covered
/// by the encoding tests for issue #517, and an over-long frame should fail
/// here on the terminator property rather than on a buffer bound.
fn encode<R: Request>(request: &R) -> Vec<u8> {
    let mut buffer = vec![0u8; R::MAX_SIZE.max(64)];
    let len = request
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("request should encode");
    buffer.truncate(len);
    buffer
}

fn terminator_count(frame: &[u8]) -> usize {
    frame
        .iter()
        .filter(|&&byte| byte == VISCA_TERMINATOR)
        .count()
}

/// Asserts the frame ends with the exported terminator and carries it once.
fn assert_terminated(name: &str, frame: &[u8]) {
    assert!(!frame.is_empty(), "{name} encoded an empty frame");
    assert_eq!(
        frame[frame.len() - 1],
        VISCA_TERMINATOR,
        "{name} must end with the exported VISCA_TERMINATOR, frame was {frame:02x?}"
    );
    assert_eq!(
        terminator_count(frame),
        1,
        "{name} must carry exactly one terminator, frame was {frame:02x?}"
    );
}

/// Encodes a request and asserts the terminator property on its frame.
macro_rules! assert_encodes_terminated {
    ($($request:expr),+ $(,)?) => {
        $(assert_terminated(stringify!($request), &encode(&$request));)+
    };
}

/// The exported terminator is the protocol's terminator, and real typed
/// commands encode it as their final byte.
///
/// This is the property the deleted source scanner claimed to protect.
#[test]
fn exported_terminator_matches_the_protocol_and_the_encoder() {
    assert_eq!(
        VISCA_TERMINATOR, 0xFF,
        "VISCA protocol requires the terminator to be 0xFF"
    );

    let power_on = encode(&PowerOn::new());
    assert_terminated("PowerOn", &power_on);
    assert_eq!(
        power_on,
        [0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        "PowerOn must encode the canonical terminated frame"
    );

    let zoom_tele = encode(&ZoomDrive::Tele);
    assert_terminated("ZoomDrive::Tele", &zoom_tele);
    assert_eq!(
        zoom_tele,
        [0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR],
        "ZoomDrive::Tele must encode the canonical terminated frame"
    );
}

/// The same property across every built-in command family, so a new or
/// re-worked encoder cannot introduce an unterminated or double-terminated
/// frame in one corner of the surface.
#[test]
fn built_in_commands_end_with_the_exported_terminator() {
    assert_encodes_terminated!(
        PowerOn::new(),
        PowerStandby::new(),
        PanTiltHome,
        PanTiltReset,
        ZoomDrive::Tele,
        ZoomDrive::Wide,
        ZoomDrive::TeleVariable(ZoomSpeed::from(Coarse::Fast)),
        ZoomDrive::WideVariable(ZoomSpeed::ZERO),
        ZoomStop,
        FocusInfinity,
        FocusStop,
        IrisReset,
        PushAfPress,
        PresetSet::new(PresetNumber::new(3).expect("preset 3 is valid")),
    );
}

/// Inquiries travel the same encode path and carry the same terminator.
#[test]
fn built_in_inquiries_end_with_the_exported_terminator() {
    let zoom_position = encode(&ZoomPositionInquiry);
    assert_terminated("ZoomPositionInquiry", &zoom_position);
    assert_eq!(
        zoom_position,
        [0x81, 0x09, 0x04, 0x47, VISCA_TERMINATOR],
        "ZoomPositionInquiry must encode the canonical terminated frame"
    );

    // `TallyGreenInquiry` uses the extended (vendor) inquiry envelope.
    assert_encodes_terminated!(PowerInquiry, TallyGreenInquiry);
}

/// Raw frame admission is validated against the same terminator the encoders
/// emit, so the escape hatch cannot accept a frame the wire would reject.
#[test]
fn raw_frames_are_admitted_against_the_exported_terminator() {
    let terminated = [0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];
    let accepted = raw::Plain::new(
        terminated,
        TimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::Normal,
    )
    .expect("a frame ending in the exported terminator must be admitted");
    assert_terminated("raw::Plain", accepted.bytes());

    let mut unterminated = terminated;
    let last = unterminated.len() - 1;
    unterminated[last] = VISCA_TERMINATOR.wrapping_sub(1);
    let rejected = raw::Plain::new(
        unterminated,
        TimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::Normal,
    );
    assert!(
        matches!(rejected, Err(Error::InvalidRequest(_))),
        "a frame not ending in the exported terminator must be rejected, got {rejected:?}"
    );
}

/// Both transport envelopes hand the encoded terminator to the wire unchanged:
/// raw VISCA passes the frame through, and Sony encapsulation prefixes its
/// 8-byte header without disturbing the payload's single trailing terminator.
#[test]
fn transport_envelopes_preserve_the_encoded_terminator() {
    let frame = encode(&PowerOn::new());
    assert_terminated("PowerOn", &frame);

    let raw_envelope = RawVisca::new(AddressingMode::Ip);
    let mut framed = BytesMut::new();
    let meta = raw_envelope.frame_into(&frame, CommandKind::Command, &mut framed);
    assert_eq!(meta.sequence, None, "raw VISCA carries no sequence number");
    assert_terminated("RawVisca frame", &framed);
    assert_eq!(
        &framed[..],
        &frame[..],
        "raw VISCA must pass the encoded frame through unchanged"
    );

    let sony_envelope = SonyEncapsulated::new(AddressingMode::Ip);
    let mut framed = BytesMut::new();
    let meta = sony_envelope.frame_into(&frame, CommandKind::Command, &mut framed);
    assert!(
        meta.sequence.is_some(),
        "Sony encapsulation must allocate a sequence number"
    );
    assert_eq!(
        framed.len(),
        SONY_HEADER_SIZE + frame.len(),
        "Sony encapsulation must prefix exactly one header"
    );
    assert_eq!(
        framed[framed.len() - 1],
        VISCA_TERMINATOR,
        "the Sony frame must still end with the exported terminator"
    );
    // The header carries a caller-independent sequence number, so the
    // exactly-once property is asserted over the VISCA payload it wraps.
    assert_terminated("SonyEncapsulated payload", &framed[SONY_HEADER_SIZE..]);
    assert_eq!(
        &framed[SONY_HEADER_SIZE..],
        &frame[..],
        "Sony encapsulation must not alter the encoded VISCA payload"
    );
}
