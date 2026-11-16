//! Bounded entry points used by the out-of-tree `cargo-fuzz` package.
//!
//! These wrappers keep production protocol modules crate-private while letting
//! the fuzz package exercise the same implementations used by owners.

use bytes::{Bytes, BytesMut};

use crate::{
    command::{parse_inquiry_payload, CommandKind, Response},
    protocol::{
        framer::{Framer, RawFramer, SonyFramer},
        sony::{PayloadType, SonyHeader},
    },
    raw::PlainRequest,
    runtime::owner::{decode_response_target, RoutingState, TargetRegistry},
    target::TargetSlot,
    transport::{AddressingMode, BufferConfig, Envelope, SonyEncapsulated},
    CommandTimeoutClass, RetryClass, SubmissionClass,
};

/// Exercise raw admission and every generated built-in inquiry decoder.
pub fn response_parser(bytes: &[u8]) {
    let _ = PlainRequest::new(
        bytes,
        CommandTimeoutClass::Quick,
        RetryClass::Never,
        SubmissionClass::Normal,
    );
    let _ = Response::parse(bytes);
    for inquiry in crate::command::inquiry_structs::FUZZ_INQUIRY_KINDS {
        let _ = Response::parse_with_type(bytes, inquiry);
        let _ = parse_inquiry_payload(bytes, inquiry);
    }
}

/// Exercise raw and Sony framing in whole-buffer and split-read forms.
pub fn protocol_framer(bytes: &[u8]) {
    let config = BufferConfig {
        recv_buffer_size: 1_024,
        max_buffer_size: 8_192,
    };
    let split = bytes.len() / 2;
    drive_framer(RawFramer::new(config), &[bytes]);
    drive_framer(SonyFramer::new(config), &[bytes]);
    drive_framer(RawFramer::new(config), &[&bytes[..split], &bytes[split..]]);
    drive_framer(SonyFramer::new(config), &[&bytes[..split], &bytes[split..]]);

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
    drive_framer(SonyFramer::new(config), &[&framed]);
}

fn drive_framer(mut framer: impl Framer, chunks: &[&[u8]]) {
    for chunk in chunks {
        if framer.push_slice(chunk).is_err() {
            return;
        }
        while framer.visit_next_frame(|_| ()).is_some() {}
        assert!(framer.buffered_len() <= framer.max_buffer_size());
    }
}

/// Exercise Sony request framing and both public and owner response extraction.
pub fn sony_envelope(bytes: &[u8]) {
    let envelope = SonyEncapsulated::new(AddressingMode::Ip);
    let _ = envelope.extract_response(bytes);
    let _ = envelope.extract_with_meta(Bytes::copy_from_slice(bytes));
    let _ = envelope.extract_owner_response(bytes);

    let mut out = BytesMut::new();
    let _ = envelope.frame_into(bytes, CommandKind::Command, &mut out);
    let _ = envelope.frame_into(bytes, CommandKind::Inquiry, &mut out);

    let middle_len = bytes.len().min(13);
    let mut visca = Vec::with_capacity(middle_len + 2);
    visca.push(0x90);
    visca.extend_from_slice(&bytes[..middle_len]);
    visca.push(0xFF);
    let framed = sony_frame(PayloadType::ViscaReply, sequence_from(bytes), &visca);
    let _ = envelope.extract_owner_response(&framed);

    let control = match bytes {
        [first, second, ..] => vec![*first, *second],
        [first] => vec![*first],
        [] => vec![0x01],
    };
    let framed = sony_frame(PayloadType::ControlReply, sequence_from(bytes), &control);
    let _ = envelope.extract_owner_response(&framed);

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
    let all_targets =
        TargetRegistry::from_registry(&crate::target::PerTarget::from_fn(|_| Some(())));
    let single_target =
        TargetRegistry::from_registry(&crate::target::PerTarget::from_fn(|target| {
            (target == TargetSlot::ALL[0]).then_some(())
        }));
    let serial = RoutingState::new(AddressingMode::Serial, all_targets);
    let single_ip = RoutingState::new(AddressingMode::Ip, single_target);
    let ambiguous_ip = RoutingState::new(AddressingMode::Ip, all_targets);

    for payload in [bytes, bytes.get(..1).unwrap_or(bytes)] {
        let _ = decode_response_target(serial, payload);
        let _ = decode_response_target(single_ip, payload);
        let _ = decode_response_target(ambiguous_ip, payload);
    }
}

/// Exercise serial startup over arbitrary bus chunks, read failures and ticks.
///
/// Each run uses finite protocol budgets and bounded reads, checks retention
/// after every action, and requires the startup to finish within a finite turn
/// budget. One pass always writes Address Set so raw reply seeds reach parsing;
/// the other also injects write failures.
#[cfg(feature = "transport-serial")]
pub fn serial_startup(bytes: &[u8]) {
    for inject_writes in [false, true] {
        assert!(
            drive_serial_startup(bytes, inject_writes).is_some(),
            "serial startup exceeded its bounded turn budget"
        );
    }
}

#[cfg(feature = "transport-serial")]
fn drive_serial_startup(bytes: &[u8], inject_writes: bool) -> Option<crate::Result<Option<u8>>> {
    use crate::transport::{
        serial::handshake::{Action, SerialStartup as StartupMachine, StartupTiming},
        SerialConfig, SerialStartup,
    };
    use std::{
        io::ErrorKind,
        time::{Duration, Instant},
    };

    let buffers = BufferConfig {
        recv_buffer_size: 64,
        max_buffer_size: 128,
    };
    let config = SerialConfig::new("fuzz")
        .with_startup(
            SerialStartup::default()
                .with_address_set(true)
                .with_interface_clear(true),
        )
        .with_read_timeout(Duration::from_millis(2))
        .with_write_timeout(Duration::from_millis(2))
        .with_buffer_config(buffers);
    let timing = StartupTiming {
        address_set_attempt: Duration::from_millis(10),
        address_set_attempts: 3,
        address_set_retry_delay: Duration::from_millis(1),
        idle_pause: Duration::from_millis(1),
        if_clear_operation: Duration::from_millis(10),
        if_clear_settle: Duration::from_millis(2),
    };
    let mut startup = match StartupMachine::new(&config, timing) {
        Ok(startup) => startup,
        Err(error) => return Some(Err(error)),
    };
    let mut now = Instant::now();
    let mut cursor = 0;
    for turn in 0..256 {
        assert!(startup.retained_len() <= buffers.framer_capacity());
        let control = bytes.get(turn % bytes.len().max(1)).copied().unwrap_or(0);
        let selector = if inject_writes {
            control.rotate_left(1)
        } else {
            control
        };
        match startup.next(now) {
            Action::Write { frame, .. } => {
                assert!(frame.len() <= startup.read_capacity());
                let written = if inject_writes && selector & 0x80 != 0 {
                    Err(crate::Error::io_timeout())
                } else {
                    Ok(())
                };
                startup.wrote(written);
            }
            Action::Read { .. } => {
                let count = (1 + usize::from(selector) % startup.read_capacity())
                    .min(bytes.len().saturating_sub(cursor));
                let end = cursor + count;
                match selector & 0x0F {
                    0x0C => startup.read(Err(crate::Error::io_timeout())),
                    0x0D => startup.read(Ok(&[])),
                    0x0E | 0x0F => startup.read(Err(std::io::Error::new(
                        ErrorKind::BrokenPipe,
                        "fuzz read failure",
                    )
                    .into())),
                    _ => {
                        startup.read(Ok(&bytes[cursor..end]));
                        cursor = end;
                    }
                }
            }
            Action::Sleep(duration) => now += duration,
            Action::DiscardInput => {}
            Action::Done(result) => {
                assert!(startup.retained_len() <= buffers.framer_capacity());
                return Some(result.map(|()| startup.addressed_cameras()));
            }
        }
        assert!(startup.retained_len() <= buffers.framer_capacity());
        // Even an immediately-ready read samples a later instant eventually.
        now += Duration::from_micros(1) + Duration::from_millis(u64::from(selector >> 6));
    }
    None
}

#[cfg(all(test, feature = "transport-serial"))]
mod serial_startup_tests {
    use super::{drive_serial_startup, serial_startup};

    const ADDRESS: &[u8] = &[0x88, 0x30, 0x02, 0xff];
    const NETWORK: &[u8] = &[0x90, 0x38, 0xff];
    const OVERSIZED: &[u8] = &{
        let mut bytes = [0x31; 258];
        bytes[257] = 0xff;
        bytes
    };
    const ECHO: &[u8] = &[0x88, 0x30, 0x01, 0xff, 0x88, 0x30, 0x02, 0xff];

    #[test]
    fn seed_corpus_reaches_serial_startup_on_stable() {
        for seed in [ADDRESS, NETWORK, OVERSIZED, ECHO, &[]] {
            serial_startup(seed);
        }
        assert!(matches!(
            drive_serial_startup(ADDRESS, false),
            Some(Ok(Some(1)))
        ));
        assert!(matches!(
            drive_serial_startup(ECHO, false),
            Some(Ok(Some(1)))
        ));
    }
}
