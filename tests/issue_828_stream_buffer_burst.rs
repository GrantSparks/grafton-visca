//! #828: a valid stream burst must never overflow the receive limits.
//!
//! `BufferConfig { recv_buffer_size: 64, max_buffer_size: 64 }` passes
//! validation. The owner reads up to `recv_buffer_size` bytes at a time, so a
//! reply split across reads followed by one full read of valid replies used to
//! exceed the framer's 64-byte bound and poison a perfectly healthy raw stream.
//! The framer now always has room for one full read on top of the retention
//! bound.

#![cfg(feature = "blocking")]

use grafton_visca_test_support::fake_camera;

use grafton_visca::{
    blocking::{Session, SessionConfig},
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    transport::{AddressingMode, SendSemantics, TransportConfig},
};

use fake_camera::FakeCamera;

/// A raw TCP camera that answers the first write with a reply split across a
/// short read and one full `recv_buffer_size` read.
fn burst_camera(recv_buffer_size: usize) -> FakeCamera {
    let mut answered = false;
    FakeCamera::new(move |_, answer| {
        if !std::mem::replace(&mut answered, true) {
            // The ACK arrives split: two bytes now ...
            answer.reply(vec![0x90, 0x41]);
            // ... and its terminator leads one full 64-byte read that also
            // carries the completion and twenty Network Change notices.
            let mut burst = vec![0xff, 0x90, 0x51, 0xff];
            for _ in 0..20 {
                burst.extend_from_slice(&[0x90, 0x38, 0xff]);
            }
            assert_eq!(burst.len(), recv_buffer_size);
            answer.reply(burst);
        }
    })
}

#[test]
fn a_split_reply_followed_by_a_full_read_keeps_the_stream_running() {
    let mut config = TransportConfig::for_tcp();
    config.buffer_config.recv_buffer_size = 64;
    config.buffer_config.max_buffer_size = 64;
    let fake = burst_camera(config.buffer_config.recv_buffer_size);
    let wire = fake
        .blocking_wire()
        .with_config(config)
        .with_semantics(SendSemantics::Stream)
        .with_addressing(AddressingMode::Ip);

    let session = Session::open(
        wire,
        SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile")),
    )
    .expect("owner session");
    let camera = session.camera::<PtzOpticsG2>().expect("camera view");

    camera
        .power()
        .on()
        .expect("the burst completes the command instead of poisoning the stream");
    session.close().expect("clean close");
}
