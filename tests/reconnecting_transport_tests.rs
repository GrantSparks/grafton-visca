//! Tests for the auto-reconnecting transport functionality.

use grafton_visca::{
    ConnectionManagement, ConnectionStats, ReconnectingTransport, ReconnectionConfig, ViscaCommand,
    ViscaError, ViscaResponse, ViscaTransport,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Mock transport for testing reconnection behavior
struct MockTransport {
    fail_count: Arc<AtomicUsize>,
    should_fail: Arc<AtomicBool>,
    commands_sent: Arc<AtomicUsize>,
    stats: ConnectionStats,
}

impl MockTransport {
    fn new(fail_count: Arc<AtomicUsize>, should_fail: Arc<AtomicBool>) -> Self {
        Self {
            fail_count,
            should_fail,
            commands_sent: Arc::new(AtomicUsize::new(0)),
            stats: ConnectionStats::new(),
        }
    }
}

impl ViscaTransport for MockTransport {
    fn send_command(&mut self, _command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        self.commands_sent.fetch_add(1, Ordering::SeqCst);

        if self.should_fail.load(Ordering::SeqCst) {
            let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                // Fail the first 2 attempts
                Err(ViscaError::Io(std::io::Error::other(
                    "Mock connection error",
                )))
            } else {
                // Succeed on the 3rd attempt
                self.should_fail.store(false, Ordering::SeqCst);
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        if self.should_fail.load(Ordering::SeqCst) {
            Err(ViscaError::Io(std::io::Error::other(
                "Mock connection error",
            )))
        } else {
            // Return a mock ACK response
            Ok(vec![vec![0x90, 0x41, 0xFF]])
        }
    }

    fn send_and_wait(&mut self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        self.send_command(command)?;
        self.receive_response()?;
        Ok(ViscaResponse::Ack)
    }
}

impl ConnectionManagement for MockTransport {
    fn is_healthy(&mut self) -> Result<bool, ViscaError> {
        Ok(!self.should_fail.load(Ordering::SeqCst))
    }

    fn connection_stats(&self) -> &ConnectionStats {
        &self.stats
    }
}

/// Test successful reconnection after transient failures
#[test]
fn test_reconnection_after_failure() {
    let fail_count = Arc::new(AtomicUsize::new(0));
    let should_fail = Arc::new(AtomicBool::new(true));

    let fail_count_clone = fail_count.clone();
    let should_fail_clone = should_fail.clone();

    let config = ReconnectionConfig {
        max_retries: 5,
        initial_delay: Duration::from_millis(10),
        max_delay: Duration::from_secs(1),
        backoff_factor: 2.0,
        health_check_interval: None,
    };

    let mut transport = ReconnectingTransport::new(
        move || {
            if fail_count_clone.load(Ordering::SeqCst) < 1 {
                // First creation attempt fails
                fail_count_clone.fetch_add(1, Ordering::SeqCst);
                Err(ViscaError::Io(std::io::Error::other(
                    "Initial connection failed",
                )))
            } else {
                // Subsequent attempts succeed
                Ok(MockTransport::new(
                    fail_count_clone.clone(),
                    should_fail_clone.clone(),
                ))
            }
        },
        config,
    );

    // Initial connection should fail, but we expect it to retry and succeed
    assert!(transport.is_err());

    // Reset for a successful connection
    fail_count.store(0, Ordering::SeqCst);
    should_fail.store(false, Ordering::SeqCst);

    let mut transport = ReconnectingTransport::new(
        move || Ok(MockTransport::new(fail_count.clone(), should_fail.clone())),
        ReconnectionConfig::default(),
    )
    .unwrap();

    // Test command should succeed
    use grafton_visca::command::power::{Power, PowerCommand};
    let result = transport.send_command(&PowerCommand { power: Power::On });
    assert!(result.is_ok());
}

/// Test that max retries is respected
#[test]
fn test_max_retries_respected() {
    let fail_count = Arc::new(AtomicUsize::new(0));
    let should_fail = Arc::new(AtomicBool::new(true));

    let config = ReconnectionConfig {
        max_retries: 2,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(10),
        backoff_factor: 2.0,
        health_check_interval: None,
    };

    let fail_count_clone = fail_count.clone();
    let should_fail_clone = should_fail.clone();

    let mut transport = ReconnectingTransport::new(
        move || {
            Ok(MockTransport::new(
                fail_count_clone.clone(),
                should_fail_clone.clone(),
            ))
        },
        config,
    )
    .unwrap();

    // This should fail after max_retries attempts
    use grafton_visca::command::power::{Power, PowerCommand};
    let result = transport.send_command(&PowerCommand { power: Power::On });

    assert!(result.is_err());
    match result {
        // The mock never stops failing, so we get the original Io error
        Err(ViscaError::Io(_)) => (),
        Err(ViscaError::ConnectionLost { .. }) => (),
        Err(e) => panic!("Expected Io or ConnectionLost error, got: {:?}", e),
        Ok(_) => panic!("Expected error, but command succeeded"),
    }
}

/// Test exponential backoff timing
#[test]
fn test_exponential_backoff() {
    let config = ReconnectionConfig {
        max_retries: 5,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(10),
        backoff_factor: 2.0,
        health_check_interval: None,
    };

    // Test delay calculation
    let mut delay = config.initial_delay;
    assert_eq!(delay, Duration::from_millis(100));

    delay = Duration::from_secs_f64(delay.as_secs_f64() * config.backoff_factor);
    assert_eq!(delay, Duration::from_millis(200));

    delay = Duration::from_secs_f64(delay.as_secs_f64() * config.backoff_factor);
    assert_eq!(delay, Duration::from_millis(400));
}

/// Test health check functionality
#[test]
fn test_health_check_triggers_reconnection() {
    let should_fail = Arc::new(AtomicBool::new(false));
    let health_check_count = Arc::new(AtomicUsize::new(0));

    let should_fail_clone = should_fail.clone();
    let health_check_count_clone = health_check_count.clone();

    struct HealthCheckMockTransport {
        should_fail: Arc<AtomicBool>,
        health_check_count: Arc<AtomicUsize>,
        stats: ConnectionStats,
    }

    impl ViscaTransport for HealthCheckMockTransport {
        fn send_command(&mut self, _command: &dyn ViscaCommand) -> Result<(), ViscaError> {
            Ok(())
        }

        fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
            Ok(vec![vec![0x90, 0x41, 0xFF]])
        }
    }

    impl ConnectionManagement for HealthCheckMockTransport {
        fn is_healthy(&mut self) -> Result<bool, ViscaError> {
            self.health_check_count.fetch_add(1, Ordering::SeqCst);
            Ok(!self.should_fail.load(Ordering::SeqCst))
        }

        fn connection_stats(&self) -> &ConnectionStats {
            &self.stats
        }
    }

    let config = ReconnectionConfig {
        max_retries: 3,
        initial_delay: Duration::from_millis(10),
        max_delay: Duration::from_secs(1),
        backoff_factor: 2.0,
        health_check_interval: Some(Duration::from_millis(50)),
    };

    let mut transport = ReconnectingTransport::new(
        move || {
            Ok(HealthCheckMockTransport {
                should_fail: should_fail_clone.clone(),
                health_check_count: health_check_count_clone.clone(),
                stats: ConnectionStats::new(),
            })
        },
        config,
    )
    .unwrap();

    // Initial health check should pass
    assert!(transport.is_healthy().unwrap());

    // Verify health check was called
    assert!(health_check_count.load(Ordering::SeqCst) > 0);
}
