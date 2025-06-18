//! Tests for ResilientTransport with async transports without Clone requirement

#[cfg(all(feature = "tokio", feature = "async-client"))]
mod tests {
    use grafton_visca::{
        camera::{profiles::PTZOpticsG2, Camera},
        command::zoom::ZoomCommand,
        transport::{
            resilient::{ResilienceConfig, ResilienceEvent, ResilientTransport},
            AsyncTcpTransport, AsyncUdpTransport, Transport,
        },
        types::SocketId,
        Error as ViscaError,
    };
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };
    use std::time::Duration;

    #[tokio::test]
    async fn test_resilient_transport_async_tcp_factory() {
        // Test creating ResilientTransport with async factory for AsyncTcpTransport
        let addr = "192.168.1.100:1259";

        // Try to create initial transport
        let tcp = match AsyncTcpTransport::new(addr).await {
            Ok(t) => t,
            Err(_) => {
                println!("Skipping test - no camera available at {}", addr);
                return;
            }
        };

        // Create resilient transport with async factory
        let mut resilient = ResilientTransport::new_async(
            tcp,
            move || async move { AsyncTcpTransport::new(addr).await.map_err(ViscaError::Io) },
            ResilienceConfig::default(),
        );

        // Test that it implements Transport
        let command = ZoomCommand::Stop;
        let result = resilient.send_command(&command, SocketId::SOCKET_0).await;

        // We don't care if it succeeds (no camera), just that it compiles
        let _ = result;
    }

    #[tokio::test]
    async fn test_resilient_transport_async_udp_factory() {
        // Test creating ResilientTransport with async factory for AsyncUdpTransport
        let addr = "192.168.1.100:52381";

        // Try to create initial transport
        let udp = match AsyncUdpTransport::new(addr).await {
            Ok(t) => t,
            Err(_) => {
                println!("Skipping test - cannot create UDP socket");
                return;
            }
        };

        // Create resilient transport with async factory
        let resilient = ResilientTransport::new_async(
            udp,
            move || async move { AsyncUdpTransport::new(addr).await.map_err(ViscaError::Io) },
            ResilienceConfig::default(),
        );

        // Create camera with resilient transport
        let camera: Camera<PTZOpticsG2> = Camera::new(resilient);

        // Test basic operation
        let _ = camera.get_power_state().await;
    }

    #[tokio::test]
    async fn test_resilient_transport_reconnection_counter() {
        // Test that the factory function is called on reconnection
        let reconnect_count = Arc::new(AtomicU32::new(0));
        let count_clone = reconnect_count.clone();

        // Create a transport that always fails
        struct FailingTransport;

        impl Transport for FailingTransport {
            fn send_command<'a>(
                &'a mut self,
                _command: &'a dyn grafton_visca::Command,
                _socket_id: SocketId,
            ) -> grafton_visca::transport::TransportFuture<'a, ()> {
                Box::pin(async move {
                    Err(ViscaError::ConnectionLost {
                        reason: "Test failure".to_string(),
                    })
                })
            }

            fn receive_response(
                &mut self,
            ) -> grafton_visca::transport::TransportFuture<'_, (SocketId, Vec<u8>)> {
                Box::pin(async move {
                    Err(ViscaError::ConnectionLost {
                        reason: "Test failure".to_string(),
                    })
                })
            }
        }

        // Create resilient transport with factory that counts calls
        let config = ResilienceConfig {
            max_retries: 1,
            max_reconnect_attempts: 3,
            ..Default::default()
        };

        let mut resilient = ResilientTransport::new_async(
            FailingTransport,
            move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
                async move { Ok(FailingTransport) }
            },
            config,
        );

        // Try to send a command - should fail and trigger reconnection attempts
        let command = ZoomCommand::Stop;
        let _ = resilient.send_command(&command, SocketId::SOCKET_0).await;

        // Check that factory was called for reconnection attempts
        let count = reconnect_count.load(Ordering::SeqCst);
        assert!(count > 0, "Factory should have been called at least once");
    }

    #[tokio::test]
    async fn test_resilient_transport_event_callbacks() {
        // Test that event callbacks are triggered correctly
        let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();

        // Create a transport that fails once then succeeds
        struct FlakeyTransport {
            fail_count: Arc<AtomicU32>,
        }

        impl Transport for FlakeyTransport {
            fn send_command<'a>(
                &'a mut self,
                _command: &'a dyn grafton_visca::Command,
                _socket_id: SocketId,
            ) -> grafton_visca::transport::TransportFuture<'a, ()> {
                let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    if count == 0 {
                        Err(ViscaError::Timeout)
                    } else {
                        Ok(())
                    }
                })
            }

            fn receive_response(
                &mut self,
            ) -> grafton_visca::transport::TransportFuture<'_, (SocketId, Vec<u8>)> {
                Box::pin(async move { Ok((SocketId::SOCKET_0, vec![0x90, 0x50, 0xFF])) })
            }
        }

        let fail_count = Arc::new(AtomicU32::new(0));

        let config = ResilienceConfig {
            max_retries: 2,
            ..Default::default()
        };

        let fail_count_factory = fail_count.clone();
        let mut resilient = ResilientTransport::new_async(
            FlakeyTransport {
                fail_count: fail_count.clone(),
            },
            move || {
                let fail_count = fail_count_factory.clone();
                async move { Ok(FlakeyTransport { fail_count }) }
            },
            config,
        );

        // Set event callback
        resilient.set_event_callback(Arc::new(move |event| {
            let events_clone = events_clone.clone();
            tokio::spawn(async move {
                events_clone.lock().await.push(event);
            });
        }));

        // Send command - should fail once then succeed
        let command = ZoomCommand::Stop;
        let result = resilient.send_command(&command, SocketId::SOCKET_0).await;
        assert!(result.is_ok());

        // Give callback time to execute
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check events
        let recorded_events = events.lock().await;
        assert!(!recorded_events.is_empty(), "Should have recorded events");

        // Should have an OperationSucceeded event with retries > 0
        let has_retry_success = recorded_events
            .iter()
            .any(|e| matches!(e, ResilienceEvent::OperationSucceeded { retries } if *retries > 0));
        assert!(has_retry_success, "Should have retry success event");
    }

    #[tokio::test]
    async fn test_resilient_transport_statistics() {
        // Test that statistics are tracked correctly
        struct CountingTransport {
            call_count: Arc<AtomicU32>,
        }

        impl Transport for CountingTransport {
            fn send_command<'a>(
                &'a mut self,
                _command: &'a dyn grafton_visca::Command,
                _socket_id: SocketId,
            ) -> grafton_visca::transport::TransportFuture<'a, ()> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    // Fail first attempt, succeed on retry
                    if count % 2 == 0 {
                        Err(ViscaError::Timeout)
                    } else {
                        Ok(())
                    }
                })
            }

            fn receive_response(
                &mut self,
            ) -> grafton_visca::transport::TransportFuture<'_, (SocketId, Vec<u8>)> {
                Box::pin(async move { Ok((SocketId::SOCKET_0, vec![0x90, 0x50, 0xFF])) })
            }
        }

        let call_count = Arc::new(AtomicU32::new(0));

        let config = ResilienceConfig {
            max_retries: 3,
            ..Default::default()
        };

        let call_count_factory = call_count.clone();
        let mut resilient = ResilientTransport::new_async(
            CountingTransport {
                call_count: call_count.clone(),
            },
            move || {
                let call_count = call_count_factory.clone();
                async move { Ok(CountingTransport { call_count }) }
            },
            config,
        );

        // Send multiple commands
        let command = ZoomCommand::Stop;
        for _ in 0..3 {
            let _ = resilient.send_command(&command, SocketId::SOCKET_0).await;
        }

        // Check statistics
        let stats = resilient.stats();
        assert_eq!(stats.total_operations, 3, "Should have 3 total operations");
        assert!(
            stats.retry_successes > 0,
            "Should have some retry successes"
        );
        assert_eq!(stats.failures, 0, "Should have no failures");
        assert!(stats.success_rate() > 99.0, "Success rate should be 100%");
    }
}

#[cfg(not(all(feature = "tokio", feature = "async-client")))]
fn main() {
    println!("Tests require tokio and async-client features");
}
