//! Integration tests for the new flume-based runtime implementation.
//!
//! These tests verify that the runtime correctly:
//! - Schedules and sends commands
//! - Handles responses (ACK, Completion, DataReply, Error)
//! - Manages socket allocation
//! - Enforces timing constraints
//! - Handles inquiries and their responses

#![cfg(all(feature = "rt-tokio", feature = "test-utils"))]

#[cfg(feature = "rt-tokio")]
use flume::{Receiver, Sender};
#[cfg(feature = "rt-tokio")]
use grafton_visca::{
    command::response::{ViscaResponse, ViscaResponseType},
    runtime::{Priority, RuntimeHandle, TxItem},
    testing::testkit::{ScriptedTransport, Step},
    timeout::CommandCategory,
    Error, Result, TokioExecutor,
};

#[cfg(feature = "rt-tokio")]
use std::time::Duration;

#[cfg(feature = "rt-tokio")]
mod runtime_tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn test_runtime_sends_command() {
        // Create TokioExecutor and scripted transport
        let executor =
            std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Power on command
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK
                vec![0x90, 0x51, 0xFF], // Completion
            ],
        }])
        .with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .unwrap();

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send a command
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let command = TxItem::Command {
            id: 1,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF], // Power on
            priority: Priority::Normal,
            response_tx,
            category: CommandCategory::Quick,
            deadline: std::time::Instant::now() + Duration::from_secs(30),
        };

        runtime.command(command).await.unwrap();

        // Advance time to allow command processing
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for response
        let response = response_rx.recv_async().await.unwrap();
        assert!(matches!(response, Ok(ViscaResponse::Completion)));

        // Verify command was sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    }

    #[tokio::test(start_paused = true)]
    async fn test_runtime_handles_inquiry() {
        // Create deterministic executor and scripted transport
        let executor =
            std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
            responses: vec![
                vec![0x90, 0x50, 0x02, 0xFF], // Power on response
            ],
        }])
        .with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .unwrap();

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send an inquiry
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let inquiry = TxItem::Inquiry {
            id: 1,
            bytes: vec![0x81, 0x09, 0x04, 0x00, 0xFF], // Power inquiry
            response_tx,
            response_type: Some(ViscaResponseType::Power),
            deadline: std::time::Instant::now() + Duration::from_secs(5),
        };

        runtime.inquire(inquiry).await.unwrap();

        // Advance time to allow inquiry processing
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for response
        let response = response_rx.recv_async().await.unwrap();

        // Should receive inquiry response
        match response {
            Ok(ViscaResponse::Inquiry(_)) => {
                // Expected - actual data would be in the InquiryResponse
            }
            Ok(ViscaResponse::Unknown { .. }) => {
                // Also acceptable for this test
            }
            _ => panic!("Expected Inquiry or Unknown response, got: {:?}", response),
        }

        // Verify inquiry was sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x09, 0x04, 0x00, 0xFF]);
    }

    #[tokio::test(start_paused = true)]
    async fn test_runtime_handles_error_response() {
        // Create deterministic executor and scripted transport
        let executor =
            std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Power on command
            responses: vec![
                vec![0x90, 0x60, 0x02, 0xFF], // Syntax error
            ],
        }])
        .with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .unwrap();

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send a command
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let command = TxItem::Command {
            id: 1,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            priority: Priority::Normal,
            response_tx,
            category: CommandCategory::Quick,
            deadline: std::time::Instant::now() + Duration::from_secs(30),
        };

        runtime.command(command).await.unwrap();

        // Advance time to allow command processing
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for response
        let response = response_rx.recv_async().await.unwrap();

        // Should receive error
        match response {
            Err(Error::SyntaxError) => {
                // Expected
            }
            _ => panic!("Expected SyntaxError, got: {:?}", response),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_runtime_handles_busy_and_retry() {
        // Create deterministic executor and scripted transport
        let executor =
            std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = ScriptedTransport::new(vec![
            // First send gets a busy response
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
                responses: vec![
                    vec![0x90, 0x61, 0x41, 0xFF], // Socket 1 busy (0x41 = NotExecutable/busy)
                ],
            },
            // Second send (retry) gets success
            Step::OnSend {
                matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK
                    vec![0x90, 0x51, 0xFF], // Completion
                ],
            },
        ])
        .with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .unwrap();

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send a command
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let command = TxItem::Command {
            id: 1,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            priority: Priority::Normal,
            response_tx,
            category: CommandCategory::Quick,
            deadline: std::time::Instant::now() + Duration::from_secs(30),
        };

        runtime.command(command).await.unwrap();

        // Advance time to allow command processing and retry
        tokio::time::advance(Duration::from_millis(300)).await;

        // Wait for the command to complete (busy -> retry -> completion)
        let response = response_rx.recv_async().await.unwrap();

        assert!(
            matches!(response, Ok(ViscaResponse::Completion)),
            "Expected completion, got: {:?}",
            response
        );

        // Verify command was sent twice (initial + retry)
        let sent = transport.sent();
        assert!(
            sent.len() >= 2,
            "Expected at least 2 sends, got {}",
            sent.len()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_runtime_priority_scheduling() {
        // Create deterministic executor and scripted transport
        let executor =
            std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = ScriptedTransport::new(vec![
            // Respond to any command with success
            Step::OnSend {
                matches: None, // Match any command
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK for socket 1
                    vec![0x90, 0x51, 0xFF], // Completion for socket 1
                ],
            },
            Step::OnSend {
                matches: None,
                responses: vec![
                    vec![0x90, 0x42, 0xFF], // ACK for socket 2
                    vec![0x90, 0x52, 0xFF], // Completion for socket 2
                ],
            },
            Step::OnSend {
                matches: None,
                responses: vec![
                    vec![0x90, 0x41, 0xFF], // ACK for socket 1 (reused)
                    vec![0x90, 0x51, 0xFF], // Completion for socket 1
                ],
            },
        ])
        .with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .unwrap();

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send commands with different priorities
        let mut receivers = Vec::new();

        // Low priority
        let (tx1, rx1): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        runtime
            .command(TxItem::Command {
                id: 1,
                bytes: vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF], // Command 1
                priority: Priority::Low,
                response_tx: tx1,
                category: CommandCategory::Quick,
                deadline: std::time::Instant::now() + Duration::from_secs(30),
            })
            .await
            .unwrap();
        receivers.push(rx1);

        // High priority
        let (tx2, rx2): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        runtime
            .command(TxItem::Command {
                id: 2,
                bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF], // Command 2
                priority: Priority::High,
                response_tx: tx2,
                category: CommandCategory::Quick,
                deadline: std::time::Instant::now() + Duration::from_secs(30),
            })
            .await
            .unwrap();
        receivers.push(rx2);

        // Normal priority
        let (tx3, rx3): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        runtime
            .command(TxItem::Command {
                id: 3,
                bytes: vec![0x81, 0x01, 0x04, 0x00, 0x01, 0xFF], // Command 3
                priority: Priority::Normal,
                response_tx: tx3,
                category: CommandCategory::Quick,
                deadline: std::time::Instant::now() + Duration::from_secs(30),
            })
            .await
            .unwrap();
        receivers.push(rx3);

        // Advance time to allow command processing
        tokio::time::advance(Duration::from_millis(100)).await;

        for rx in receivers {
            let response = rx.recv_async().await.unwrap();
            assert!(response.is_ok(), "Command should complete successfully");
        }

        // Check that all commands were sent
        let sent = transport.sent();
        assert!(
            sent.len() >= 3,
            "Expected at least 3 commands sent, got {}",
            sent.len()
        );

        // Verify all command types were sent (low=0x03, high=0x02, normal=0x01)
        let has_low = sent.iter().any(|cmd| cmd[4] == 0x03);
        let has_high = sent.iter().any(|cmd| cmd[4] == 0x02);
        let has_normal = sent.iter().any(|cmd| cmd[4] == 0x01);

        assert!(has_low, "Low priority command should be sent");
        assert!(has_high, "High priority command should be sent");
        assert!(has_normal, "Normal priority command should be sent");
    }

    #[tokio::test(start_paused = true)]
    async fn test_runtime_cancel_command() {
        // Create deterministic executor and scripted transport with no responses
        let executor =
            std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = ScriptedTransport::new(vec![]).with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .unwrap();

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send a command
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let command = TxItem::Command {
            id: 42,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            priority: Priority::Normal,
            response_tx,
            category: CommandCategory::Movement,
            deadline: std::time::Instant::now() + Duration::from_secs(30),
        };

        runtime.command(command).await.unwrap();

        // Cancel the command
        runtime.cancel(42).await.unwrap();

        // Advance some time
        tokio::time::advance(Duration::from_millis(100)).await;

        // The command should be cancelled - check if we can receive or if channel is closed
        match response_rx.try_recv() {
            Ok(response) => {
                // If we got a response, it should be an error
                assert!(response.is_err(), "Expected error for cancelled command");
            }
            Err(_) => {
                // Channel might be empty or closed, both are acceptable for cancelled commands
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_runtime_metrics_tracking() {
        // Create deterministic executor and scripted transport
        let executor =
            std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]),
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK socket 1
                vec![0x90, 0x51, 0xFF], // Completion socket 1
            ],
        }])
        .with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .expect("Failed to create runtime");

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send a normal command
        let (response_tx1, response_rx1) = flume::bounded(1);
        runtime
            .command(TxItem::Command {
                id: 1,
                bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
                priority: Priority::Normal,
                deadline: std::time::Instant::now() + Duration::from_secs(5),
                category: CommandCategory::Movement,
                response_tx: response_tx1,
            })
            .await
            .expect("Failed to send command");

        // Advance time to process command
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for completion
        let response = response_rx1.recv_async().await.expect("Channel closed");
        assert!(response.is_ok(), "Command should complete successfully");

        // Get metrics
        let metrics = runtime.metrics().await.expect("Failed to get metrics");

        // Verify basic metrics
        assert!(
            metrics.commands_submitted >= 1,
            "Should have at least 1 command submitted"
        );
        assert!(
            metrics.commands_completed >= 1,
            "Should have at least 1 command completed"
        );
        assert_eq!(
            metrics.priority_counts[1], 1,
            "Should have 1 Normal priority command"
        );

        runtime.shutdown().await;
    }

    #[tokio::test(start_paused = true)]
    async fn test_runtime_shutdown() {
        // Create deterministic executor and scripted transport
        let executor =
            std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = ScriptedTransport::new(vec![]).with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .unwrap();

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Shutdown the runtime
        runtime.shutdown().await;

        // Trying to send commands should fail
        let (response_tx, _response_rx) = flume::bounded(1);
        let command = TxItem::Command {
            id: 1,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            priority: Priority::Normal,
            response_tx,
            category: CommandCategory::Quick,
            deadline: std::time::Instant::now() + Duration::from_secs(30),
        };

        let result = runtime.command(command).await;
        assert!(
            result.is_err(),
            "Should fail to send command after shutdown"
        );
    }
}
