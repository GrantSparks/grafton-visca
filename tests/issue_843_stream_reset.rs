//! A stream connection reset still ends its owner session.
#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
#![allow(clippy::expect_used, clippy::unwrap_used)]
use grafton_visca::{operation::AppliedOnly, transport::SendSemantics, Error};
use std::{io::ErrorKind, sync::Arc};

struct StreamCamera(grafton_visca_test_support::fake_camera::FakeCamera);

impl StreamCamera {
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> grafton_visca_test_support::fake_camera::BlockingWire {
        self.0.blocking_wire().with_semantics(SendSemantics::Stream)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> grafton_visca_test_support::fake_camera::AsyncWire {
        self.0.async_wire().with_semantics(SendSemantics::Stream)
    }
}

grafton_visca_test_support::facade_matrix! {
    fn a_stream_connection_reset_still_ends_the_session() {
        use grafton_visca_test_support::{
            fake_camera::FakeCamera,
            profile_fixtures::{session_config, NonDefaultCompileTimeProfile},
        };

        let reset = StreamCamera(FakeCamera::new(|_, answer| {
            answer.fault(Error::Io { source: Arc::new(ErrorKind::ConnectionReset.into()) });
        }));
        let session = open!(reset, session_config()).unwrap();
        let camera = session.camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::CameraId::CAMERA_1).unwrap();
        let error = wait!(wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).unwrap().applied())
            .expect_err("a connection reset remains session death outside the Windows UDP seam");
        assert!(error.requires_new_session(), "{error:?}");
    }
}
