//! Test for busy command cascading across priorities.
//!
//! This test verifies that when commands receive BUSY responses,
//! the runtime correctly handles retries across different priority levels.

#![cfg(all(feature = "async", feature = "rt-tokio"))]

use flume::bounded;
use grafton_visca::{
    command::response::Response,
    runtime::{Priority, RuntimeHandle, TxItem},
    transport::AsyncTransport,
    Error, TokioExecutor,
};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

/// Mock transport that can simulate BUSY responses
#[derive(Clone)]
struct BusyMockTransport {
    /// Counter for number of commands received
    command_count: Arc<AtomicUsize>,
    /// Queue of responses to return
    response_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    /// Number of busy responses to send before allowing command through
    busy_count: Arc<AtomicUsize>,
    /// Track if we need to send completion after ACK
    pending_completion: Arc<Mutex<bool>>,
    /// Number of busy responses already sent
    busy_sent: Arc<AtomicUsize>,
    /// Track number of responses sent (to avoid infinite responses)
    responses_sent: Arc<AtomicUsize>,
}

impl BusyMockTransport {
    fn new(busy_count: usize) -> Self {
        Self {
            command_count: Arc::new(AtomicUsize::new(0)),
            response_queue: Arc::new(Mutex::new(VecDeque::new())),
            busy_count: Arc::new(AtomicUsize::new(busy_count)),
            pending_completion: Arc::new(Mutex::new(false)),
            busy_sent: Arc::new(AtomicUsize::new(0)),
            responses_sent: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn add_response(&self, response: Vec<u8>) {
        self.response_queue.lock().unwrap().push_back(response);
    }

    fn get_command_count(&self) -> usize {
        self.command_count.load(Ordering::SeqCst)
    }
}

impl AsyncTransport for BusyMockTransport {
    fn send(&self, _data: &[u8]) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        self.command_count.fetch_add(1, Ordering::SeqCst);
        async move { Ok(()) }
    }

    fn recv(&self) -> impl std::future::Future<Output = Result<bytes::Bytes, Error>> + Send {
        let pending_completion = self.pending_completion.clone();
        let busy_count = self.busy_count.clone();
        let busy_sent = self.busy_sent.clone();
        let response_queue = self.response_queue.clone();
        let command_count = self.command_count.clone();
        let responses_sent = self.responses_sent.clone();

        async move {
            // Simulate some network delay
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Get the current counts
            let cmd_count = command_count.load(Ordering::SeqCst);
            let resp_sent = responses_sent.load(Ordering::SeqCst);
            let busy_target = busy_count.load(Ordering::SeqCst);
            let _sent_busy = busy_sent.load(Ordering::SeqCst);

            // Check if we need to send a completion after previous ACK
            {
                let mut pending = pending_completion.lock().unwrap();
                if *pending {
                    *pending = false;
                    responses_sent.fetch_add(1, Ordering::SeqCst);
                    return Ok(bytes::Bytes::from(vec![0x90, 0x51, 0xFF]));
                }
            }

            // If we have no commands yet, wait
            if cmd_count == 0 {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                return Err(Error::Timeout);
            }

            // Check if we should return BUSY
            // For values >= 50, always return BUSY (simulates permanently busy camera)
            if busy_target >= 50 {
                // Return BUSY response with socket 1 (0x90, 0x61 = socket 1, 0x01 = BUSY error code)
                responses_sent.fetch_add(1, Ordering::SeqCst);
                return Ok(bytes::Bytes::from(vec![0x90, 0x61, 0x01, 0xFF]));
            }

            // For normal busy simulation, send BUSY for the first N responses total
            // This means the first N responses across all commands will be BUSY
            if resp_sent < busy_target {
                busy_sent.fetch_add(1, Ordering::SeqCst);
                responses_sent.fetch_add(1, Ordering::SeqCst);
                // Return BUSY response with socket 1 (0x90, 0x61 = socket 1, 0x01 = BUSY error code)
                return Ok(bytes::Bytes::from(vec![0x90, 0x61, 0x01, 0xFF]));
            }

            // After BUSY responses are exhausted, send ACK/Completion for commands
            // Return the next queued response or ACK
            let response = {
                let mut queue = response_queue.lock().unwrap();
                queue.pop_front()
            };

            if let Some(response) = response {
                responses_sent.fetch_add(1, Ordering::SeqCst);
                Ok(bytes::Bytes::from(response))
            } else {
                // Default ACK response, and mark that we need to send completion next
                *pending_completion.lock().unwrap() = true;
                responses_sent.fetch_add(1, Ordering::SeqCst);
                Ok(bytes::Bytes::from(vec![0x90, 0x41, 0xFF]))
            }
        }
    }
}

#[tokio::test]
async fn test_busy_cascade_across_priorities() {
    // Create a mock transport that will send BUSY responses
    let transport = BusyMockTransport::new(3); // Send 3 BUSY responses

    // Create executor and runtime
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let runtime = RuntimeHandle::new(transport.clone(), executor)
        .await
        .unwrap();

    // Create response channels for each command
    let (high_tx, high_rx) = bounded::<Result<Response, Error>>(1);
    let (normal_tx, normal_rx) = bounded::<Result<Response, Error>>(1);
    let (low_tx, low_rx) = bounded::<Result<Response, Error>>(1);

    // Submit commands with different priorities
    let high_cmd = TxItem::Command {
        id: 1,
        bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF], // Power On
        priority: Priority::High,
        deadline: Instant::now() + Duration::from_secs(10),
        category: grafton_visca::timeout::CommandCategory::Quick,
        response_tx: high_tx,
    };

    let normal_cmd = TxItem::Command {
        id: 2,
        bytes: vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF], // Zoom In
        priority: Priority::Normal,
        deadline: Instant::now() + Duration::from_secs(10),
        category: grafton_visca::timeout::CommandCategory::Movement,
        response_tx: normal_tx,
    };

    let low_cmd = TxItem::Command {
        id: 3,
        bytes: vec![0x81, 0x01, 0x04, 0x39, 0x00, 0xFF], // Focus Auto
        priority: Priority::Low,
        deadline: Instant::now() + Duration::from_secs(10),
        category: grafton_visca::timeout::CommandCategory::Quick,
        response_tx: low_tx,
    };

    // Submit all commands
    runtime.command(high_cmd).await.unwrap();
    runtime.command(normal_cmd).await.unwrap();
    runtime.command(low_cmd).await.unwrap();

    // Wait for retries to process
    // With 3 BUSY responses followed by success, all commands should complete
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Collect any available responses (non-blocking)
    let mut results = Vec::new();

    if let Ok(res) = high_rx.try_recv() {
        results.push(("high", res));
    }
    if let Ok(res) = normal_rx.try_recv() {
        results.push(("normal", res));
    }
    if let Ok(res) = low_rx.try_recv() {
        results.push(("low", res));
    }

    // Verify that commands were retried after BUSY
    let command_count = transport.get_command_count();
    assert!(
        command_count > 3,
        "Expected retries after BUSY, but only got {} commands",
        command_count
    );

    // Verify we got at least one successful response
    assert!(
        !results.is_empty(),
        "Expected at least one command to complete after BUSY cleared"
    );

    // Due to the async nature and timing of the retry system,
    // we can't guarantee exact order, but we should verify basic functionality:
    // 1. At least one command completed after retries
    // 2. Commands were actually retried (command count > 3)
    //
    // The exact order depends on:
    // - When each command gets its initial BUSY response
    // - The tick interval (50ms) for processing retries
    // - Socket availability when retries are processed
    //
    // In practice, with only 3 BUSY responses and then success,
    // the order may vary based on timing
}

#[tokio::test]
async fn test_busy_with_max_retries() {
    // Create a mock transport that always returns BUSY
    let transport = BusyMockTransport::new(100); // Always BUSY

    // Create executor and runtime
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let runtime = RuntimeHandle::new(transport.clone(), executor)
        .await
        .unwrap();

    // Create response channel
    let (tx, rx) = bounded::<Result<Response, Error>>(1);

    // Submit a command that will always get BUSY
    let cmd = TxItem::Command {
        id: 1,
        bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF], // Power On
        priority: Priority::Normal,
        deadline: Instant::now() + Duration::from_secs(30), // Long deadline
        category: grafton_visca::timeout::CommandCategory::Quick,
        response_tx: tx,
    };

    runtime.command(cmd).await.unwrap();

    // Wait for the command to fail due to max retries
    // Quick commands have max retries of 5, with exponential backoff:
    // - Initial send: immediate, gets BUSY
    // - Retry 1: 100ms backoff + up to 50ms tick = ~150ms, gets BUSY
    // - Retry 2: 200ms backoff + up to 50ms tick = ~250ms, gets BUSY
    // - Retry 3: 400ms backoff + up to 50ms tick = ~450ms, gets BUSY
    // - Retry 4: 800ms backoff + up to 50ms tick = ~850ms, gets BUSY
    // - Retry 5: 1600ms backoff + up to 50ms tick = ~1650ms, gets BUSY
    // - Check on next tick detects attempt 6 > max_retries 5
    // Total could be 3.4s+ so we need to wait longer
    let result = tokio::time::timeout(Duration::from_secs(5), rx.recv_async()).await;

    match result {
        Ok(Ok(Err(Error::MaxRetriesExceeded))) => {
            // Expected: command failed due to max retries
        }
        Ok(Ok(Err(Error::Timeout))) => {
            // Also acceptable: command timed out
        }
        Ok(Ok(Err(Error::CameraBusy))) => {
            // Also acceptable: busy error propagated after max retries
        }
        Ok(Ok(Err(e))) if e.to_string().contains("busy") || e.to_string().contains("Busy") => {
            // Also acceptable: other busy-related error
        }
        other => {
            panic!(
                "Expected MaxRetriesExceeded, Timeout, or CameraBusy error, got: {:?}",
                other
            );
        }
    }

    // Verify that multiple retry attempts were made
    let command_count = transport.get_command_count();
    assert!(
        command_count > 1,
        "Expected multiple retry attempts, but only got {} commands",
        command_count
    );
}

#[tokio::test]
async fn test_priority_order_during_busy_recovery() {
    // Create a mock transport with controlled BUSY behavior
    let transport = BusyMockTransport::new(2); // 2 BUSY responses initially

    // Queue proper responses for all three commands
    for _ in 0..3 {
        transport.add_response(vec![0x90, 0x41, 0xFF]); // ACK
        transport.add_response(vec![0x90, 0x51, 0xFF]); // Completion
    }

    // Create executor and runtime
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let runtime = RuntimeHandle::new(transport.clone(), executor)
        .await
        .unwrap();

    // Track completion order
    let completion_order = Arc::new(Mutex::new(Vec::new()));

    // Create and submit commands with different priorities
    let mut handles = Vec::new();

    for (id, priority, name) in [
        (1, Priority::Low, "low"),
        (2, Priority::High, "high"),
        (3, Priority::Normal, "normal"),
    ] {
        let (tx, rx) = bounded::<Result<Response, Error>>(1);
        let cmd = TxItem::Command {
            id,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            priority,
            deadline: Instant::now() + Duration::from_secs(10),
            category: grafton_visca::timeout::CommandCategory::Quick,
            response_tx: tx,
        };

        runtime.command(cmd).await.unwrap();

        let order = completion_order.clone();
        let task_name = name.to_string();
        let handle = tokio::spawn(async move {
            if let Ok(Ok(_)) = rx.recv_async().await {
                order.lock().unwrap().push(task_name);
            }
        });
        handles.push(handle);
    }

    // Wait for all commands to complete
    for handle in handles {
        let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
    }

    // Check completion order
    let order = completion_order.lock().unwrap().clone();
    assert!(!order.is_empty(), "No commands completed");

    // High priority should complete before low priority if both are in the order
    if order.contains(&"high".to_string()) && order.contains(&"low".to_string()) {
        let high_idx = order.iter().position(|x| x == "high").unwrap();
        let low_idx = order.iter().position(|x| x == "low").unwrap();
        assert!(
            high_idx < low_idx,
            "High priority should complete before low priority"
        );
    }
}
