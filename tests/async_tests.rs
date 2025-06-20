#![allow(missing_docs)]
#![cfg(feature = "async")]

//! Tests for async functionality in the unified client.
//!
//! These tests verify async-specific behavior including concurrency,
//! timeouts, and proper error propagation through async chains.

mod common;

use common::MockAsyncTransport;
use grafton_visca::{
    command::{
        pan_tilt::PanTiltCommand,
        power::{Power, PowerCommand},
        zoom::ZoomCommand,
        InquiryCommand,
    },
    Error, Response,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn test_async_send_receive_basic() {
    let mock = MockAsyncTransport::new();
    mock.add_response(vec![0x90, 0x50, 0xFF]).await;

    let sent_commands = mock.sent_commands.clone();
    let mut transport = mock.into_visca_transport();

    let command = PowerCommand { power: Power::On };
    let response = transport.send_command(&command).await.unwrap();

    assert!(matches!(response, Response::Completion));

    let sent = sent_commands.lock().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
}

#[tokio::test]
async fn test_concurrent_commands() {
    // Test that multiple commands can be sent concurrently
    // Note: ViscaTransport handles its own internal state, so we need to use
    // a channel transport or similar for true concurrent access
    let mock = MockAsyncTransport::new();

    // Prepare responses for 3 commands
    mock.add_ack_completion(0).await;
    mock.add_ack_completion(1).await;
    mock.add_ack_completion(0).await; // Socket 0 reused after completion

    let command_counter = mock.command_counter.clone();
    let transport = Arc::new(Mutex::new(mock.into_visca_transport()));

    // Spawn 3 concurrent command tasks
    let t1 = transport.clone();
    let task1 = tokio::spawn(async move {
        let mut t = t1.lock().await;
        t.send_command(&PanTiltCommand::Home).await
    });

    let t2 = transport.clone();
    let task2 = tokio::spawn(async move {
        let mut t = t2.lock().await;
        t.send_command(&ZoomCommand::Stop).await
    });

    let t3 = transport.clone();
    let task3 = tokio::spawn(async move {
        let mut t = t3.lock().await;
        t.send_command(&PowerCommand { power: Power::On }).await
    });

    // All should complete successfully
    assert!(task1.await.unwrap().is_ok());
    assert!(task2.await.unwrap().is_ok());
    assert!(task3.await.unwrap().is_ok());

    // Verify all 3 commands were sent
    let count = *command_counter.lock().await;
    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_semaphore_limiting() {
    // Test that semaphore properly limits concurrent commands to 2
    let semaphore = Arc::new(Semaphore::new(2));

    // Track timing
    let start = tokio::time::Instant::now();

    // Try to send 3 commands with semaphore limiting to 2
    let mut tasks = vec![];

    for _ in 0..3 {
        let sem = semaphore.clone();

        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            // Create a new ViscaTransport for this task
            let mock = MockAsyncTransport::new().with_delay(50);
            mock.add_response(vec![0x90, 0x50, 0xFF]).await;
            let mut visca_transport = mock.into_visca_transport();

            let _response = visca_transport
                .send_command(&PowerCommand { power: Power::On })
                .await
                .unwrap();

            sleep(Duration::from_millis(100)).await; // Simulate command execution time
        });

        tasks.push(task);
    }

    // Wait for all to complete
    for task in tasks {
        task.await.unwrap();
    }

    let elapsed = start.elapsed();

    // With semaphore limiting to 2, the 3rd command should wait
    // Expected: 2 batches * 100ms = ~200ms (plus overhead)
    // Adding more tolerance for test timing
    assert!(elapsed >= Duration::from_millis(150)); // At least batch processing

    // On Windows CI runners, timing can be significantly slower due to virtualization
    // and resource constraints. Allow more time on Windows.
    #[cfg(target_os = "windows")]
    assert!(elapsed < Duration::from_millis(1000)); // Very generous timeout for Windows CI

    #[cfg(not(target_os = "windows"))]
    assert!(elapsed < Duration::from_millis(500)); // Normal timeout for other platforms
}

#[tokio::test]
async fn test_timeout_handling() {
    let mock = MockAsyncTransport::new().with_delay(100);
    // Don't add any response - should timeout
    let mut transport = mock.into_visca_transport();

    let command = PowerCommand { power: Power::On };

    // Test command timeout
    let result = timeout(Duration::from_millis(50), transport.send_command(&command)).await;

    assert!(result.is_err()); // Timeout from tokio

    // Test with longer timeout - should get Error::Timeout
    let result = timeout(Duration::from_millis(200), transport.send_command(&command)).await;

    assert!(result.is_ok()); // No tokio timeout
    assert!(matches!(result.unwrap(), Err(Error::Timeout))); // But VISCA timeout
}

#[tokio::test]
async fn test_error_propagation() {
    // Test that errors propagate correctly through async chains
    let mock = MockAsyncTransport::new().fail_after_n_commands(2);

    // Add responses for first two commands
    mock.add_response(vec![0x90, 0x50, 0xFF]).await;
    mock.add_response(vec![0x90, 0x50, 0xFF]).await;

    let mut transport = mock.into_visca_transport();

    // First two commands should succeed
    transport
        .send_command(&PowerCommand { power: Power::On })
        .await
        .unwrap();
    transport.send_command(&ZoomCommand::Stop).await.unwrap();

    // Third command should fail
    let result = transport.send_command(&PanTiltCommand::Home).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        Error::Io(e) => {
            assert_eq!(e.kind(), std::io::ErrorKind::ConnectionAborted);
        }
        _ => panic!("Expected IO error"),
    }
}

#[tokio::test]
async fn test_inquiry_async_handling() {
    let mock = MockAsyncTransport::new();

    // Add inquiry response (no ACK for inquiries)
    mock.add_response(vec![
        0x90, 0x50, // Header
        0x00, 0x01, 0x02, 0x03, // Pan
        0x04, 0x05, 0x06, 0x07, // Tilt
        0xFF,
    ])
    .await;

    let mut transport = mock.into_visca_transport();

    let response = transport
        .send_command(&InquiryCommand::PanTiltPosition)
        .await
        .unwrap();

    // Should get parsed pan/tilt position response
    match response {
        Response::InquiryResponse(_inquiry) => {
            // Inquiry responses are parsed correctly
        }
        _ => panic!("Expected inquiry response, got {:?}", response),
    }
}

#[tokio::test]
async fn test_concurrent_timeout_handling() {
    // Test that timeouts in concurrent operations don't affect each other
    let mock = MockAsyncTransport::new().with_delay(50);

    // Add response only for first command
    mock.add_response(vec![0x90, 0x50, 0xFF]).await;

    let transport = Arc::new(Mutex::new(mock.into_visca_transport()));

    let command1 = PowerCommand { power: Power::On };
    let command2 = PowerCommand {
        power: Power::Standby,
    };

    let t1 = transport.clone();
    let task1 = tokio::spawn(async move {
        let mut t = t1.lock().await;
        timeout(Duration::from_millis(100), t.send_command(&command1)).await
    });

    let t2 = transport.clone();
    let task2 = tokio::spawn(async move {
        let mut t = t2.lock().await;
        timeout(Duration::from_millis(100), t.send_command(&command2)).await
    });

    let (r1, r2) = tokio::join!(task1, task2);

    // First should succeed
    assert!(r1.unwrap().is_ok());

    // Second should timeout (no response available)
    let r2_result = r2.unwrap();
    assert!(r2_result.is_ok()); // tokio timeout didn't fire
    assert!(matches!(r2_result.unwrap(), Err(Error::Timeout)));
}

#[tokio::test]
async fn test_async_command_sequence() {
    // Test a realistic sequence of async commands
    let mock = MockAsyncTransport::new();

    // Prepare a sequence of responses
    mock.add_ack_completion(0).await; // Home
    mock.add_response(vec![
        0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
    ])
    .await; // Position inquiry
    mock.add_ack_completion(1).await; // Zoom

    let command_counter = mock.command_counter.clone();
    let mut transport = mock.into_visca_transport();

    // Execute command sequence
    let home_response = transport.send_command(&PanTiltCommand::Home).await.unwrap();
    assert!(matches!(home_response, Response::Completion));

    let pos_response = transport
        .send_command(&InquiryCommand::PanTiltPosition)
        .await
        .unwrap();
    match pos_response {
        Response::InquiryResponse(_) => {}
        _ => panic!("Expected inquiry response"),
    }

    let zoom_response = transport.send_command(&ZoomCommand::Stop).await.unwrap();
    assert!(matches!(zoom_response, Response::Completion));

    assert_eq!(*command_counter.lock().await, 3);
}

#[tokio::test]
async fn test_mock_transport_utilities() {
    // This test demonstrates the utility methods of MockAsyncTransport
    let mock = MockAsyncTransport::new().with_delay(5); // Use the with_delay builder method

    // Use add_response to queue a custom response
    mock.add_response(vec![0x90, 0x60, 0x02, 0xFF]).await; // Syntax error

    let command_counter = mock.command_counter.clone();
    let mut transport = mock.into_visca_transport();

    // Send a command and verify we get the error
    let command = PowerCommand { power: Power::On };
    let result = transport.send_command(&command).await;

    match result {
        Ok(Response::Error(_)) => {}
        _ => panic!("Expected error response, got {:?}", result),
    }

    // Verify command count
    assert_eq!(*command_counter.lock().await, 1);
}

#[tokio::test]
async fn test_mock_transport_ack_completion_helper() {
    // Demonstrates the add_ack_completion helper method
    let mock = MockAsyncTransport::new();

    // Use the helper to add both ACK and completion
    mock.add_ack_completion(0).await;

    let mut transport = mock.into_visca_transport();

    let command = ZoomCommand::Stop;
    let response = transport.send_command(&command).await.unwrap();

    // Should get completion response (ACK is handled internally)
    assert!(matches!(response, Response::Completion));
}
