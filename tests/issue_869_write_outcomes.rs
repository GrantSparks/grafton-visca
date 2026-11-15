//! Failed physical sends and pre-admission refusals have distinct outcomes.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use grafton_visca::{
    completion::AppliedOnly, profiles::SonyFR7, raw, request::builtin::ZoomStop,
    transport::SendSemantics, ControlClass, Error, ErrorKind, RetryClass, SessionConfig,
    TimeoutClass,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, frames, FakeCamera},
};

struct Wire {
    peer: FakeCamera,
    semantics: SendSemantics,
}

impl Wire {
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.peer.blocking_wire().with_semantics(self.semantics)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.peer.async_wire().with_semantics(self.semantics)
    }
}

fn io_cause(error: &Error) -> Option<io::ErrorKind> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = current {
        if let Some(cause) = error.downcast_ref::<io::Error>() {
            return Some(cause.kind());
        }
        if let Some(cause) = error.downcast_ref::<Arc<io::Error>>() {
            return Some(cause.kind());
        }
        if let Some(cause) = error.downcast_ref::<Arc<Error>>() {
            current = Some(cause.as_ref());
            continue;
        }
        current = error.source();
    }
    None
}

facade_matrix! {
    fn every_failed_datagram_send_keeps_its_os_cause_and_the_session_usable() {
        for kind in [io::ErrorKind::ConnectionRefused, io::ErrorKind::BrokenPipe,
            io::ErrorKind::NotConnected, io::ErrorKind::Other] {
            let first = Arc::new(AtomicBool::new(true));
            let wire = Wire {
                semantics: SendSemantics::Datagram,
                peer: FakeCamera::visca(move |bytes, answer| {
                    if bytes.first() != Some(&0x81) { return; }
                    if first.swap(false, Ordering::Relaxed) {
                        answer.fail_send(io::Error::from(kind).into());
                    } else {
                        answer.reply(frames::ack(1)).reply(frames::complete(1));
                    }
                }),
            };
            let session = open!(wire, SessionConfig::from_compile_time::<SonyFR7>().unwrap()).unwrap();
            let camera = session.camera::<SonyFR7>().unwrap();
            let mut first = wait!(camera.submit::<AppliedOnly, _>(&ZoomStop)).unwrap();
            let error = wait!(first.applied()).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Transport, "{kind:?}: {error:?}");
            assert!(!error.requires_new_session());
            assert_eq!(io_cause(&error), Some(kind));
            let mut next = wait!(camera.submit::<AppliedOnly, _>(&ZoomStop)).unwrap();
            wait!(next.applied()).unwrap();
            let metrics = wait!(session.metrics()).unwrap();
            assert_eq!(metrics.writes, 2);
            assert_eq!(metrics.write_failures, 1);
            wait!(session.close()).unwrap();
        }
    }

    fn an_unframeable_request_fails_before_admission_on_stream_and_datagram() {
        for semantics in [SendSemantics::Stream, SendSemantics::Datagram] {
            let wire = Wire {
                semantics,
                peer: FakeCamera::visca(|bytes, answer| {
                    if bytes.first() == Some(&0x81) {
                        answer.reply(frames::ack(1)).reply(frames::complete(1));
                    }
                }),
            };
            let session = open!(wire, SessionConfig::from_compile_time::<SonyFR7>().unwrap()).unwrap();
            let initial_writes = wire.peer.write_count();
            let camera = session.camera::<SonyFR7>().unwrap();
            // FR7's sourced five-nibble positions need 17 payload bytes;
            // this opaque 18-byte request exceeds its actual envelope bound.
            let mut bytes = vec![0; 18];
            bytes[..4].copy_from_slice(&[0x81, 0x01, 0x04, 0x99]);
            bytes[17] = 0xff;
            let request = raw::Plain::new(bytes, TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal).unwrap();
            assert!(matches!(wait!(camera.execute(&request)), Err(Error::InvalidRequest(_))));
            let metrics = wait!(session.metrics()).unwrap();
            assert_eq!(metrics.admitted, 0);
            assert_eq!(metrics.writes, 0);
            assert_eq!(metrics.write_failures, 0);
            assert_eq!(wire.peer.write_count(), initial_writes);
            let mut next = wait!(camera.submit::<AppliedOnly, _>(&ZoomStop)).unwrap();
            wait!(next.applied()).unwrap();
            wait!(session.close()).unwrap();
        }
    }
}
