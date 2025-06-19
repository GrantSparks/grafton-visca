//! Tests for camera concurrency control with semaphore and session management.

mod common;

#[cfg(feature = "async-client")]
use std::sync::Arc;

#[cfg(feature = "async-client")]
use std::time::Duration;

#[cfg(feature = "async-client")]
use grafton_visca::{
    camera::{Camera, GenericVisca},
    command::inquiry::InquiryCommand,
    Result,
};

#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
use grafton_visca::{
    camera::{Camera, GenericVisca},
    transport::BlockingAdapter,
    Result,
};

#[cfg(feature = "async-client")]
use common::MockAsyncTransport;

#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
use common::MockTransport;

#[cfg(feature = "async-client")]
#[tokio::test]
async fn test_camera_readiness_check() -> Result<()> {
    // Create a mock transport for testing
    let mock_transport = MockAsyncTransport::new();
    mock_transport.add_ack_completion(0).await;
    let camera = Arc::new(Camera::<GenericVisca>::new(mock_transport));

    // Initially should be ready
    assert!(camera.is_ready());
    assert_eq!(camera.pending_commands(), 0);

    Ok(())
}

#[cfg(feature = "async-client")]
#[tokio::test]
async fn test_concurrent_command_limit() -> Result<()> {
    // Create a mock transport for testing
    let mock_transport = MockAsyncTransport::new();
    // Pre-populate responses for multiple concurrent commands
    for i in 0..4 {
        mock_transport.add_ack_completion(i as u8 % 2).await;
    }
    let camera = Arc::new(Camera::<GenericVisca>::new(mock_transport));

    // Send two commands to fill both sockets
    let cam1 = camera.clone();
    let cam2 = camera.clone();

    let handle1 =
        tokio::spawn(async move { cam1.send_raw_async(&InquiryCommand::PanTiltPosition).await });

    let handle2 =
        tokio::spawn(async move { cam2.send_raw_async(&InquiryCommand::ZoomPosition).await });

    // Give commands time to start
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Camera should not be ready when both sockets are in use
    assert!(!camera.is_ready());
    assert_eq!(camera.pending_commands(), 2);

    // Wait for the first two commands to complete
    let _ = handle1.await;
    let _ = handle2.await;

    // Now camera should be ready again
    assert!(camera.is_ready());
    assert_eq!(camera.pending_commands(), 0);

    Ok(())
}

#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
#[test]
fn test_blocking_camera_readiness() -> Result<()> {
    // Create a mock transport for testing
    let mock_transport = MockTransport::with_ack_completion();
    let camera = Camera::<GenericVisca>::new(BlockingAdapter(mock_transport));

    // Initially should be ready
    assert!(camera.is_ready());
    assert_eq!(camera.pending_commands(), 0);

    Ok(())
}
