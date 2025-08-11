//! Tests for runtime requirement enforcement.
//!
//! These tests verify that async operations properly require a runtime
//! and return appropriate errors when no runtime is configured.

#![cfg(feature = "async")]

use bytes::Bytes;
use grafton_visca::transport::Transport;
use std::future::Future;
use std::pin::Pin;

/// Mock transport that always succeeds but never actually sends/receives
#[derive(Debug)]
struct MockTransport;

impl Transport for MockTransport {
    type Error = std::io::Error;
    type SendFut<'a> = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>;
    type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<Bytes, Self::Error>> + Send + 'a>>;

    fn send<'a>(&'a self, _data: &'a [u8]) -> Self::SendFut<'a> {
        Box::pin(async { Ok(()) })
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        Box::pin(async {
            // Return a mock ACK response
            Ok(Bytes::from_static(&[0x90, 0x41, 0xFF]))
        })
    }
}

/// Mock transport that returns proper VISCA response sequences
#[derive(Debug)]
struct MockTransportWithResponses {
    responses: std::sync::Mutex<Vec<Vec<u8>>>,
    index: std::sync::Mutex<usize>,
}

impl MockTransportWithResponses {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            responses: std::sync::Mutex::new(vec![
                vec![0x90, 0x41, 0xFF], // ACK for first command
                vec![0x90, 0x51, 0xFF], // Completion for first command
                vec![0x90, 0x41, 0xFF], // ACK for second command
                vec![0x90, 0x51, 0xFF], // Completion for second command
            ]),
            index: std::sync::Mutex::new(0),
        }
    }
}

impl Transport for MockTransportWithResponses {
    type Error = std::io::Error;
    type SendFut<'a> = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>;
    type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<Bytes, Self::Error>> + Send + 'a>>;

    fn send<'a>(&'a self, _data: &'a [u8]) -> Self::SendFut<'a> {
        Box::pin(async { Ok(()) })
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        Box::pin(async move {
            let mut index = self.index.lock().unwrap();
            let responses = self.responses.lock().unwrap();

            if *index < responses.len() {
                let response = responses[*index].clone();
                *index += 1;
                Ok(Bytes::from(response))
            } else {
                // Return completion if we run out of responses
                Ok(Bytes::from_static(&[0x90, 0x51, 0xFF]))
            }
        })
    }
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn test_operations_fail_without_runtime() {
    use grafton_visca::{
        camera::{profiles::PTZOpticsG2, Camera},
        prelude::r#async::*,
        Error,
    };

    // Create camera without runtime
    let transport = MockTransport;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);
    // Note: NOT calling with_runtime()

    // Power operations should fail
    let result = camera.power_on().await;
    assert!(matches!(result, Err(Error::InvalidState(_))));
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No runtime configured"));

    // Pan/tilt operations should fail
    let result = camera.pan_tilt_home().await;
    assert!(matches!(result, Err(Error::InvalidState(_))));

    // Zoom operations should fail
    let result = camera.zoom_stop().await;
    assert!(matches!(result, Err(Error::InvalidState(_))));

    // Movement detection should fail
    let result = camera.await_idle(std::time::Duration::from_secs(1)).await;
    assert!(matches!(result, Err(Error::InvalidState(_))));
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn test_operations_succeed_with_runtime() {
    use grafton_visca::{
        camera::{profiles::PTZOpticsG2, Camera},
        prelude::r#async::*,
        runtime::{SharedRuntime, TokioRuntime},
    };
    use std::sync::Arc;

    // Create a better mock transport that returns proper VISCA responses
    let transport = MockTransportWithResponses::new();
    let runtime: SharedRuntime = Arc::new(TokioRuntime);
    let camera = Camera::<PTZOpticsG2, _>::new(transport).with_runtime_only(runtime);

    // Operations should succeed with proper mock responses
    let result = camera.zoom_stop().await;
    // Should succeed now
    assert!(result.is_ok());
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn test_custom_runtime_works() {
    use grafton_visca::{
        camera::{profiles::PTZOpticsG2, Camera},
        executor::{Sleep, SpawnableFuture, Spawner},
        prelude::r#async::*,
        runtime::{GenericRuntime, SharedRuntime},
    };
    use std::{pin::Pin, sync::Arc, time::Duration};

    // Create a custom runtime using tokio components
    #[derive(Debug, Clone)]
    struct TokioSleep;

    impl Sleep for TokioSleep {
        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            Box::pin(tokio::time::sleep(duration))
        }
    }

    #[derive(Debug, Clone)]
    struct TokioSpawner;

    impl Spawner for TokioSpawner {
        fn spawn(&self, task: SpawnableFuture) {
            tokio::spawn(task);
        }
    }

    // Create camera with custom runtime
    let transport = MockTransportWithResponses::new();
    let runtime: SharedRuntime = Arc::new(GenericRuntime::new(TokioSleep, TokioSpawner));
    let camera = Camera::<PTZOpticsG2, _>::new(transport).with_runtime_only(runtime);

    // Operations should succeed
    let result = camera.zoom_stop().await;
    // Should succeed with proper mock
    assert!(result.is_ok());
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn test_movement_detection_requires_runtime() {
    use grafton_visca::{
        camera::{profiles::PTZOpticsG2, Camera},
        prelude::r#async::*,
        Error,
    };

    // Create camera without runtime
    let transport = MockTransport;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Movement detection should fail without runtime
    // Using await_idle which internally uses movement detection
    let result = camera
        .await_idle(std::time::Duration::from_millis(100))
        .await;
    assert!(matches!(result, Err(Error::InvalidState(_))));
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No runtime configured"));
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn test_power_delays_require_runtime() {
    use grafton_visca::{
        camera::{profiles::PTZOpticsG2, Camera},
        prelude::r#async::*,
        Error,
    };

    // Create camera without runtime
    let transport = MockTransport;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Power on should fail due to missing runtime for delay
    let result = camera.power_on().await;
    assert!(matches!(result, Err(Error::InvalidState(_))));

    // Power off should also fail
    let result = camera.power_off().await;
    assert!(matches!(result, Err(Error::InvalidState(_))));
}
