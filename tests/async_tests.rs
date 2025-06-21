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
use tokio::sync::Semaphore;
use tokio::time::sleep;

#[tokio::test]
async fn test_async_send_receive_basic() {
    let mock = MockAsyncTransport::new();
    mock.add_response(vec![0x90, 0x50, 0xFF]).await;

    let sent_commands = mock.sent_commands.clone();
    let transport = mock.into_visca_transport();

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
    // Since commands are serialized by the Mutex, they'll likely all use socket 0
    mock.add_ack_completion(0).await; // First command
    mock.add_ack_completion(0).await; // Second command
    mock.add_ack_completion(0).await; // Third command

    let command_counter = mock.command_counter.clone();
    let transport = mock.into_visca_transport();

    // Spawn 3 concurrent command tasks
    let t1 = transport.clone();
    let task1 = tokio::spawn(async move { t1.send_command(&PanTiltCommand::Home).await });

    let t2 = transport.clone();
    let task2 = tokio::spawn(async move { t2.send_command(&ZoomCommand::Stop).await });

    let t3 = transport.clone();
    let task3 =
        tokio::spawn(async move { t3.send_command(&PowerCommand { power: Power::On }).await });

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
            let visca_transport = mock.into_visca_transport();

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
    let mock = MockAsyncTransport::new();
    // Don't add any response - should timeout
    let transport = mock.into_visca_transport();

    let command = PowerCommand { power: Power::On };

    // Test that transport returns a timeout error when no response is available
    let result = transport.send_command(&command).await;

    // Should get CommandTimeout error from the mock
    assert!(matches!(result, Err(Error::CommandTimeout { .. })));
}

#[tokio::test]
async fn test_error_propagation() {
    // Test that errors propagate correctly through async chains
    let mock = MockAsyncTransport::new().fail_after_n_commands(2);

    // Add responses for first two commands
    mock.add_response(vec![0x90, 0x50, 0xFF]).await;
    mock.add_response(vec![0x90, 0x50, 0xFF]).await;

    let transport = mock.into_visca_transport();

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

    let transport = mock.into_visca_transport();

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
    let mock = MockAsyncTransport::new();

    // Add response only for first command
    mock.add_response(vec![0x90, 0x50, 0xFF]).await;

    let transport = mock.into_visca_transport();

    let command1 = PowerCommand { power: Power::On };
    let command2 = PowerCommand {
        power: Power::Standby,
    };

    let t1 = transport.clone();
    let task1 = tokio::spawn(async move { t1.send_command(&command1).await });

    let t2 = transport.clone();
    let task2 = tokio::spawn(async move { t2.send_command(&command2).await });

    let (r1, r2) = tokio::join!(task1, task2);

    // First should succeed
    assert!(r1.unwrap().is_ok());

    // Second should timeout (no response available)
    assert!(matches!(r2.unwrap(), Err(Error::CommandTimeout { .. })));
}

#[tokio::test]
async fn test_async_command_sequence() {
    // Test a realistic sequence of async commands
    let mock = MockAsyncTransport::new();

    // Prepare a sequence of responses
    // Home command will get socket 0
    mock.add_ack_completion(0).await;
    // Position inquiry will reuse socket 0 after home completes
    mock.add_response(vec![
        0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
    ])
    .await;
    // Zoom will also reuse socket 0
    mock.add_ack_completion(0).await;

    let command_counter = mock.command_counter.clone();
    let transport = mock.into_visca_transport();

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
    let transport = mock.into_visca_transport();

    // Send a command and verify we get the error
    let command = PowerCommand { power: Power::On };
    let result = transport.send_command(&command).await;

    match result {
        Err(Error::SyntaxError) => {}
        _ => panic!("Expected SyntaxError, got {:?}", result),
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

    let transport = mock.into_visca_transport();

    let command = ZoomCommand::Stop;
    let response = transport.send_command(&command).await.unwrap();

    // Should get completion response (ACK is handled internally)
    assert!(matches!(response, Response::Completion));
}
