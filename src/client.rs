//! Thread-safe VISCA client for concurrent camera control.
//!
//! This module provides a thread-safe wrapper around VISCA transports,
//! eliminating the need for RefCell in user code and enabling safe
//! concurrent access from multiple threads.

#![cfg(all(feature = "blocking-client", not(feature = "async-client")))]

// Standard library
use std::sync::{Arc, Mutex};

// Crate imports
use crate::{send_command_and_wait, ViscaCommand, ViscaError, ViscaResponse, ViscaTransport};

/// Thread-safe VISCA client that can be safely shared across threads.
///
/// This client wraps a VISCA transport in an Arc<Mutex<>> to provide
/// interior mutability and thread safety. It eliminates the need for
/// RefCell in user code and allows the client to be cloned and shared
/// between threads.
///
/// # Deprecated
/// This client is deprecated in favor of the unified `ViscaClient` from `unified_client`.
/// The new client provides both blocking and async APIs in a single type.
///
/// # Example
///
/// ```no_run
/// use grafton_visca::{ViscaClient, UdpTransport};
/// use grafton_visca::command::{PowerCommand, power::Power};
/// use std::sync::Arc;
/// use std::thread;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a thread-safe client
/// let transport = UdpTransport::new("192.168.1.100:5678")?;
/// let client = ViscaClient::new(Box::new(transport));
///
/// // Clone the client for use in another thread
/// let client_clone = client.clone();
///
/// // Use the client from multiple threads
/// let handle = thread::spawn(move || {
///     let command = PowerCommand { power: Power::On };
///     client_clone.send(&command)
/// });
///
/// // Wait for the thread to complete
/// handle.join().unwrap()?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
#[deprecated(since = "0.4.0", note = "Use the unified `ViscaClient` instead")]
pub struct ViscaClient {
    transport: Arc<Mutex<Box<dyn ViscaTransport + Send>>>,
}

impl ViscaClient {
    /// Creates a new thread-safe VISCA client with the given transport.
    ///
    /// # Arguments
    /// * `transport` - A boxed transport that implements `ViscaTransport + Send`
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaClient, UdpTransport, ViscaError};
    /// let transport = UdpTransport::new("192.168.1.100:5678")?;
    /// let client = ViscaClient::new(Box::new(transport));
    /// # Ok::<(), ViscaError>(())
    /// ```
    #[must_use]
    pub fn new(transport: Box<dyn ViscaTransport + Send>) -> Self {
        Self {
            transport: Arc::new(Mutex::new(transport)),
        }
    }

    /// Sends a VISCA command and waits for its completion.
    ///
    /// This method internally locks the transport, sends the command,
    /// and waits for the response. The lock is held for the duration
    /// of the command execution.
    ///
    /// # Arguments
    /// * `command` - The VISCA command to send
    ///
    /// # Returns
    /// Returns the response from the camera, which can be:
    /// - `ViscaResponse::Completion` for commands with no data response
    /// - `ViscaResponse::InquiryResponse(...)` for inquiry commands
    /// - `ViscaResponse::Error(...)` if the camera reports an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - The mutex is poisoned (another thread panicked while holding the lock)
    /// - The transport encounters an error
    /// - The camera returns an error response
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaClient, UdpTransport, ViscaError, ViscaResponse};
    /// # use grafton_visca::command::{ZoomCommand};
    /// # let transport = UdpTransport::new("192.168.1.100:5678")?;
    /// # let client = ViscaClient::new(Box::new(transport));
    /// match client.send(&ZoomCommand::TeleStandard)? {
    ///     ViscaResponse::Completion => println!("Zoom command completed"),
    ///     ViscaResponse::Error(e) => println!("Camera error: {:?}", e),
    ///     _ => println!("Unexpected response"),
    /// }
    /// # Ok::<(), ViscaError>(())
    /// ```
    pub fn send(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| ViscaError::InvalidParameter("Mutex poisoned".into()))?;

        send_command_and_wait(&mut **transport, command)
    }

    /// Attempts to send a command without blocking.
    ///
    /// This method tries to acquire the transport lock without blocking.
    /// If another thread is currently using the transport, this method
    /// returns an error immediately instead of waiting.
    ///
    /// # Arguments
    /// * `command` - The VISCA command to send
    ///
    /// # Returns
    /// Returns the response from the camera if successful, or an error if:
    /// - The transport is currently in use by another thread
    /// - The command execution fails
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaClient, UdpTransport, ViscaError};
    /// # use grafton_visca::command::{ZoomCommand};
    /// # let transport = UdpTransport::new("192.168.1.100:5678")?;
    /// # let client = ViscaClient::new(Box::new(transport));
    /// match client.try_send(&ZoomCommand::WideStandard) {
    ///     Ok(response) => println!("Command sent successfully"),
    ///     Err(ViscaError::InvalidParameter(msg)) if msg.contains("busy") => {
    ///         println!("Transport is busy, try again later");
    ///     }
    ///     Err(e) => println!("Command failed: {:?}", e),
    /// }
    /// # Ok::<(), ViscaError>(())
    /// ```
    pub fn try_send(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        let mut transport = self
            .transport
            .try_lock()
            .map_err(|_| ViscaError::InvalidParameter("Transport is busy".into()))?;

        send_command_and_wait(&mut **transport, command)
    }

    /// Sends a command with a timeout.
    ///
    /// This method attempts to acquire the transport lock with a timeout.
    /// If the lock cannot be acquired within the specified duration,
    /// an error is returned.
    ///
    /// Note: This method requires an additional dependency on a timeout
    /// mechanism and is provided as a convenience for future implementation.
    /// Currently, it uses the standard blocking `send` method.
    ///
    /// # Arguments
    /// * `command` - The VISCA command to send
    /// * `timeout` - Maximum time to wait for the transport lock
    ///
    /// # Returns
    /// Returns the response from the camera if successful, or an error if:
    /// - The timeout expires before acquiring the lock
    /// - The command execution fails
    pub fn send_with_timeout(
        &self,
        command: &dyn ViscaCommand,
        _timeout: std::time::Duration,
    ) -> Result<ViscaResponse, ViscaError> {
        // For now, we use the standard send method
        // A proper implementation would use parking_lot::Mutex
        // or a similar mutex with timeout support
        self.send(command)
    }
}

// Safety: ViscaClient can be safely sent between threads because:
// 1. Arc<Mutex<T>> is Send and Sync when T is Send
// 2. Box<dyn ViscaTransport + Send> explicitly requires Send
// 3. Mutex provides thread-safe interior mutability
unsafe impl Send for ViscaClient {}
unsafe impl Sync for ViscaClient {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_client_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<ViscaClient>();
        assert_sync::<ViscaClient>();
    }

    #[test]
    fn test_client_can_be_cloned() {
        use crate::UdpTransport;

        let transport = UdpTransport::new("127.0.0.1:1259").unwrap();
        let client = ViscaClient::new(Box::new(transport));
        let _client_clone = client.clone();
    }

    #[test]
    fn test_client_arc_usage() {
        use crate::UdpTransport;

        let transport = UdpTransport::new("127.0.0.1:1259").unwrap();
        let client = Arc::new(ViscaClient::new(Box::new(transport)));
        let client_clone = Arc::clone(&client);

        let handle = thread::spawn(move || {
            let _local_ref = client_clone;
        });

        handle.join().unwrap();
    }
}
