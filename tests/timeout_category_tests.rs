//! Comprehensive tests for timeout behavior across all command categories.
//!
//! These tests verify that:
//! 1. Each command category uses the correct timeout duration
//! 2. Timeout configuration changes are applied correctly
//! 3. Socket manager respects custom timeout configurations
//! 4. Timeout errors are handled deterministically

#[cfg(all(test, feature = "rt-tokio"))]
mod timeout_tests {
    use bytes::Bytes;
    use grafton_visca::{
        camera::CameraAsync,
        camera::profiles::GenericVisca,
        Error,
        timeout::TimeoutConfig,
        transport::AsyncTransport,
    };
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    /// Mock transport that simulates delays for testing timeouts
    #[derive(Debug, Clone)]
    struct DelayedMockTransport {
        send_delay: Duration,
        recv_delay: Duration,
        responses: Arc<Mutex<Vec<Bytes>>>,
        recv_count: Arc<Mutex<usize>>,
        send_count: Arc<Mutex<usize>>,
        last_send_time: Arc<Mutex<Option<Instant>>>,
    }

    impl DelayedMockTransport {
        fn new(send_delay: Duration, recv_delay: Duration) -> Self {
            Self {
                send_delay,
                recv_delay,
                responses: Arc::new(Mutex::new(Vec::new())),
                recv_count: Arc::new(Mutex::new(0)),
                send_count: Arc::new(Mutex::new(0)),
                last_send_time: Arc::new(Mutex::new(None)),
            }
        }

        fn add_response(&self, response: Vec<u8>) {
            self.responses.lock().unwrap().push(Bytes::from(response));
        }

        fn get_send_count(&self) -> usize {
            *self.send_count.lock().unwrap()
        }

        fn get_recv_count(&self) -> usize {
            *self.recv_count.lock().unwrap()
        }
    }

    impl AsyncTransport for DelayedMockTransport {
        fn send(&self, _data: &[u8]) -> impl std::future::Future<Output = Result<(), Error>> + Send {
            let send_count = self.send_count.clone();
            let last_send_time = self.last_send_time.clone();
            let send_delay = self.send_delay;
            
            async move {
                *send_count.lock().unwrap() += 1;
                *last_send_time.lock().unwrap() = Some(Instant::now());
                
                if send_delay > Duration::ZERO {
                    tokio::time::sleep(send_delay).await;
                }
                Ok(())
            }
        }

        fn recv(&self) -> impl std::future::Future<Output = Result<Bytes, Error>> + Send {
            let recv_count = self.recv_count.clone();
            let recv_delay = self.recv_delay;
            let responses = self.responses.clone();
            
            async move {
                let count = {
                    let mut count = recv_count.lock().unwrap();
                    *count += 1;
                    *count - 1
                };

                if recv_delay > Duration::ZERO {
                    tokio::time::sleep(recv_delay).await;
                }

                let response = {
                    let responses = responses.lock().unwrap();
                    if count < responses.len() {
                        Some(responses[count].clone())
                    } else {
                        None
                    }
                };
                
                if let Some(response) = response {
                    Ok(response)
                } else {
                    // Simulate hanging recv for timeout testing
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    Err(Error::Timeout)
                }
            }
        }
    }

    #[tokio::test]
    async fn test_quick_command_timeout() {
        // Create transport with delay longer than quick timeout
        let transport = DelayedMockTransport::new(Duration::ZERO, Duration::from_millis(300));
        
        // Add ACK response but delay will cause timeout
        transport.add_response(vec![0x90, 0x41, 0xFF]); // ACK
        
        // Create camera with custom timeout config
        let mut camera: CameraAsync<GenericVisca, _> = CameraAsync::from_transport(transport.clone());
        
        // Set very short timeout for quick commands
        let mut config = TimeoutConfig::default();
        config.quick_timeout = Duration::from_millis(100);
        camera.set_timeout_config(config);
        
        // Initialize socket manager with runtime
        let runtime = std::sync::Arc::new(grafton_visca::runtime::TokioRuntime);
        camera = camera.with_runtime(runtime);
        
        // Try a quick command (power inquiry)
        let start = Instant::now();
        let result = camera.power_inquiry().await;
        let elapsed = start.elapsed();
        
        // Should timeout after ~100ms, not wait for 300ms recv delay
        assert!(result.is_err());
        assert!(elapsed < Duration::from_millis(200));
        assert!(elapsed >= Duration::from_millis(100));
        
        // Verify that send was attempted
        assert_eq!(transport.get_send_count(), 1);
    }

    #[tokio::test]
    async fn test_movement_command_timeout() {
        // Create transport with delay longer than movement timeout
        let transport = DelayedMockTransport::new(Duration::ZERO, Duration::from_secs(3));
        
        // Add ACK response but delay will cause timeout
        transport.add_response(vec![0x90, 0x41, 0xFF]); // ACK
        
        // Create camera with custom timeout config
        let mut camera: CameraAsync<GenericVisca, _> = CameraAsync::from_transport(transport.clone());
        
        // Set 1 second timeout for movement commands
        let mut config = TimeoutConfig::default();
        config.movement_timeout = Duration::from_secs(1);
        camera.set_timeout_config(config);
        
        // Initialize socket manager with runtime
        let runtime = std::sync::Arc::new(grafton_visca::runtime::TokioRuntime);
        camera = camera.with_runtime(runtime);
        
        // Try a movement command (pan_tilt_home)
        let start = Instant::now();
        let result = camera.pan_tilt_home().await;
        let elapsed = start.elapsed();
        
        // Should timeout after ~1s, not wait for 3s recv delay
        assert!(result.is_err());
        assert!(elapsed < Duration::from_millis(1500));
        assert!(elapsed >= Duration::from_millis(1000));
        
        // Verify that send was attempted
        assert_eq!(transport.get_send_count(), 1);
    }

    #[tokio::test]
    async fn test_preset_command_timeout() {
        // Create transport with delay longer than preset timeout
        let transport = DelayedMockTransport::new(Duration::ZERO, Duration::from_secs(4));
        
        // Add ACK response but delay will cause timeout
        transport.add_response(vec![0x90, 0x41, 0xFF]); // ACK
        
        // Create camera with custom timeout config
        let mut camera: CameraAsync<GenericVisca, _> = CameraAsync::from_transport(transport.clone());
        
        // Set 2 second timeout for preset commands
        let mut config = TimeoutConfig::default();
        config.preset_timeout = Duration::from_secs(2);
        camera.set_timeout_config(config);
        
        // Initialize socket manager with runtime
        let runtime = std::sync::Arc::new(grafton_visca::runtime::TokioRuntime);
        camera = camera.with_runtime(runtime);
        
        // Try a preset command
        let start = Instant::now();
        let result = camera.preset_recall(grafton_visca::PresetNumber::new(1).unwrap()).await;
        let elapsed = start.elapsed();
        
        // Should timeout after ~2s, not wait for 4s recv delay
        assert!(result.is_err());
        assert!(elapsed < Duration::from_millis(2500));
        assert!(elapsed >= Duration::from_millis(2000));
        
        // Verify that send was attempted
        assert_eq!(transport.get_send_count(), 1);
    }

    #[tokio::test]
    async fn test_timeout_config_update() {
        // Create transport with consistent delays
        let transport = DelayedMockTransport::new(Duration::ZERO, Duration::from_millis(600));
        
        // Add responses
        transport.add_response(vec![0x90, 0x50, 0xFF]); // Power on response
        transport.add_response(vec![0x90, 0x50, 0xFF]); // Power on response
        
        // Create camera with default timeout
        let mut camera: CameraAsync<GenericVisca, _> = CameraAsync::from_transport(transport.clone());
        
        // Initialize socket manager with runtime
        let runtime = std::sync::Arc::new(grafton_visca::runtime::TokioRuntime);
        camera = camera.with_runtime(runtime);
        
        // First command should succeed with default timeout (5s)
        let start = Instant::now();
        let result = camera.power_inquiry().await;
        let elapsed = start.elapsed();
        
        // Should succeed since 600ms < 5s default
        assert!(result.is_ok());
        assert!(elapsed >= Duration::from_millis(600));
        assert!(elapsed < Duration::from_secs(1));
        
        // Update timeout to be shorter
        let mut config = TimeoutConfig::default();
        config.quick_timeout = Duration::from_millis(300);
        camera.set_timeout_config(config);
        
        // Second command should timeout with new config
        let start = Instant::now();
        let result = camera.power_inquiry().await;
        let elapsed = start.elapsed();
        
        // Should timeout after ~300ms
        assert!(result.is_err());
        assert!(elapsed < Duration::from_millis(400));
        assert!(elapsed >= Duration::from_millis(300));
    }

    #[tokio::test]
    async fn test_concurrent_timeouts_different_categories() {
        // Create transport with moderate delay
        let transport = DelayedMockTransport::new(Duration::ZERO, Duration::from_millis(800));
        
        // Add responses for multiple commands
        transport.add_response(vec![0x90, 0x50, 0xFF]); // Power response
        transport.add_response(vec![0x90, 0x51, 0xFF]); // Completion
        transport.add_response(vec![0x90, 0x50, 0xFF]); // Zoom response
        
        // Create camera with different timeouts per category
        let mut camera: CameraAsync<GenericVisca, _> = CameraAsync::from_transport(transport);
        
        let mut config = TimeoutConfig::default();
        config.quick_timeout = Duration::from_millis(500);    // Will timeout
        config.movement_timeout = Duration::from_secs(2);      // Will succeed
        camera.set_timeout_config(config);
        
        // Initialize socket manager with runtime
        let runtime = std::sync::Arc::new(grafton_visca::runtime::TokioRuntime);
        camera = camera.with_runtime(runtime);
        
        // Launch concurrent commands with different timeout categories
        let camera_clone = camera.clone();
        let quick_handle = tokio::spawn(async move {
            let start = Instant::now();
            let result = camera_clone.power_inquiry().await;
            (result, start.elapsed())
        });
        
        let camera_clone = camera.clone();
        let movement_handle = tokio::spawn(async move {
            // Small delay to ensure ordering
            tokio::time::sleep(Duration::from_millis(50)).await;
            let start = Instant::now();
            let result = camera_clone.zoom_stop().await;
            (result, start.elapsed())
        });
        
        // Wait for both to complete
        let (quick_result, quick_elapsed) = quick_handle.await.unwrap();
        let (movement_result, movement_elapsed) = movement_handle.await.unwrap();
        
        // Quick command should timeout
        assert!(quick_result.is_err());
        assert!(quick_elapsed < Duration::from_millis(600));
        assert!(quick_elapsed >= Duration::from_millis(500));
        
        // Movement command should succeed
        assert!(movement_result.is_ok());
        assert!(movement_elapsed >= Duration::from_millis(800));
        assert!(movement_elapsed < Duration::from_millis(1000));
    }
}