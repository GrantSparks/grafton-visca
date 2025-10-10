//! Serial transport for VISCA over RS-232/422.
//!
//! This module provides serial communication for VISCA protocol,
//! supporting both RS-232 and RS-422 connections with proper
//! Address Set and I/F Clear initialization.

use tracing::trace;

use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    command::CommandKind,
    error::{Error, Result},
    transport::{
        builder::{AddressingMode, TransportConfig},
        serial::{
            handshake::blocking_handshake::{address_set_blocking, if_clear_blocking},
            Config as SerialConfig,
        },
        BlockingTransport, HasTransportConfig,
    },
};

/// Serial transport implementation for blocking I/O.
///
/// This transport operates at the stream level, reading/writing raw bytes.
/// Framing and retry logic are handled by the runtime layer.
#[derive(Debug)]
pub struct SerialTransport {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    config: SerialConfig,
    transport_config: TransportConfig,
}

impl SerialTransport {
    /// Create a new serial transport with the given configuration.
    pub fn new(config: SerialConfig) -> Result<Self> {
        // Open serial port
        let port = serialport::new(&config.port, config.baud_rate)
            .timeout(config.read_timeout)
            .open()
            .map_err(|e| {
                Error::TransportError(format!("Failed to open serial port: {e}").into())
            })?;

        let if_clear = config.if_clear_on_connect;
        let address_set = config.address_set_on_connect;

        // Create TransportConfig from SerialConfig
        let transport_config = TransportConfig {
            read_timeout: config.read_timeout,
            write_timeout: config.write_timeout,
            buffer_config: config.buffer_config,
            retry_config: config.retry_config,
            addressing: AddressingMode::Serial, // Serial transport uses Serial addressing
            ..Default::default()
        };

        let transport = Self {
            port: Arc::new(Mutex::new(port)),
            config,
            transport_config,
        };

        // Perform initialization if requested
        if if_clear {
            let mut port = transport
                .port
                .lock()
                .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
            if_clear_blocking(&mut **port)?;
        }
        if address_set {
            let mut port = transport
                .port
                .lock()
                .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
            address_set_blocking(&mut **port, Duration::from_secs(2))?;
        }

        Ok(transport)
    }

    /// Send raw bytes to the serial port.
    fn send_raw(&self, bytes: &[u8]) -> Result<()> {
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        port.write_all(bytes)
            .map_err(|e| Error::TransportError(format!("Serial write error: {e}").into()))?;
        port.flush()
            .map_err(|e| Error::TransportError(format!("Serial flush error: {e}").into()))?;
        trace!("Sent {} bytes: {:02X?}", bytes.len(), bytes);
        Ok(())
    }
}

impl HasTransportConfig for SerialTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.transport_config
    }
}

// SerialTransport keeps using &self because it has interior mutability
// This is necessary for hardware constraints
impl BlockingTransport for SerialTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<()> {
        // Apply write timeout
        let write_timeout = self.config.write_timeout;
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        let original_timeout = port.timeout();
        port.set_timeout(write_timeout).map_err(|e| {
            Error::TransportError(format!("Failed to set write timeout: {e}").into())
        })?;

        // Send the data
        let result = self.send_raw(bytes);

        // Restore original timeout
        port.set_timeout(original_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize> {
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;

        match port.read(dst) {
            Ok(n) => {
                trace!("Read {} bytes from serial port", n);
                Ok(n)
            }
            Err(e) => Err(e.into()),
        }
    }

    fn recv_into_with_timeout(&mut self, dst: &mut [u8], timeout: Duration) -> Result<usize> {
        // Temporarily set the timeout on the port
        let mut port = self
            .port
            .lock()
            .map_err(|_| Error::LockPoisoned("serial port mutex"))?;
        let original_timeout = port.timeout();
        port.set_timeout(timeout)
            .map_err(|e| Error::TransportError(format!("Failed to set timeout: {e}").into()))?;

        // Read into the provided buffer
        let result = match port.read(dst) {
            Ok(n) => {
                trace!("Read {} bytes from serial port", n);
                Ok(n)
            }
            Err(io_err)
                if io_err.kind() == std::io::ErrorKind::TimedOut
                    || io_err.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Err(Error::Timeout)
            }
            Err(io_err) => Err(io_err.into()),
        };

        // Restore original timeout
        port.set_timeout(original_timeout)
            .map_err(|e| Error::TransportError(format!("Failed to restore timeout: {e}").into()))?;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_config_default() {
        let config = SerialConfig::default();
        assert_eq!(config.baud_rate, 9600);
        assert_eq!(config.camera_address, 1);
        assert!(config.if_clear_on_connect);
        assert!(!config.address_set_on_connect);
    }
}
