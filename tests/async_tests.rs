#![cfg(feature = "async-client")]

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
    transport::Transport,
    ViscaCommand, ViscaError,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{timeout, sleep};

#[tokio::test]
async fn test_async_send_receive_basic() {
    let mut transport = MockAsyncTransport::new();
    transport.add_response(vec![0x90, 0x50, 0xFF]).await;

    let command = PowerCommand { power: Power::On };
    transport.send_command(&command).await.unwrap();

    let sent = transport.sent_commands.lock().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);

    drop(sent); // Release lock
    
    let responses = transport.receive_response().await.unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0], vec![0x90, 0x50, 0xFF]);
}

#[tokio::test]
async fn test_concurrent_commands() {
    // Test that multiple commands can be sent concurrently
    let transport = Arc::new(Mutex::new(MockAsyncTransport::new()));
    
    // Prepare responses for 3 concurrent commands
    {
        let t = transport.lock().await;
        t.add_ack_completion(0).await;
        t.add_ack_completion(1).await;
        t.add_ack_completion(0).await; // Socket 0 reused after completion
    }
    
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
    let count = transport.lock().await.command_count().await;
    assert_eq!(count, 3);
}

#[tokio::test]
async fn test_semaphore_limiting() {
    // Test that semaphore properly limits concurrent commands to 2
    let semaphore = Arc::new(Semaphore::new(2));
    let transport = Arc::new(Mutex::new(MockAsyncTransport::new().with_delay(50)));
    
    // Track timing
    let start = tokio::time::Instant::now();
    
    // Try to send 3 commands with semaphore limiting to 2
    let mut tasks = vec![];
    
    for i in 0..3 {
        let sem = semaphore.clone();
        let t = transport.clone();
        
        let task = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let mut transport = t.lock().await;
            transport.add_response(vec![0x90, 0x50, 0xFF]).await;
            transport.send_command(&PowerCommand { power: Power::On }).await.unwrap();
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
    assert!(elapsed < Duration::from_millis(500)); // Allow more time for CI/slower systems
}

#[tokio::test]
async fn test_timeout_handling() {
    let mut transport = MockAsyncTransport::new().with_delay(100);
    // Don't add any response - should timeout
    
    // Test command timeout
    let result = timeout(
        Duration::from_millis(50),
        transport.receive_response()
    ).await;
    
    assert!(result.is_err()); // Timeout from tokio
    
    // Test with longer timeout - should get ViscaError::Timeout
    let result = timeout(
        Duration::from_millis(200),
        transport.receive_response()
    ).await;
    
    assert!(result.is_ok()); // No tokio timeout
    assert!(matches!(result.unwrap(), Err(ViscaError::Timeout))); // But VISCA timeout
}

#[tokio::test]
async fn test_error_propagation() {
    // Test that errors propagate correctly through async chains
    let mut transport = MockAsyncTransport::new().fail_after_n_commands(2);
    
    // First two commands should succeed
    transport.send_command(&PowerCommand { power: Power::On }).await.unwrap();
    transport.send_command(&ZoomCommand::Stop).await.unwrap();
    
    // Third command should fail
    let result = transport.send_command(&PanTiltCommand::Home).await;
    assert!(result.is_err());
    
    match result.unwrap_err() {
        ViscaError::Io(e) => {
            assert_eq!(e.kind(), std::io::ErrorKind::ConnectionAborted);
        }
        _ => panic!("Expected IO error"),
    }
}

#[tokio::test]
async fn test_inquiry_async_handling() {
    let mut transport = MockAsyncTransport::new();
    
    // Add inquiry response (no ACK for inquiries)
    transport.add_response(vec![
        0x90, 0x50, // Header
        0x00, 0x01, 0x02, 0x03, // Pan
        0x04, 0x05, 0x06, 0x07, // Tilt
        0xFF
    ]).await;
    
    transport.send_command(&InquiryCommand::PanTiltPosition).await.unwrap();
    
    let responses = transport.receive_response().await.unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].len(), 11); // Full inquiry response
}

#[tokio::test]
async fn test_concurrent_timeout_handling() {
    // Test that timeouts in concurrent operations don't affect each other
    let transport = Arc::new(Mutex::new(MockAsyncTransport::new().with_delay(50)));
    
    // Add response only for first command
    transport.lock().await.add_response(vec![0x90, 0x50, 0xFF]).await;
    
    let t1 = transport.clone();
    let task1 = tokio::spawn(async move {
        let mut t = t1.lock().await;
        timeout(Duration::from_millis(100), t.receive_response()).await
    });
    
    let t2 = transport.clone();
    let task2 = tokio::spawn(async move {
        let mut t = t2.lock().await;
        timeout(Duration::from_millis(100), t.receive_response()).await
    });
    
    let (r1, r2) = tokio::join!(task1, task2);
    
    // First should succeed
    assert!(r1.unwrap().is_ok());
    
    // Second should timeout (no response available)
    let r2_result = r2.unwrap();
    assert!(r2_result.is_ok()); // tokio timeout didn't fire
    assert!(matches!(r2_result.unwrap(), Err(ViscaError::Timeout)));
}

#[tokio::test] 
async fn test_async_command_sequence() {
    // Test a realistic sequence of async commands
    let mut transport = MockAsyncTransport::new();
    
    // Prepare a sequence of responses
    transport.add_ack_completion(0).await; // Home
    transport.add_response(vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF]).await; // Position inquiry
    transport.add_ack_completion(1).await; // Zoom
    
    // Execute command sequence
    transport.send_command(&PanTiltCommand::Home).await.unwrap();
    let _ = transport.receive_response().await.unwrap(); // ACK
    let _ = transport.receive_response().await.unwrap(); // Completion
    
    transport.send_command(&InquiryCommand::PanTiltPosition).await.unwrap();
    let pos_response = transport.receive_response().await.unwrap();
    assert_eq!(pos_response[0].len(), 11); // Valid position response
    
    transport.send_command(&ZoomCommand::Stop).await.unwrap();
    let _ = transport.receive_response().await.unwrap(); // ACK
    let _ = transport.receive_response().await.unwrap(); // Completion
    
    assert_eq!(transport.command_count().await, 3);
}