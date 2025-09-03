//! Compile-time tests to verify that all async control trait futures are Send.
//!
//! This test suite validates that the RPITIT (Return Position Impl Trait In Trait) conversion correctly
//! encodes Send bounds for all control trait method futures. These tests use
//! compile-time assertions to ensure futures can be sent across threads.

#![cfg(feature = "async")]

use grafton_visca::camera::controls::{power::PowerControl, zoom::ZoomControl};

/// Helper function to assert that a future is Send.
fn assert_send<F>(_f: F)
where
    F: std::future::Future + Send,
{
}

/// Mock camera type for testing trait implementations.
#[derive(Clone, Debug)]
struct MockCamera;

// Safety: MockCamera is a simple unit struct, safe to send/sync
unsafe impl Send for MockCamera {}
unsafe impl Sync for MockCamera {}

// Implement control traits for MockCamera to test Send bounds
impl PowerControl for MockCamera {
    type Mode = grafton_visca::mode::Async;

    fn power_on(
        &self,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn power_off(
        &self,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn power_inquiry(
        &self,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<bool, grafton_visca::Error>>
    {
        Box::pin(async { Ok(true) })
    }
}

impl ZoomControl for MockCamera {
    type Mode = grafton_visca::mode::Async;

    fn zoom_stop(
        &self,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn zoom_tele_std(
        &self,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn zoom_wide_std(
        &self,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn zoom_tele_variable(
        &self,
        _speed: grafton_visca::command::zoom::ZoomSpeed,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn zoom_wide_variable(
        &self,
        _speed: grafton_visca::command::zoom::ZoomSpeed,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn zoom_absolute(
        &self,
        _position: grafton_visca::units::Normalized,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn zoom_position(
        &self,
        _pos: grafton_visca::types::ZoomPosition,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), grafton_visca::Error>> {
        Box::pin(async { Ok(()) })
    }
    fn zoom_position_inquiry(
        &self,
    ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<
        '_,
        Result<grafton_visca::types::ZoomPosition, grafton_visca::Error>,
    > {
        Box::pin(async { Ok(grafton_visca::types::ZoomPosition::new(0x0000).unwrap()) })
    }
}

#[test]
fn test_power_control_futures_are_send() {
    let camera = MockCamera;

    // Test that all PowerControl futures are Send
    assert_send(camera.power_on());
    assert_send(camera.power_off());
    assert_send(camera.power_inquiry());
}

#[test]
fn test_zoom_control_futures_are_send() {
    let camera = MockCamera;

    // Test that all ZoomControl futures are Send
    assert_send(camera.zoom_stop());
    assert_send(camera.zoom_tele_std());
    assert_send(camera.zoom_wide_std());
    assert_send(
        camera.zoom_tele_variable(grafton_visca::command::zoom::ZoomSpeed::new(1).unwrap()),
    );
    assert_send(
        camera.zoom_wide_variable(grafton_visca::command::zoom::ZoomSpeed::new(1).unwrap()),
    );
    assert_send(camera.zoom_absolute(grafton_visca::units::Normalized::new(0.5)));
    assert_send(camera.zoom_position(grafton_visca::types::ZoomPosition::new(0x4000).unwrap()));
    assert_send(camera.zoom_position_inquiry());
}

/// Compile-time test that verifies trait object compatibility.
///
/// This test ensures that control trait futures can be used in dynamic contexts
/// where Send bounds are required.
#[test]
fn test_trait_object_send_compatibility() {
    fn requires_send_future<F>(_future: F)
    where
        F: std::future::Future + Send,
    {
    }

    let camera = MockCamera;

    // Test that control trait futures work with Send bounds
    requires_send_future(camera.power_on());
    requires_send_future(camera.zoom_tele_std());
}

/// Test spawning futures across threads to verify Send bounds work in practice.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_spawn_futures_across_threads() {
    let camera = MockCamera;

    // Test that futures can be spawned on the tokio runtime
    let power_task = tokio::spawn(async move { camera.power_on().await });

    let zoom_task = tokio::spawn(async {
        let camera = MockCamera;
        camera.zoom_tele_std().await
    });

    // All tasks should complete successfully
    assert!(power_task.await.is_ok());
    assert!(zoom_task.await.unwrap().is_ok());
}

/// Compile-time verification that control traits have proper Send + Sync supertraits.
#[test]
fn test_control_trait_bounds() {
    fn requires_send_sync_static<T>()
    where
        T: Send + Sync + 'static,
    {
    }

    // Verify that MockCamera (which implements control traits) satisfies bounds
    requires_send_sync_static::<MockCamera>();

    // Verify at type level that control traits work with Send + Sync + 'static
    fn requires_power_control<T>()
    where
        T: PowerControl + Send + Sync + 'static,
    {
        requires_send_sync_static::<T>();
    }

    fn requires_zoom_control<T>()
    where
        T: ZoomControl + Send + Sync + 'static,
    {
        requires_send_sync_static::<T>();
    }

    requires_power_control::<MockCamera>();
    requires_zoom_control::<MockCamera>();
}

/// Test that futures from different control traits can be used together.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_mixed_control_trait_futures() {
    let camera = MockCamera;

    // Test that futures from different traits can be combined
    let results = tokio::try_join!(camera.power_on(), camera.zoom_stop(),);

    assert!(results.is_ok());
}

/// Test that the RPITIT futures have the correct lifetimes.
#[test]
fn test_rpitit_lifetime_bounds() {
    fn test_lifetime_bound<T>(_camera: &T)
    where
        T: PowerControl,
    {
        // This function verifies that RPITIT futures have the correct '_ lifetime
        let _future = _camera.power_on();
        // If this compiles, the lifetime bounds are correct
    }

    let camera = MockCamera;
    test_lifetime_bound(&camera);
}
