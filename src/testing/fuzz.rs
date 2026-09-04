//! Bounded entry points used by the out-of-tree `cargo-fuzz` package.
//!
//! These wrappers keep production protocol modules crate-private while letting
//! the fuzz package exercise the same implementations used by owners.

use bytes::{Bytes, BytesMut};

use crate::{
    command::{parse_inquiry_payload, CommandKind, Response},
    protocol::{
        framer::{FramingMode, ProtocolFramer},
        sony::{PayloadType, SonyHeader},
    },
    raw::Plain,
    runtime::owner::{decode_response_target, RoutingState, TargetRegistry},
    transport::{AddressingMode, BufferConfig, Envelope, SonyEncapsulated},
    CameraId, ControlClass, RetryClass, TimeoutClass,
};

/// Exercise raw admission and every generated built-in inquiry decoder.
pub fn response_parser(bytes: &[u8]) {
    let _ = Plain::new(
        bytes,
        TimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::Normal,
    );
    let _ = Response::parse(bytes);
    for inquiry in crate::command::inquiry_structs::FUZZ_INQUIRY_KINDS {
        let _ = Response::parse_with_type(bytes, inquiry);
        let _ = parse_inquiry_payload(bytes, inquiry);
    }
}

/// Exercise raw and Sony framing in whole-buffer and split-read forms.
pub fn protocol_framer(bytes: &[u8]) {
    for mode in [FramingMode::RawVisca, FramingMode::SonyEncapsulated] {
        drive_framer(mode, &[bytes]);
        let split = bytes.len() / 2;
        drive_framer(mode, &[&bytes[..split], &bytes[split..]]);
    }

    // Always drive a structurally valid Sony boundary whose payload is still
    // controlled by the fuzzer, so short runs get past the eight-byte header.
    let payload_len = bytes.len().min(32);
    let header = SonyHeader {
        payload_type: PayloadType::ViscaReply,
        payload_length: payload_len as u16,
        sequence_number: sequence_from(bytes),
    };
    let mut framed = header.encode().to_vec();
    framed.extend_from_slice(&bytes[..payload_len]);
    drive_framer(FramingMode::SonyEncapsulated, &[&framed]);
}

fn drive_framer(mode: FramingMode, chunks: &[&[u8]]) {
    let config = BufferConfig {
        recv_buffer_size: 1_024,
        max_buffer_size: 8_192,
    };
    let mut framer = ProtocolFramer::new_with_config_and_mode(config, mode);
    for chunk in chunks {
        if framer.push_slice(chunk).is_err() {
            return;
        }
        for frame in framer.drain_frames() {
            let _ = frame;
        }
    }
}

/// Exercise Sony request framing and both public and owner response extraction.
pub fn sony_envelope(bytes: &[u8]) {
    let envelope = SonyEncapsulated::new(AddressingMode::Ip);
    let _ = envelope.extract_response(bytes);
    let _ = envelope.extract_with_meta(Bytes::copy_from_slice(bytes));
    let _ = envelope.extract_owner_response(Bytes::copy_from_slice(bytes));

    let mut out = BytesMut::new();
    let _ = envelope.frame_into(bytes, CommandKind::Command, &mut out);
    let _ = envelope.frame_into(bytes, CommandKind::Inquiry, &mut out);

    let middle_len = bytes.len().min(13);
    let mut visca = Vec::with_capacity(middle_len + 2);
    visca.push(0x90);
    visca.extend_from_slice(&bytes[..middle_len]);
    visca.push(0xFF);
    let framed = sony_frame(PayloadType::ViscaReply, sequence_from(bytes), &visca);
    let _ = envelope.extract_owner_response(Bytes::from(framed));

    let control = match bytes {
        [first, second, ..] => vec![*first, *second],
        [first] => vec![*first],
        [] => vec![0x01],
    };
    let framed = sony_frame(PayloadType::ControlReply, sequence_from(bytes), &control);
    let _ = envelope.extract_owner_response(Bytes::from(framed));

    envelope.frame_sequence_reset_into(&mut out);
}

fn sony_frame(payload_type: PayloadType, sequence_number: u32, payload: &[u8]) -> Vec<u8> {
    let header = SonyHeader {
        payload_type,
        payload_length: payload.len() as u16,
        sequence_number,
    };
    let mut framed = header.encode().to_vec();
    framed.extend_from_slice(payload);
    framed
}

fn sequence_from(bytes: &[u8]) -> u32 {
    let mut sequence = [0_u8; 4];
    let count = bytes.len().min(sequence.len());
    sequence[..count].copy_from_slice(&bytes[..count]);
    u32::from_be_bytes(sequence)
}

/// Exercise strict serial and bounded IP response-source routing.
pub fn response_target(bytes: &[u8]) {
    let Ok(all_targets) = TargetRegistry::from_targets(&[
        CameraId::CAMERA_1,
        CameraId::CAMERA_2,
        CameraId::CAMERA_3,
        CameraId::CAMERA_4,
        CameraId::CAMERA_5,
        CameraId::CAMERA_6,
        CameraId::CAMERA_7,
    ]) else {
        return;
    };
    let Ok(single_target) = TargetRegistry::from_targets(&[CameraId::CAMERA_1]) else {
        return;
    };
    let serial = RoutingState::new(AddressingMode::Serial, all_targets);
    let single_ip = RoutingState::new(AddressingMode::Ip, single_target);
    let ambiguous_ip = RoutingState::new(AddressingMode::Ip, all_targets);

    for payload in [bytes, bytes.get(..1).unwrap_or(bytes)] {
        let _ = decode_response_target(serial, payload);
        let _ = decode_response_target(single_ip, payload);
        let _ = decode_response_target(ambiguous_ip, payload);
    }
}
