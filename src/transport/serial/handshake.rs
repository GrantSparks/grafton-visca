//! Runtime-agnostic serial handshake module for VISCA communication.
//!
//! This module provides unified handshake logic for both async and blocking
//! serial transports, including I/F Clear and Address Set operations.
//!
//! The handshake process is protocol-aware and uses the existing ProtocolFramer
//! for robust frame handling instead of manual buffer scanning.

use std::time::Duration;
use tracing::{debug, trace, warn};

use crate::{
    camera_id::CameraId,
    command::{
        bytes::VISCA_TERMINATOR,
        encode_visca::ViscaEncode,
        system::{AddressSetCommand, InterfaceClearCommand},
    },
    error::{Error, Result},
    protocol::framer::ProtocolFramer,
    transport::buffer::BufferConfig,
};

/// Result of parsing address set response bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseOutcome {
    /// Partial response received, more data needed.
    Partial {
        /// Number of cameras discovered so far.
        camera_count: u8,
    },
    /// Address set complete.
    Complete {
        /// Total number of cameras discovered.
        camera_count: u8,
    },
}

/// Parse address set response bytes from the buffer.
///
/// This function processes VISCA address set responses:
/// - Camera address assignments: 0x88 0x30 0x01..0x07 0xFF
/// - Completion marker: 0x88 0x30 0x02 0xFF
///
/// Returns `ParseOutcome::Partial` if more data is needed,
/// or `ParseOutcome::Complete` when the address set is finished.
///
/// Note: In VISCA protocol, cameras respond with their assigned number (1-7),
/// and 0x02 is specifically the "Network Change" completion marker, not camera 2.
pub fn parse_address_set_bytes(buf: &[u8]) -> ParseOutcome {
    let mut camera_count = 0u8;
    let mut i = 0;

    while i < buf.len() {
        // Look for address setting response: 88 30 0p FF where p is camera number or completion
        if i + 3 < buf.len()
            && buf[i] == 0x88
            && buf[i + 1] == 0x30
            && buf[i + 3] == VISCA_TERMINATOR
        {
            // Check for completion marker first (0x02 is specifically the completion marker)
            if buf[i + 2] == 0x02 {
                // End of address setting (0x02 is the network change complete marker)
                debug!("Address Set complete, {} cameras found", camera_count);
                return ParseOutcome::Complete { camera_count };
            } else if buf[i + 2] >= 0x01 && buf[i + 2] <= 0x07 {
                // Camera address assignment - track the highest camera number
                // Note: Cameras can have IDs 1-7, but 2 is NOT used as it's the completion marker
                let camera_num = buf[i + 2];
                if camera_num > camera_count {
                    camera_count = camera_num;
                }
                trace!("Camera {} assigned address", camera_num);
                i += 4;
            } else {
                // Unknown response byte, skip
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    ParseOutcome::Partial { camera_count }
}

// Async handshake functions (feature-gated for tokio-serial)
#[cfg(feature = "tokio-serial")]
pub mod async_handshake {
    use super::*;
    use crate::{
        executor::Executor,
        transport::async_io::{AsyncReadExt, AsyncWriteExt},
    };
    use bytes::BytesMut;

    /// Send I/F Clear command to reset all devices on the bus.
    ///
    /// This is executor-driven and runtime-agnostic.
    pub async fn if_clear_async<E, S>(exec: &E, io: &mut S) -> Result<()>
    where
        E: Executor,
        S: AsyncWriteExt + Send + ?Sized,
    {
        debug!("Sending I/F Clear command");
        let cmd = InterfaceClearCommand::new();
        let mut buffer = [0u8; 16];

        // InterfaceClearCommand is const-constructed and guaranteed to encode
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .map_err(|e| Error::TransportError(format!("Failed to encode IF Clear: {e}").into()))?;

        io.write_all(&buffer[..len]).await?;
        io.flush().await?;

        // Wait for I/F Clear to complete using executor's sleep
        exec.sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    /// Send Address Set command to assign addresses to devices.
    ///
    /// Returns the number of cameras detected.
    /// This is executor-driven and runtime-agnostic.
    pub async fn address_set_async<E, S>(exec: &E, io: &mut S, timeout: Duration) -> Result<u8>
    where
        E: Executor,
        S: AsyncReadExt + AsyncWriteExt + Send + ?Sized,
    {
        let max_attempts = 3;

        for attempt in 0..max_attempts {
            debug!("Address Set attempt {}", attempt + 1);
            let cmd = AddressSetCommand::new();
            let mut buffer = [0u8; 16];

            // AddressSetCommand is const-constructed and guaranteed to encode
            let len = cmd
                .encode_into(CameraId::CAMERA_1, &mut buffer)
                .map_err(|e| {
                    Error::TransportError(format!("Failed to encode Address Set: {e}").into())
                })?;

            io.write_all(&buffer[..len]).await?;
            io.flush().await?;

            // Parse response properly
            match recv_address_set_response_async(exec, io, timeout).await {
                Ok(camera_count) => {
                    debug!("Address Set successful, found {camera_count} cameras");
                    return Ok(camera_count);
                }
                Err(Error::Timeout) if attempt < max_attempts - 1 => {
                    warn!("Address Set timeout, retrying...");
                    exec.sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::MaxRetriesExceeded)
    }

    /// Receive and parse Address Set response using executor-driven timeout.
    async fn recv_address_set_response_async<E, S>(
        exec: &E,
        stream: &mut S,
        timeout_duration: Duration,
    ) -> Result<u8>
    where
        E: Executor,
        S: AsyncReadExt + Send + ?Sized,
    {
        // Use ProtocolFramer for robust frame handling
        let mut framer = ProtocolFramer::new_with_config(BufferConfig::for_serial());
        let mut response_buffer = BytesMut::with_capacity(128);
        let start = std::time::Instant::now();

        // Simple timeout loop without nested executor timeouts
        while start.elapsed() < timeout_duration {
            // Try to read some data
            let mut temp_buf = vec![0u8; 64];

            // Do a short non-blocking read attempt
            match stream.read(&mut temp_buf).await {
                Ok(n) if n > 0 => {
                    response_buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Address Set response: {:02X?}", &temp_buf[..n]);

                    // Push data to framer
                    framer.push_slice(&temp_buf[..n])?;

                    // Try to extract frames and parse them
                    for frame_result in framer.drain_frames() {
                        let frame = frame_result?;
                        match parse_address_set_bytes(&frame) {
                            ParseOutcome::Complete { camera_count } => {
                                return Ok(camera_count);
                            }
                            ParseOutcome::Partial { .. } => {
                                // Continue accumulating
                            }
                        }
                    }

                    // Also check accumulated buffer for backward compatibility
                    match parse_address_set_bytes(&response_buffer) {
                        ParseOutcome::Complete { camera_count } => {
                            return Ok(camera_count);
                        }
                        ParseOutcome::Partial { .. } => {
                            // Continue waiting for more data
                        }
                    }
                }
                Ok(_) => {
                    // No data read, continue waiting
                    exec.sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    // Check if it's an I/O error with TimedOut kind
                    if let Error::Io(io_err) = &e {
                        if io_err.kind() == std::io::ErrorKind::TimedOut {
                            // Timeout on this read, but total timeout not reached yet
                            exec.sleep(Duration::from_millis(10)).await;
                            continue;
                        }
                    }
                    return Err(e);
                }
            }
        }

        // Total timeout reached - check if we got any cameras
        match parse_address_set_bytes(&response_buffer) {
            ParseOutcome::Partial { camera_count } if camera_count > 0 => {
                debug!(
                    "Address Set timeout reached, but {} cameras were found",
                    camera_count
                );
                Ok(camera_count)
            }
            _ => {
                debug!("Address Set timeout - no cameras found");
                Err(Error::Timeout)
            }
        }
    }
}

// Blocking handshake functions
#[cfg(all(not(feature = "async"), feature = "serialport"))]
pub mod blocking_handshake {
    use super::*;
    use std::{
        io::{Read, Write},
        time::Instant,
    };

    /// Send I/F Clear command to reset all devices on the bus (blocking).
    pub fn if_clear_blocking<S>(io: &mut S) -> Result<()>
    where
        S: Write + ?Sized,
    {
        debug!("Sending I/F Clear command");
        let cmd = InterfaceClearCommand::new();
        let mut buffer = [0u8; 16];

        // InterfaceClearCommand is const-constructed and guaranteed to encode
        let len = cmd
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .map_err(|e| Error::TransportError(format!("Failed to encode IF Clear: {e}").into()))?;

        io.write_all(&buffer[..len])
            .map_err(|e| Error::TransportError(format!("Serial write error: {e}").into()))?;
        io.flush()
            .map_err(|e| Error::TransportError(format!("Serial flush error: {e}").into()))?;

        // Wait for I/F Clear to complete
        std::thread::sleep(Duration::from_millis(100));
        Ok(())
    }

    /// Send Address Set command to assign addresses to devices (blocking).
    ///
    /// Returns the number of cameras detected.
    pub fn address_set_blocking<S>(io: &mut S, timeout: Duration) -> Result<u8>
    where
        S: Read + Write + ?Sized,
    {
        let max_attempts = 3;

        for attempt in 0..max_attempts {
            debug!("Address Set attempt {}", attempt + 1);
            let cmd = AddressSetCommand::new();
            let mut buffer = [0u8; 16];

            // AddressSetCommand is const-constructed and guaranteed to encode
            let len = cmd
                .encode_into(CameraId::CAMERA_1, &mut buffer)
                .map_err(|e| {
                    Error::TransportError(format!("Failed to encode Address Set: {e}").into())
                })?;

            io.write_all(&buffer[..len])
                .map_err(|e| Error::TransportError(format!("Serial write error: {e}").into()))?;
            io.flush()
                .map_err(|e| Error::TransportError(format!("Serial flush error: {e}").into()))?;

            // Parse response properly
            match recv_address_set_response_blocking(io, timeout) {
                Ok(camera_count) => {
                    debug!("Address Set successful, found {camera_count} cameras");
                    return Ok(camera_count);
                }
                Err(Error::Timeout) if attempt < max_attempts - 1 => {
                    warn!("Address Set timeout, retrying...");
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::MaxRetriesExceeded)
    }

    /// Receive and parse Address Set response (blocking).
    fn recv_address_set_response_blocking<S>(stream: &mut S, timeout: Duration) -> Result<u8>
    where
        S: Read + ?Sized,
    {
        // Use ProtocolFramer for robust frame handling
        let mut framer = ProtocolFramer::new_with_config(BufferConfig::for_serial());
        let mut response_buffer = Vec::with_capacity(128);
        let mut temp_buf = [0u8; 64];

        let start = Instant::now();

        while start.elapsed() < timeout {
            match stream.read(&mut temp_buf) {
                Ok(n) if n > 0 => {
                    response_buffer.extend_from_slice(&temp_buf[..n]);
                    trace!("Address Set response: {:02X?}", &temp_buf[..n]);

                    // Push data to framer
                    framer.push_slice(&temp_buf[..n])?;

                    // Try to extract frames and parse them
                    for frame_result in framer.drain_frames() {
                        let frame = frame_result?;
                        match parse_address_set_bytes(&frame) {
                            ParseOutcome::Complete { camera_count } => {
                                return Ok(camera_count);
                            }
                            ParseOutcome::Partial { .. } => {
                                // Continue accumulating
                            }
                        }
                    }

                    // Also check accumulated buffer for backward compatibility
                    match parse_address_set_bytes(&response_buffer) {
                        ParseOutcome::Complete { camera_count } => {
                            return Ok(camera_count);
                        }
                        ParseOutcome::Partial { .. } => {
                            // Continue waiting for more data
                        }
                    }
                }
                Ok(_) => {
                    // No data, continue waiting
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Check if we got any cameras before timeout
                    match parse_address_set_bytes(&response_buffer) {
                        ParseOutcome::Partial { camera_count } if camera_count > 0 => {
                            debug!(
                                "Address Set timeout reached, but {} cameras were found",
                                camera_count
                            );
                            return Ok(camera_count);
                        }
                        _ => {
                            debug!("Address Set timeout - no cameras found");
                            return Err(Error::Timeout);
                        }
                    }
                }
                Err(e) => {
                    return Err(Error::TransportError(
                        format!("Error reading Address Set response: {e}").into(),
                    ));
                }
            }
        }

        // Total timeout reached
        match parse_address_set_bytes(&response_buffer) {
            ParseOutcome::Partial { camera_count } if camera_count > 0 => {
                debug!(
                    "Address Set timeout reached, but {} cameras were found",
                    camera_count
                );
                Ok(camera_count)
            }
            _ => {
                debug!("Address Set timeout - no cameras found");
                Err(Error::Timeout)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_address_set_single_camera() {
        // Single camera assigned address
        let data = vec![0x88, 0x30, 0x01, VISCA_TERMINATOR];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Partial { camera_count: 1 });
    }

    #[test]
    fn test_parse_address_set_multiple_cameras() {
        // Multiple cameras assigned addresses, then complete
        let data = vec![
            0x88,
            0x30,
            0x01,
            VISCA_TERMINATOR, // Camera 1
            0x88,
            0x30,
            0x02,
            VISCA_TERMINATOR, // Complete marker (0x02 is always completion, not camera 2)
        ];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 1 });
    }

    #[test]
    fn test_parse_address_set_three_cameras() {
        // Two cameras scenario - cameras respond in order
        let data = vec![
            0x88,
            0x30,
            0x01,
            VISCA_TERMINATOR, // Camera 1
            0x88,
            0x30,
            0x02,
            VISCA_TERMINATOR, // Complete marker (after 1 camera)
        ];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 1 });

        // Three camera sequence - cameras are numbered 1, 2, 3 but camera IDs 1-7 are valid
        let data = vec![
            0x88,
            0x30,
            0x01,
            VISCA_TERMINATOR, // Camera 1
            0x88,
            0x30,
            0x03,
            VISCA_TERMINATOR, // Camera 3 (camera 2 exists, ID 3 assigned)
            0x88,
            0x30,
            0x02,
            VISCA_TERMINATOR, // Complete marker
        ];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 3 });
    }

    #[test]
    fn test_parse_address_set_with_noise() {
        // Data with noise/other bytes
        let data = vec![
            0x00,
            0x11, // Noise
            0x88,
            0x30,
            0x01,
            VISCA_TERMINATOR, // Camera 1
            0xAA,
            0xBB, // More noise
            0x88,
            0x30,
            0x02,
            VISCA_TERMINATOR, // Complete
        ];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 1 });
    }

    #[test]
    fn test_parse_address_set_partial_frame() {
        // Incomplete frame
        let data = vec![0x88, 0x30, 0x01]; // Missing terminator
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Partial { camera_count: 0 });
    }

    #[test]
    fn test_parse_address_set_empty() {
        let data = vec![];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Partial { camera_count: 0 });
    }

    #[test]
    fn test_parse_address_set_fragmented() {
        // Test parsing in fragments
        let mut accumulated = Vec::new();

        // Fragment 1
        accumulated.extend_from_slice(&[0x88, 0x30]);
        let result = parse_address_set_bytes(&accumulated);
        assert_eq!(result, ParseOutcome::Partial { camera_count: 0 });

        // Fragment 2
        accumulated.extend_from_slice(&[0x01, VISCA_TERMINATOR]);
        let result = parse_address_set_bytes(&accumulated);
        assert_eq!(result, ParseOutcome::Partial { camera_count: 1 });

        // Fragment 3 - completion marker
        accumulated.extend_from_slice(&[0x88, 0x30, 0x02, VISCA_TERMINATOR]);
        let result = parse_address_set_bytes(&accumulated);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 1 });
    }
}
