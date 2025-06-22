//! Unified VISCA transport wrapper that works with both blocking and async transports.
//!
//! This module provides a single ViscaTransport implementation that adapts to the
//! transport type it wraps, handling all VISCA protocol specifics.

use std::time::Duration;

use crate::{
    command::{
        response::{parse_response, Response},
        ResponseType,
    },
    error::Error,
    Command,
};

/// VISCA protocol constants.
const VISCA_TERMINATOR: u8 = 0xFF;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const ACK_TIMEOUT: Duration = Duration::from_millis(500);
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(30);

/// VISCA transport wrapper that handles protocol-specific logic.
///
/// This wraps any transport (blocking or async) and adds VISCA protocol handling:
/// - Command formatting and termination
/// - ACK/Completion response handling
/// - Response parsing and validation
/// - Timeout error conversion
///
/// The storage model adapts based on features:
/// - For blocking: stores T directly, methods take &mut self
/// - For async: stores `Arc<Mutex<T>>`, methods take &self
#[derive(Debug)]
pub struct ViscaTransport<T> {
    #[cfg(not(feature = "async"))]
    transport: T,
    #[cfg(feature = "async")]
    transport: std::sync::Arc<crate::sync_primitives::Mutex<T>>,
}

// Common implementation
impl<T> ViscaTransport<T> {
    /// Create a transport for blocking usage (stores T directly)
    #[allow(dead_code)]
    pub(crate) fn new_blocking(transport: T) -> Self {
        Self {
            #[cfg(not(feature = "async"))]
            transport,
            #[cfg(feature = "async")]
            transport: std::sync::Arc::new(crate::sync_primitives::Mutex::new(transport)),
        }
    }

    /// Create a transport for async usage (stores `Arc<Mutex<T>>`)
    #[cfg(feature = "async")]
    pub(crate) fn new_async(transport: T) -> Self {
        Self {
            transport: std::sync::Arc::new(crate::sync_primitives::Mutex::new(transport)),
        }
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
mod blocking_impl {
    use super::*;
    use crate::transport::blocking::Transport;

    impl<T: Transport> ViscaTransport<T> {
        /// Create a new VISCA transport wrapping a raw transport.
        pub fn new(transport: T) -> Self {
            Self::new_blocking(transport)
        }

        /// Get a reference to the underlying transport.
        pub fn inner(&self) -> &T {
            &self.transport
        }

        /// Get a mutable reference to the underlying transport.
        pub fn inner_mut(&mut self) -> &mut T {
            &mut self.transport
        }

        /// Send a VISCA command and wait for the response.
        pub fn send_command(&mut self, command: &dyn Command) -> Result<Response, Error> {
            // Get command bytes
            let mut cmd_bytes = command.to_bytes()?;

            // For VISCA over IP, we don't modify the camera address
            // The camera address should already be in the command bytes from to_bytes()

            // Ensure command ends with terminator
            if cmd_bytes.last() != Some(&VISCA_TERMINATOR) {
                cmd_bytes.push(VISCA_TERMINATOR);
            }

            log::debug!("Sending VISCA command: {:02X?}", cmd_bytes);

            // Send command
            self.transport.send(&cmd_bytes)?;

            // Handle response based on command type
            match command.response_type() {
                None => {
                    // Action command - wait for ACK then Completion
                    let ack = self.wait_for_ack(ACK_TIMEOUT)?;
                    match ack {
                        Response::Ack => {
                            // Now wait for completion
                            self.wait_for_completion(COMPLETION_TIMEOUT)
                        }
                        Response::Completion => {
                            // Some cameras send completion directly
                            Ok(Response::Completion)
                        }
                        other => Ok(other),
                    }
                }
                Some(response_type) => {
                    // Inquiry command - wait for data response
                    self.wait_for_inquiry(DEFAULT_TIMEOUT, &response_type)
                }
            }
        }

        /// Wait for ACK response.
        fn wait_for_ack(&mut self, timeout: Duration) -> Result<Response, Error> {
            let data = self.receive_with_timeout(timeout)?;
            self.parse_ack_or_completion(&data)
        }

        /// Wait for completion response.
        fn wait_for_completion(&mut self, timeout: Duration) -> Result<Response, Error> {
            self.wait_for_ack(timeout)
        }

        /// Wait for inquiry response with specific data type.
        fn wait_for_inquiry(
            &mut self,
            timeout: Duration,
            response_type: &ResponseType,
        ) -> Result<Response, Error> {
            let data = self.receive_with_timeout(timeout)?;

            log::debug!("Received VISCA inquiry response: {:02X?}", data);

            // Parse the response with the specific type
            match parse_response(&data, response_type) {
                Ok(response) => Ok(response),
                Err(e) => {
                    log::error!("Failed to parse VISCA response: {:?}", e);
                    Err(Error::InvalidResponse {
                        expected: format!("Valid {:?} response", response_type),
                        actual: data,
                    })
                }
            }
        }

        /// Receive data with timeout handling.
        fn receive_with_timeout(&mut self, timeout: Duration) -> Result<Vec<u8>, Error> {
            match self.transport.receive(timeout) {
                Ok(data) => {
                    if data.is_empty() {
                        Err(Error::InvalidResponseLength)
                    } else {
                        log::debug!("Received VISCA response: {:02X?}", data);
                        Ok(data)
                    }
                }
                Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::TimedOut => {
                    Err(Error::Timeout)
                }
                Err(e) => Err(e),
            }
        }

        /// Parse ACK/Completion/Error response.
        fn parse_ack_or_completion(&self, data: &[u8]) -> Result<Response, Error> {
            // Basic ACK/Completion/Error parsing
            if data.len() >= 3 && data[0] == 0x90 && data[data.len() - 1] == 0xFF {
                match data[1] {
                    0x40..=0x4F => Ok(Response::Ack),
                    0x50..=0x5F => Ok(Response::Completion),
                    0x60..=0x6F => {
                        // Error response - error code is in data[2]
                        if data.len() >= 4 {
                            Err(Error::from_code(data[2]))
                        } else {
                            Err(Error::InvalidResponseLength)
                        }
                    }
                    _ => Ok(Response::Unknown(data.to_vec())),
                }
            } else {
                Err(Error::InvalidResponse {
                    expected: "Valid VISCA response".to_string(),
                    actual: data.to_vec(),
                })
            }
        }

        /// Check if the transport is connected.
        pub fn is_connected(&self) -> bool {
            self.transport.is_connected()
        }

        /// Get a description of this transport.
        pub fn description(&self) -> &str {
            self.transport.description()
        }
    }
}

// Async implementation
#[cfg(feature = "async")]
mod async_impl {
    use super::*;
    use crate::transport::AsyncTransport;

    impl<T: AsyncTransport> ViscaTransport<T> {
        /// Create a new VISCA transport wrapping an async transport.
        pub fn new(transport: T) -> Self {
            Self::new_async(transport)
        }

        /// Get access to the inner transport through a closure.
        pub async fn with_inner<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&T) -> R,
        {
            let guard = self.transport.lock().await;
            f(&*guard)
        }

        /// Get mutable access to the inner transport through a closure.
        pub async fn with_inner_mut<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&mut T) -> R,
        {
            let mut guard = self.transport.lock().await;
            f(&mut *guard)
        }

        /// Send a VISCA command and wait for the response.
        pub async fn send_command(&self, command: &dyn Command) -> Result<Response, Error> {
            // Get command bytes
            let mut cmd_bytes = command.to_bytes()?;

            // For VISCA over IP, we don't modify the camera address
            // The camera address should already be in the command bytes from to_bytes()

            // Ensure command ends with terminator
            if cmd_bytes.last() != Some(&VISCA_TERMINATOR) {
                cmd_bytes.push(VISCA_TERMINATOR);
            }

            log::debug!("Sending VISCA command: {:02X?}", cmd_bytes);

            // Send command
            {
                let transport = self.transport.lock().await;
                transport.send(&cmd_bytes).await?;
            }

            // Handle response based on command type
            match command.response_type() {
                None => {
                    // Action command - wait for ACK then Completion
                    let ack = self.wait_for_ack().await?;
                    match ack {
                        Response::Ack => {
                            // Now wait for completion
                            self.wait_for_completion().await
                        }
                        Response::Completion => {
                            // Some cameras send completion directly
                            Ok(Response::Completion)
                        }
                        other => Ok(other),
                    }
                }
                Some(response_type) => {
                    // Inquiry command - wait for data response
                    self.wait_for_inquiry(&response_type).await
                }
            }
        }

        /// Wait for ACK response.
        async fn wait_for_ack(&self) -> Result<Response, Error> {
            let data = self.receive_with_timeout(ACK_TIMEOUT).await?;
            self.parse_ack_or_completion(&data)
        }

        /// Wait for completion response.
        async fn wait_for_completion(&self) -> Result<Response, Error> {
            let data = self.receive_with_timeout(COMPLETION_TIMEOUT).await?;
            self.parse_ack_or_completion(&data)
        }

        /// Wait for inquiry response with specific data type.
        async fn wait_for_inquiry(&self, response_type: &ResponseType) -> Result<Response, Error> {
            let data = self.receive_with_timeout(DEFAULT_TIMEOUT).await?;

            log::debug!("Received VISCA inquiry response: {:02X?}", data);

            // Parse the response with the specific type
            match parse_response(&data, response_type) {
                Ok(response) => Ok(response),
                Err(e) => {
                    log::error!("Failed to parse VISCA response: {:?}", e);
                    Err(Error::InvalidResponse {
                        expected: format!("Valid {:?} response", response_type),
                        actual: data,
                    })
                }
            }
        }

        /// Receive data with timeout handling.
        async fn receive_with_timeout(&self, timeout: Duration) -> Result<Vec<u8>, Error> {
            // Create the future with the transport lock
            let fut = async {
                let transport = self.transport.lock().await;
                transport.receive().await
            };

            // Apply timeout based on available runtime
            #[cfg(feature = "tokio")]
            let result = tokio::time::timeout(timeout, fut).await;

            #[cfg(not(feature = "tokio"))]
            let result: Result<Result<Vec<u8>, Error>, std::convert::Infallible> = {
                // For non-tokio async, we'll let the transport handle timeouts
                // This is a limitation of being runtime-agnostic
                let _ = timeout; // Unused in non-tokio case
                Ok(fut.await)
            };

            match result {
                Ok(Ok(data)) => {
                    if data.is_empty() {
                        Err(Error::InvalidResponseLength)
                    } else {
                        log::debug!("Received VISCA response: {:02X?}", data);
                        Ok(data)
                    }
                }
                Ok(Err(e)) => Err(e),
                #[cfg(feature = "tokio")]
                Err(_) => Err(Error::Timeout),
                #[cfg(not(feature = "tokio"))]
                Err(_) => unreachable!("Non-tokio timeout should not fail"),
            }
        }

        /// Parse ACK/Completion/Error response.
        fn parse_ack_or_completion(&self, data: &[u8]) -> Result<Response, Error> {
            // Basic ACK/Completion/Error parsing
            if data.len() >= 3 && data[0] == 0x90 && data[data.len() - 1] == 0xFF {
                match data[1] {
                    0x40..=0x4F => Ok(Response::Ack),
                    0x50..=0x5F => Ok(Response::Completion),
                    0x60..=0x6F => {
                        // Error response - error code is in data[2]
                        if data.len() >= 4 {
                            Err(Error::from_code(data[2]))
                        } else {
                            Err(Error::InvalidResponseLength)
                        }
                    }
                    _ => Ok(Response::Unknown(data.to_vec())),
                }
            } else {
                Err(Error::InvalidResponse {
                    expected: "Valid VISCA response".to_string(),
                    actual: data.to_vec(),
                })
            }
        }

        /// Send raw bytes (for internal testing).
        pub async fn send_raw(&self, data: &[u8]) -> Result<(), Error> {
            let transport = self.transport.lock().await;
            transport.send(data).await
        }

        /// Receive raw bytes (for internal testing).
        pub async fn receive_raw(&self) -> Result<Vec<u8>, Error> {
            let transport = self.transport.lock().await;
            transport.receive().await
        }

        /// Check if the transport is connected.
        pub async fn is_connected(&self) -> bool {
            // AsyncTransport doesn't have is_connected, assume true
            true
        }

        /// Get a description of this transport.
        pub async fn description(&self) -> String {
            let transport = self.transport.lock().await;
            format!("{:?}", *transport)
        }
    }

    // Clone implementation for async version
    impl<T: AsyncTransport> Clone for ViscaTransport<T> {
        fn clone(&self) -> Self {
            Self {
                transport: self.transport.clone(),
            }
        }
    }
}
