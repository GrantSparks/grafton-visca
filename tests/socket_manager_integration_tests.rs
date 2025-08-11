//! Integration tests for socket manager with realistic transport scenarios
//!
//! These tests verify the socket manager behavior with various transport
//! implementations and edge cases that might occur in real-world usage.

#[cfg(feature = "rt-tokio")]
mod integration_tests {
    use bytes::Bytes;
    use grafton_visca::r#async::prelude::*;
    use grafton_visca::transport::Transport;
    use grafton_visca::{camera::profiles::PTZOpticsG2, Camera, Error, PresetNumber};
    use std::collections::VecDeque;
    use std::future::{ready, Future, Ready};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    // VISCA protocol constants
    const VISCA_TERMINATOR: u8 = 0xFF;
    const SOCKET_1_ACK: u8 = 0x41;
    const SOCKET_2_ACK: u8 = 0x42;
    const SOCKET_1_COMPLETION: u8 = 0x51;
    const SOCKET_2_COMPLETION: u8 = 0x52;

    /// Simulates a transport with realistic network delays
    #[derive(Debug, Clone)]
    struct DelayedTransport {
        inner: Arc<Mutex<DelayedTransportInner>>,
    }

    #[derive(Debug)]
    struct DelayedTransportInner {
        sent_commands: Vec<Vec<u8>>,
        response_delay: Duration,
        network_jitter: bool,
        packet_loss_rate: f32,
        command_counter: usize,
        call_count: usize,
    }

    impl DelayedTransport {
        fn new(response_delay: Duration) -> Self {
            Self {
                inner: Arc::new(Mutex::new(DelayedTransportInner {
                    sent_commands: Vec::new(),
                    response_delay,
                    network_jitter: false,
                    packet_loss_rate: 0.0,
                    command_counter: 0,
                    call_count: 0,
                })),
            }
        }

        fn with_jitter(self) -> Self {
            self.inner.lock().unwrap().network_jitter = true;
            self
        }

        fn with_packet_loss(self, rate: f32) -> Self {
            self.inner.lock().unwrap().packet_loss_rate = rate;
            self
        }

        fn get_sent_commands(&self) -> Vec<Vec<u8>> {
            self.inner.lock().unwrap().sent_commands.clone()
        }
    }

    impl Transport for DelayedTransport {
        type Error = Error;
        type SendFut<'a> = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>;
        type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<Bytes, Self::Error>> + Send + 'a>>;

        fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
            Box::pin(async move {
                let mut inner = self.inner.lock().unwrap();
                inner.sent_commands.push(data.to_vec());
                inner.command_counter += 1;

                // Simulate packet loss
                if inner.packet_loss_rate > 0.0 {
                    let should_drop = rand::random::<f32>() < inner.packet_loss_rate;
                    if should_drop {
                        return Err(Error::Timeout);
                    }
                }

                Ok(())
            })
        }

        fn recv(&self) -> Self::RecvFut<'_> {
            Box::pin(async move {
                let (delay, cmd_idx, count) = {
                    let mut inner = self.inner.lock().unwrap();
                    let mut delay = inner.response_delay;

                    // Add jitter if enabled
                    if inner.network_jitter {
                        let jitter_ms = (rand::random::<f32>() * 50.0) as u64;
                        delay = delay + Duration::from_millis(jitter_ms);
                    }

                    let count = inner.call_count;
                    inner.call_count += 1;

                    (delay, inner.command_counter, count)
                };

                // Simulate network delay
                sleep(delay).await;

                // Determine which socket to use based on command index
                let socket_byte = if cmd_idx % 2 == 1 { 0x90 } else { 0x91 };

                if count % 2 == 0 {
                    // Return ACK
                    let ack_type = if socket_byte == 0x90 {
                        SOCKET_1_ACK
                    } else {
                        SOCKET_2_ACK
                    };
                    Ok(Bytes::from(vec![socket_byte, ack_type, VISCA_TERMINATOR]))
                } else {
                    // Return Completion
                    let completion_type = if socket_byte == 0x90 {
                        SOCKET_1_COMPLETION
                    } else {
                        SOCKET_2_COMPLETION
                    };
                    Ok(Bytes::from(vec![
                        socket_byte,
                        completion_type,
                        VISCA_TERMINATOR,
                    ]))
                }
            })
        }
    }

    /// Transport that simulates camera busy responses
    #[derive(Debug, Clone)]
    struct BusyTransport {
        inner: Arc<Mutex<BusyTransportInner>>,
    }

    #[derive(Debug)]
    struct BusyTransportInner {
        sent_commands: Vec<Vec<u8>>,
        busy_count: usize,
        current_busy: usize,
        recv_count: usize,
    }

    impl BusyTransport {
        fn new(busy_count: usize) -> Self {
            Self {
                inner: Arc::new(Mutex::new(BusyTransportInner {
                    sent_commands: Vec::new(),
                    busy_count,
                    current_busy: 0,
                    recv_count: 0,
                })),
            }
        }

        fn get_sent_commands(&self) -> Vec<Vec<u8>> {
            self.inner.lock().unwrap().sent_commands.clone()
        }
    }

    impl Transport for BusyTransport {
        type Error = Error;
        type SendFut<'a> = Ready<Result<(), Self::Error>>;
        type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<Bytes, Self::Error>> + Send + 'a>>;

        fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
            let mut inner = self.inner.lock().unwrap();
            inner.sent_commands.push(data.to_vec());
            ready(Ok(()))
        }

        fn recv(&self) -> Self::RecvFut<'_> {
            Box::pin(async move {
                let mut inner = self.inner.lock().unwrap();
                let count = inner.recv_count;
                inner.recv_count += 1;

                // First few responses are "busy"
                if inner.current_busy < inner.busy_count {
                    inner.current_busy += 1;
                    // Return Socket Busy response (0x60 0x03)
                    return Ok(Bytes::from(vec![0x90, 0x60, 0x03, VISCA_TERMINATOR]));
                }

                // After busy responses, return normal ACK/Completion
                if count % 2 == 0 {
                    Ok(Bytes::from(vec![0x90, SOCKET_1_ACK, VISCA_TERMINATOR]))
                } else {
                    Ok(Bytes::from(vec![
                        0x90,
                        SOCKET_1_COMPLETION,
                        VISCA_TERMINATOR,
                    ]))
                }
            })
        }
    }

    /// Transport that queues multiple commands and processes them in order
    #[derive(Debug, Clone)]
    struct QueuedTransport {
        inner: Arc<Mutex<QueuedTransportInner>>,
    }

    #[derive(Debug)]
    struct QueuedTransportInner {
        sent_commands: Vec<Vec<u8>>,
        command_queue: VecDeque<Vec<u8>>,
        response_queue: VecDeque<Bytes>,
        processing: Arc<AtomicBool>,
    }

    impl QueuedTransport {
        fn new() -> Self {
            Self {
                inner: Arc::new(Mutex::new(QueuedTransportInner {
                    sent_commands: Vec::new(),
                    command_queue: VecDeque::new(),
                    response_queue: VecDeque::new(),
                    processing: Arc::new(AtomicBool::new(false)),
                })),
            }
        }

        fn get_queue_depth(&self) -> usize {
            self.inner.lock().unwrap().command_queue.len()
        }

        fn process_next_command(&self) {
            let mut inner = self.inner.lock().unwrap();
            if let Some(_cmd) = inner.command_queue.pop_front() {
                // Add ACK and Completion to response queue
                inner.response_queue.push_back(Bytes::from(vec![
                    0x90,
                    SOCKET_1_ACK,
                    VISCA_TERMINATOR,
                ]));
                inner.response_queue.push_back(Bytes::from(vec![
                    0x90,
                    SOCKET_1_COMPLETION,
                    VISCA_TERMINATOR,
                ]));
            }
        }
    }

    impl Transport for QueuedTransport {
        type Error = Error;
        type SendFut<'a> = Ready<Result<(), Self::Error>>;
        type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<Bytes, Self::Error>> + Send + 'a>>;

        fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
            let mut inner = self.inner.lock().unwrap();
            inner.sent_commands.push(data.to_vec());
            inner.command_queue.push_back(data.to_vec());

            // Auto-process if not already processing
            if !inner.processing.load(Ordering::SeqCst) {
                inner.processing.store(true, Ordering::SeqCst);
                drop(inner); // Release lock before processing
                self.process_next_command();
            }

            ready(Ok(()))
        }

        fn recv(&self) -> Self::RecvFut<'_> {
            Box::pin(async move {
                loop {
                    let response = {
                        let mut inner = self.inner.lock().unwrap();
                        inner.response_queue.pop_front()
                    };

                    if let Some(response) = response {
                        return Ok(response);
                    }

                    // Wait a bit for responses to be queued
                    sleep(Duration::from_millis(10)).await;
                }
            })
        }
    }

    #[tokio::test]
    async fn test_concurrent_commands_with_delays() {
        // Create camera with delayed transport
        let transport = DelayedTransport::new(Duration::from_millis(50));
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Send multiple commands concurrently
        let start = Instant::now();

        let futures = vec![
            Box::pin(camera.zoom_in()) as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
            Box::pin(camera.zoom_out()) as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
            Box::pin(camera.pan_tilt_stop())
                as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
            Box::pin(camera.focus_auto())
                as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
        ];

        // Execute all commands concurrently
        let results = futures::future::join_all(futures).await;

        let elapsed = start.elapsed();

        // All commands should succeed
        for result in results {
            assert!(result.is_ok(), "Command failed: {:?}", result);
        }

        // Verify commands were sent
        let sent = transport.get_sent_commands();
        assert_eq!(sent.len(), 4, "Should have sent 4 commands");

        // With socket manager, commands should be parallelized
        // Total time should be less than sequential execution would take
        assert!(
            elapsed < Duration::from_millis(400),
            "Commands took too long: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_socket_manager_handles_busy_responses() {
        // Create camera with busy transport (first 2 responses are busy)
        let transport = BusyTransport::new(2);
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Send command that will get busy responses initially
        let result = camera.zoom_in().await;

        // Command should eventually succeed after retries
        assert!(
            result.is_ok(),
            "Command should succeed after busy responses"
        );

        // Verify retries happened
        let sent = transport.get_sent_commands();
        assert!(
            sent.len() >= 2,
            "Should have retried after busy response, sent: {}",
            sent.len()
        );
    }

    #[tokio::test]
    async fn test_command_queueing_under_load() {
        // Create camera with queued transport
        let transport = QueuedTransport::new();
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Send many commands rapidly
        let mut handles = Vec::new();
        for i in 0..10 {
            let cam = camera.clone();
            let handle = tokio::spawn(async move {
                if i % 2 == 0 {
                    cam.zoom_in().await
                } else {
                    cam.zoom_out().await
                }
            });
            handles.push(handle);
        }

        // Let some commands queue up
        sleep(Duration::from_millis(50)).await;

        // Check queue depth
        let depth = transport.get_queue_depth();
        assert!(depth > 0, "Commands should be queued");

        // Process all commands
        for _ in 0..10 {
            transport.process_next_command();
            sleep(Duration::from_millis(10)).await;
        }

        // Wait for all commands to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Command should succeed");
        }
    }

    #[tokio::test]
    async fn test_socket_manager_with_network_jitter() {
        // Create camera with jittery transport
        let transport = DelayedTransport::new(Duration::from_millis(20)).with_jitter();
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Send multiple commands and verify they all complete despite jitter
        let results = futures::future::join_all(vec![
            Box::pin(camera.pan_tilt_home())
                as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
            Box::pin(camera.pan_tilt_reset())
                as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
            Box::pin(camera.pan_tilt_stop())
                as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
            Box::pin(camera.zoom_stop()) as Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>,
        ])
        .await;

        // All commands should succeed despite network jitter
        for (i, result) in results.iter().enumerate() {
            assert!(
                result.is_ok(),
                "Command {} failed with jitter: {:?}",
                i,
                result
            );
        }

        // Verify all commands were sent
        let sent = transport.get_sent_commands();
        assert_eq!(sent.len(), 4, "All commands should be sent");
    }

    #[tokio::test]
    async fn test_socket_allocation_strategy() {
        // Test that socket manager properly allocates sockets for concurrent commands
        let transport = DelayedTransport::new(Duration::from_millis(100));
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Send two commands that should use different sockets
        let (r1, r2) = tokio::join!(camera.zoom_in(), camera.pan_tilt_home());

        assert!(r1.is_ok(), "First command should succeed");
        assert!(r2.is_ok(), "Second command should succeed");

        // Both commands should have been sent immediately (socket manager uses 2 sockets)
        let sent = transport.get_sent_commands();
        assert_eq!(sent.len(), 2, "Both commands should be sent");
    }

    #[tokio::test]
    async fn test_recovery_from_transport_errors() {
        // Create transport that has 10% packet loss
        let transport = DelayedTransport::new(Duration::from_millis(10)).with_packet_loss(0.1);
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Send multiple commands - some will fail due to packet loss
        let mut successes = 0;
        let mut failures = 0;

        for _ in 0..20 {
            match camera.focus_auto().await {
                Ok(_) => successes += 1,
                Err(_) => failures += 1,
            }
        }

        // With 10% packet loss, most commands should still succeed
        assert!(
            successes > failures,
            "Should have more successes ({}) than failures ({})",
            successes,
            failures
        );
    }

    #[tokio::test]
    async fn test_socket_manager_shutdown() {
        // Test that socket manager properly cleans up when camera is dropped
        let transport = DelayedTransport::new(Duration::from_millis(10));

        {
            let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
                .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

            // Send a command
            camera.zoom_in().await.unwrap();
        } // Camera dropped here, socket manager should clean up

        // Give time for cleanup
        sleep(Duration::from_millis(100)).await;

        // Create new camera with same transport - should work fine
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Should be able to send commands with new camera
        let result = camera.zoom_out().await;
        assert!(result.is_ok(), "New camera should work after cleanup");
    }

    #[tokio::test]
    async fn test_concurrent_preset_operations() {
        // Test concurrent preset recall and set operations
        let transport = DelayedTransport::new(Duration::from_millis(20));
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Set multiple presets concurrently
        let preset_futures = (1..=5).map(|i| {
            let preset = PresetNumber::new(i).unwrap();
            camera.preset_set(preset)
        });

        let results = futures::future::join_all(preset_futures).await;

        // All preset sets should succeed
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Preset {} set failed: {:?}", i + 1, result);
        }

        // Recall presets concurrently
        let recall_futures = (1..=5).map(|i| {
            let preset = PresetNumber::new(i).unwrap();
            camera.preset_recall(preset)
        });

        let results = futures::future::join_all(recall_futures).await;

        // All preset recalls should succeed
        for (i, result) in results.iter().enumerate() {
            assert!(
                result.is_ok(),
                "Preset {} recall failed: {:?}",
                i + 1,
                result
            );
        }
    }

    #[tokio::test]
    async fn test_high_frequency_command_bursts() {
        // Test sending rapid bursts of commands
        let transport = DelayedTransport::new(Duration::from_millis(5));
        let camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_runtime(Arc::new(grafton_visca::runtime::TokioRuntime));

        // Send 50 commands as fast as possible
        let start = Instant::now();
        let mut handles = Vec::new();

        for i in 0..50 {
            let cam = camera.clone();
            let handle = tokio::spawn(async move {
                match i % 4 {
                    0 => cam.zoom_in().await,
                    1 => cam.zoom_out().await,
                    2 => cam.focus_auto().await,
                    _ => cam.focus_stop().await,
                }
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok(), "Command should succeed in burst");
        }

        let elapsed = start.elapsed();

        // With socket manager parallelization, this should complete relatively quickly
        assert!(
            elapsed < Duration::from_secs(3),
            "Burst took too long: {:?}",
            elapsed
        );

        // Verify all commands were sent
        let sent = transport.get_sent_commands();
        assert_eq!(sent.len(), 50, "All burst commands should be sent");
    }
}
