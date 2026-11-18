//! Runtime-neutral caller-owned session construction.
//!
//! This runs in every async-enabled matrix leg. It proves that a
//! consumer-defined [`Executor`] can start and close a public [`Session`] over
//! a caller-owned [`AsyncTransport`], whether or not one of the crate's runtime
//! adapters is also enabled.

#![cfg(feature = "async")]

use grafton_visca_test_support::fake_camera;

use std::{future::Future, pin::Pin, time::Duration};

use grafton_visca::{
    config::SessionConfig,
    profiles::{ProfileSpec, PtzOpticsG2},
    r#async::Session,
    runtime::Executor,
    transport::AddressingMode,
};

use fake_camera::FakeCamera;

/// A small caller-owned executor that needs no crate runtime feature.
#[derive(Clone, Copy, Debug)]
struct ThreadExecutor;

struct ThreadTimer(async_io::Timer);

impl Future for ThreadTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        Pin::new(&mut self.get_mut().0).poll(context).map(|_| ())
    }
}

impl grafton_visca::runtime::ResettableTimer for ThreadTimer {
    fn reset(self: Pin<&mut Self>, deadline: std::time::Instant) {
        self.get_mut().0.set_at(deadline);
    }
}

impl Executor for ThreadExecutor {
    type Timer = ThreadTimer;

    fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
        ThreadTimer(async_io::Timer::at(deadline))
    }

    fn spawn_detached<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        std::thread::spawn(move || pollster::block_on(future));
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        async move {
            async_io::Timer::after(duration).await;
        }
    }
}

#[test]
fn caller_owned_session_opens_and_closes_on_a_custom_executor() {
    let executor = ThreadExecutor;
    pollster::block_on(async move {
        let expected_profile = ProfileSpec::from_compile_time::<PtzOpticsG2>()?;
        let session = Session::open(
            FakeCamera::silent()
                .async_wire()
                .with_addressing(AddressingMode::Ip),
            &SessionConfig::from_compile_time::<PtzOpticsG2>()?,
            executor,
        )
        .await?;
        let camera =
            session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)?;
        assert_eq!(
            camera.target(),
            grafton_visca::types::IndividualCameraId::CAMERA_1
        );
        assert_eq!(camera.profile(), &expected_profile);
        session.close().await
    })
    .expect("runtime-neutral caller-owned session closes");
}
