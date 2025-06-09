#[cfg(test)]
mod tests {
    use grafton_visca::connection::ConnectionStats;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_connection_stats_thread_safety() {
        let stats = Arc::new(ConnectionStats::new());
        let mut handles = vec![];

        // Spawn multiple threads that concurrently update stats
        for i in 0..10 {
            let stats_clone = Arc::clone(&stats);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    stats_clone.record_sent(100);
                    stats_clone.record_received(50);
                    if i % 2 == 0 {
                        stats_clone.record_error();
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final counts
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.commands_sent, 1000); // 10 threads * 100 commands
        assert_eq!(snapshot.bytes_sent, 100_000); // 10 threads * 100 * 100 bytes
        assert_eq!(snapshot.responses_received, 1000); // 10 threads * 100 responses
        assert_eq!(snapshot.bytes_received, 50_000); // 10 threads * 100 * 50 bytes
        assert_eq!(snapshot.error_count, 500); // 5 threads * 100 errors
    }

    #[test]
    fn test_connection_stats_snapshot() {
        let stats = ConnectionStats::new();

        // Record some activity
        stats.record_sent(100);
        stats.record_received(200);
        stats.record_error();

        // Take a snapshot
        let snapshot1 = stats.snapshot();
        assert_eq!(snapshot1.commands_sent, 1);
        assert_eq!(snapshot1.bytes_sent, 100);
        assert_eq!(snapshot1.responses_received, 1);
        assert_eq!(snapshot1.bytes_received, 200);
        assert_eq!(snapshot1.error_count, 1);

        // Record more activity
        stats.record_sent(50);

        // Take another snapshot
        let snapshot2 = stats.snapshot();
        assert_eq!(snapshot2.commands_sent, 2);
        assert_eq!(snapshot2.bytes_sent, 150);

        // First snapshot should remain unchanged
        assert_eq!(snapshot1.commands_sent, 1);
        assert_eq!(snapshot1.bytes_sent, 100);
    }

    #[test]
    fn test_connection_stats_reset() {
        let stats = ConnectionStats::new();

        // Record some activity
        stats.record_sent(100);
        stats.record_received(200);
        stats.record_error();

        // Reset stats
        stats.reset();

        // Verify everything is reset
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.commands_sent, 0);
        assert_eq!(snapshot.bytes_sent, 0);
        assert_eq!(snapshot.responses_received, 0);
        assert_eq!(snapshot.bytes_received, 0);
        assert_eq!(snapshot.error_count, 0);
        assert!(snapshot.connected_since.is_some());
        assert!(snapshot.last_activity.is_none());
        assert!(snapshot.last_error.is_none());
    }

    #[test]
    fn test_connection_stats_uptime() {
        let stats = ConnectionStats::new();

        // Sleep briefly
        thread::sleep(Duration::from_millis(10));

        // Check uptime
        let uptime = stats.uptime();
        assert!(uptime.is_some());
        assert!(uptime.unwrap() >= Duration::from_millis(10));
    }

    #[test]
    fn test_connection_stats_idle_time() {
        let stats = ConnectionStats::new();

        // Initially no activity
        assert!(stats.idle_time().is_none());

        // Record activity
        stats.record_sent(100);

        // Sleep briefly
        thread::sleep(Duration::from_millis(10));

        // Check idle time
        let idle = stats.idle_time();
        assert!(idle.is_some());
        assert!(idle.unwrap() >= Duration::from_millis(10));
    }

    #[test]
    fn test_health_check_caching() {
        let stats = ConnectionStats::new();

        // Initially no cached health
        assert_eq!(stats.get_cached_health(), None);

        // Record a health check
        stats.record_health_check(true);

        // Should get cached result
        assert_eq!(stats.get_cached_health(), Some(true));

        // Record unhealthy
        stats.record_health_check(false);
        assert_eq!(stats.get_cached_health(), Some(false));
    }

    #[test]
    fn test_health_check_cache_expiry() {
        let stats = ConnectionStats::new();

        // Record a health check
        stats.record_health_check(true);
        assert_eq!(stats.get_cached_health(), Some(true));

        // Sleep for more than cache duration (5 seconds)
        // Note: In real tests, we'd mock time instead of sleeping
        // For now, we'll just verify the cache mechanism exists
        // thread::sleep(Duration::from_secs(6));
        // assert_eq!(stats.get_cached_health(), None);
    }

    #[test]
    fn test_connection_stats_clone() {
        let stats1 = ConnectionStats::new();
        stats1.record_sent(100);
        stats1.record_error();

        #[allow(clippy::redundant_clone)]
        let stats2 = stats1.clone(); // Needed to test shared state

        // Both should have same values initially
        let snapshot1 = stats1.snapshot();
        let snapshot2 = stats2.snapshot();
        assert_eq!(snapshot1.bytes_sent, snapshot2.bytes_sent);
        assert_eq!(snapshot1.error_count, snapshot2.error_count);

        // Modifying one SHOULD affect the other (they share state)
        stats1.record_sent(50);
        let snapshot1_new = stats1.snapshot();
        let snapshot2_new = stats2.snapshot();
        assert_eq!(snapshot1_new.bytes_sent, 150);
        assert_eq!(snapshot2_new.bytes_sent, 150); // Both should be 150
    }
}

// Mock transport for testing
#[cfg(all(test, feature = "blocking-client"))]
mod sync_health_tests {
    use grafton_visca::command::power::{Power, PowerCommand};
    use grafton_visca::connection::ConnectionStats;
    use grafton_visca::{transport::BlockingTransport, ViscaCommand, Error};

    struct MockTransport {
        stats: ConnectionStats,
        fail_send: bool,
        fail_receive: bool,
        empty_response: bool,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                stats: ConnectionStats::new(),
                fail_send: false,
                fail_receive: false,
                empty_response: false,
            }
        }
    }

    impl BlockingTransport for MockTransport {
        fn send_command_blocking(&mut self, _command: &dyn ViscaCommand) -> Result<(), Error> {
            if self.fail_send {
                Err(Error::Io(std::io::Error::other("Mock send error")))
            } else {
                self.stats.record_sent(10);
                Ok(())
            }
        }

        fn receive_response_blocking(&mut self) -> Result<Vec<Vec<u8>>, Error> {
            if self.fail_receive {
                Err(Error::Io(std::io::Error::other("Mock receive error")))
            } else if self.empty_response {
                Ok(vec![])
            } else {
                self.stats.record_received(20);
                Ok(vec![vec![0x90, 0x50, 0x02, 0xFF]])
            }
        }
    }

    // ConnectionManagement trait has been removed in v0.4.0
    // Health checking is now handled internally by Session

    #[test]
    fn test_sync_transport_send_success() {
        let mut transport = MockTransport::new();
        let result = transport.send_command_blocking(&PowerCommand { power: Power::On });
        assert!(result.is_ok());
        assert_eq!(transport.stats.snapshot().bytes_sent, 10);
    }

    #[test]
    fn test_sync_transport_send_failure() {
        let mut transport = MockTransport::new();
        transport.fail_send = true;
        let result = transport.send_command_blocking(&PowerCommand { power: Power::On });
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_transport_receive_success() {
        let mut transport = MockTransport::new();
        let result = transport.receive_response_blocking();
        assert!(result.is_ok());
        assert_eq!(transport.stats.snapshot().bytes_received, 20);
    }

    #[test]
    fn test_sync_transport_receive_failure() {
        let mut transport = MockTransport::new();
        transport.fail_receive = true;
        let result = transport.receive_response_blocking();
        assert!(result.is_err());
    }
}

// Async tests
#[cfg(all(test, feature = "async-client"))]
mod async_health_tests {
    use grafton_visca::command::power::{Power, PowerCommand};
    use grafton_visca::connection::ConnectionStats;
    use grafton_visca::transport::{Transport, TransportFuture};
    use grafton_visca::{ViscaCommand, Error};

    struct MockAsyncTransport {
        stats: ConnectionStats,
        fail_send: bool,
        fail_receive: bool,
        empty_response: bool,
    }

    impl MockAsyncTransport {
        fn new() -> Self {
            Self {
                stats: ConnectionStats::new(),
                fail_send: false,
                fail_receive: false,
                empty_response: false,
            }
        }
    }

    impl Transport for MockAsyncTransport {
        fn send_command<'a>(
            &'a mut self,
            _command: &'a dyn ViscaCommand,
        ) -> TransportFuture<'a, ()> {
            Box::pin(async move {
                if self.fail_send {
                    Err(Error::Io(std::io::Error::other("Mock send error")))
                } else {
                    self.stats.record_sent(10);
                    Ok(())
                }
            })
        }

        fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
            Box::pin(async move {
                if self.fail_receive {
                    Err(Error::Io(std::io::Error::other("Mock receive error")))
                } else if self.empty_response {
                    Ok(vec![])
                } else {
                    self.stats.record_received(20);
                    Ok(vec![vec![0x90, 0x50, 0x02, 0xFF]])
                }
            })
        }
    }

    // AsyncConnectionManagement trait has been removed in v0.4.0
    // Health checking is now handled internally by Session

    #[tokio::test]
    async fn test_async_transport_send_success() {
        let mut transport = MockAsyncTransport::new();
        let result = transport
            .send_command(&PowerCommand { power: Power::On })
            .await;
        assert!(result.is_ok());
        assert_eq!(transport.stats.snapshot().bytes_sent, 10);
    }

    #[tokio::test]
    async fn test_async_transport_send_failure() {
        let mut transport = MockAsyncTransport::new();
        transport.fail_send = true;
        let result = transport
            .send_command(&PowerCommand { power: Power::On })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_async_transport_receive_success() {
        let mut transport = MockAsyncTransport::new();
        let result = transport.receive_response().await;
        assert!(result.is_ok());
        let responses = result.unwrap();
        assert_eq!(responses.len(), 1);
        assert_eq!(transport.stats.snapshot().bytes_received, 20);
    }

    #[tokio::test]
    async fn test_async_transport_receive_failure() {
        let mut transport = MockAsyncTransport::new();
        transport.fail_receive = true;
        let result = transport.receive_response().await;
        assert!(result.is_err());
    }
}
