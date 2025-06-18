use std::time::Duration;
use crate::error::Error;

/// Core transport trait defining the interface for sending and receiving VISCA commands
pub trait Transport: Send + Sync {
    /// Send a command and wait for a response
    fn send_command(&mut self, command: &[u8]) -> Result<Vec<u8>, Error>;
    
    /// Check if the transport is connected
    fn is_connected(&self) -> bool;
    
    /// Close the transport connection
    fn close(&mut self) -> Result<(), Error>;
}

/// Async transport trait for async/await support
#[cfg(feature = "async-client")]
pub trait AsyncTransport: Send + Sync {
    /// Send a command and wait for a response asynchronously
    fn send_command(&mut self, command: &[u8]) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + Send + '_>>;
    
    /// Check if the transport is connected
    fn is_connected(&self) -> bool;
    
    /// Close the transport connection asynchronously
    fn close(&mut self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + '_>>;
}

/// Builder trait for creating configured transport instances
pub trait TransportBuilder: Sized {
    /// The transport type this builder creates
    type Transport: Transport;
    
    /// Build the transport instance
    fn build(self) -> Result<Self::Transport, Error>;
    
    /// Set the timeout for operations
    fn timeout(self, timeout: Duration) -> Self;
    
    /// Set the number of retry attempts
    fn retries(self, retries: u32) -> Self;
}

/// Transport-specific error type
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// Connection to transport failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    /// Failed to send data through transport
    #[error("Send failed: {0}")]
    SendFailed(String),
    
    /// Failed to receive data from transport
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),
    
    /// Operation timed out
    #[error("Timeout occurred after {0:?}")]
    Timeout(Duration),
    
    /// Transport is not connected
    #[error("Transport is not connected")]
    NotConnected,
    
    /// IO error occurred
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration for transport behavior
#[derive(Debug, Clone, Copy)]
pub struct TransportConfig {
    /// Timeout for send/receive operations
    pub timeout: Duration,
    
    /// Number of retry attempts
    pub retries: u32,
    
    /// Delay between retry attempts
    pub retry_delay: Duration,
    
    /// Buffer size for receiving data
    pub buffer_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            retries: 3,
            retry_delay: Duration::from_millis(100),
            buffer_size: 1024,
        }
    }
}

/// Statistics for transport operations
#[derive(Debug, Default, Clone, Copy)]
pub struct TransportStats {
    /// Total commands sent
    pub commands_sent: u64,
    
    /// Total responses received
    pub responses_received: u64,
    
    /// Total errors encountered
    pub errors: u64,
    
    /// Total retries performed
    pub retries: u64,
    
    /// Total timeouts
    pub timeouts: u64,
}

/// Extended transport trait with statistics and diagnostics
pub trait TransportExt: Transport {
    /// Get transport statistics
    fn stats(&self) -> &TransportStats;
    
    /// Reset statistics
    fn reset_stats(&mut self);
    
    /// Get the transport configuration
    fn config(&self) -> &TransportConfig;
}