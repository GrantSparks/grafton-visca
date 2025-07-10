//! Transport adapter for protocol handling and timing.

// Removed unused imports
use crate::Error;

/// Adapter that wraps transports with protocol-specific handling.
#[derive(Debug)]
pub struct TransportAdapter<T> {
    transport: T,
    #[allow(dead_code)] // TODO: Will be used for Sony protocol sequence numbering
    sequence_number: u8,
}

impl<T> TransportAdapter<T> {
    /// Create a new transport adapter.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            sequence_number: 0,
        }
    }

    #[allow(dead_code)] // TODO: Will be used for Sony protocol sequence numbering
    fn next_sequence(&mut self) -> u8 {
        let seq = self.sequence_number;
        self.sequence_number = self.sequence_number.wrapping_add(1);
        seq
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<T> TransportAdapter<T>
where
    T: crate::transport::blocking::BlockingTransport,
{
    /// Send a command through the blocking transport.
    pub fn send_blocking(&mut self, command: &[u8]) -> Result<Vec<u8>, Error> {
        // Handle protocol framing based on profile requirements
        // For Sony encapsulated protocol, we'd add sequence numbers
        // For now, increment sequence for future use
        self.sequence_number = self.next_sequence();

        self.transport.send(command)?;
        self.transport
            .receive(std::time::Duration::from_millis(5000))
    }

    /// Send a const command through the blocking transport.
    pub fn send_const_blocking(&mut self, command: &'static [u8]) -> Result<Vec<u8>, Error> {
        self.send_blocking(command)
    }

    /// Send an array command through the blocking transport.
    pub fn send_array_blocking<const N: usize>(
        &mut self,
        command: [u8; N],
    ) -> Result<Vec<u8>, Error> {
        self.send_blocking(&command)
    }

    /// Send a command and parse the response.
    pub fn send_command<const N: usize>(
        &mut self,
        command: &[u8; N],
    ) -> Result<crate::command::Response, Error> {
        let response_bytes = self.send_blocking(command)?;
        crate::command::Response::parse(&response_bytes)
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<T> TransportAdapter<T>
where
    T: crate::transport::AsyncTransport,
{
    /// Send a command through the async transport.
    pub async fn send_async(&self, command: &[u8]) -> Result<Vec<u8>, Error> {
        // In the real implementation, this would handle protocol framing
        // For now, just pass through
        self.transport.send(command).await?;
        self.transport.receive().await
    }

    /// Send a const command through the async transport.
    pub async fn send_const_async(&self, command: &'static [u8]) -> Result<Vec<u8>, Error> {
        self.send_async(command).await
    }

    /// Send an array command through the async transport.
    pub async fn send_array_async<const N: usize>(
        &self,
        command: [u8; N],
    ) -> Result<Vec<u8>, Error> {
        self.send_async(&command).await
    }

    /// Send a command and parse the response.
    pub async fn send_command<const N: usize>(
        &self,
        command: &[u8; N],
    ) -> Result<crate::command::Response, Error> {
        let response_bytes = self.send_async(command).await?;
        crate::command::Response::parse(&response_bytes)
    }
}
