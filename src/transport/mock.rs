use std::time::Duration;
use crate::error::Error;
use super::traits::{Transport, TransportBuilder, TransportConfig, TransportStats, TransportExt};

/// Mock transport for testing purposes
#[derive(Debug)]
pub struct MockTransport {
    connected: bool,
    config: TransportConfig,
    stats: TransportStats,
    responses: Vec<Vec<u8>>,
    response_index: usize,
}

impl MockTransport {
    /// Create a new mock transport with predefined responses
    pub fn new(responses: Vec<Vec<u8>>) -> Self {
        Self {
            connected: true,
            config: TransportConfig::default(),
            stats: TransportStats::default(),
            responses,
            response_index: 0,
        }
    }

    /// Add a response to the mock transport
    pub fn add_response(&mut self, response: Vec<u8>) {
        self.responses.push(response);
    }
}

impl Transport for MockTransport {
    fn send_command(&mut self, _command: &[u8]) -> Result<Vec<u8>, Error> {
        if !self.connected {
            self.stats.errors += 1;
            return Err(Error::ConnectionLost { reason: "Not connected".to_string() });
        }

        self.stats.commands_sent += 1;
        
        // Return the next response in the queue
        if self.response_index < self.responses.len() {
            let response = self.responses[self.response_index].clone();
            self.response_index += 1;
            self.stats.responses_received += 1;
            Ok(response)
        } else {
            self.stats.errors += 1;
            Err(Error::Timeout)
        }
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn close(&mut self) -> Result<(), Error> {
        self.connected = false;
        Ok(())
    }
}

impl TransportExt for MockTransport {
    fn stats(&self) -> &TransportStats {
        &self.stats
    }

    fn reset_stats(&mut self) {
        self.stats = TransportStats::default();
    }

    fn config(&self) -> &TransportConfig {
        &self.config
    }
}

/// Builder for MockTransport
#[derive(Debug)]
pub struct MockTransportBuilder {
    config: TransportConfig,
    responses: Vec<Vec<u8>>,
}

impl MockTransportBuilder {
    /// Create a new MockTransportBuilder
    pub fn new() -> Self {
        Self {
            config: TransportConfig::default(),
            responses: Vec::new(),
        }
    }

    /// Add a single response to the mock transport
    pub fn with_response(mut self, response: Vec<u8>) -> Self {
        self.responses.push(response);
        self
    }

    /// Set all responses for the mock transport
    pub fn with_responses(mut self, responses: Vec<Vec<u8>>) -> Self {
        self.responses = responses;
        self
    }
}

impl TransportBuilder for MockTransportBuilder {
    type Transport = MockTransport;

    fn build(self) -> Result<Self::Transport, Error> {
        Ok(MockTransport {
            connected: true,
            config: self.config,
            stats: TransportStats::default(),
            responses: self.responses,
            response_index: 0,
        })
    }

    fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    fn retries(mut self, retries: u32) -> Self {
        self.config.retries = retries;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_transport() {
        let mut transport = MockTransportBuilder::new()
            .with_response(vec![0x90, 0x50, 0xFF])
            .with_response(vec![0x90, 0x51, 0xFF])
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();

        // Test sending commands
        let response1 = transport.send_command(&[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]).unwrap();
        assert_eq!(response1, vec![0x90, 0x50, 0xFF]);

        let response2 = transport.send_command(&[0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]).unwrap();
        assert_eq!(response2, vec![0x90, 0x51, 0xFF]);

        // Test stats
        assert_eq!(transport.stats().commands_sent, 2);
        assert_eq!(transport.stats().responses_received, 2);

        // Test timeout when no more responses
        let result = transport.send_command(&[0x81, 0x01, 0x04, 0x00, 0x04, 0xFF]);
        assert!(result.is_err());
        assert_eq!(transport.stats().errors, 1);

        // Test connection state
        assert!(transport.is_connected());
        transport.close().unwrap();
        assert!(!transport.is_connected());
    }
}