//! # grafton-visca
//!
//! A production-ready Rust implementation of the VISCA over IP protocol for controlling PTZ (Pan-Tilt-Zoom) cameras.
//!
//! ## What is VISCA?
//!
//! VISCA (Video System Control Architecture) is a protocol developed by Sony for controlling PTZ cameras
//! commonly used in robotics, broadcasting, video conferencing, and surveillance applications. This crate
//! implements VISCA over IP, allowing you to control networked PTZ cameras from Rust applications.
//!
//! ## Features
//!
//! - **Complete Command Coverage**: Full support for PTZOptics G2 VISCA commands
//! - **Robust Protocol Handling**: Proper ACK/Completion state machine with socket management
//! - **Multiple Transports**: Both UDP (port 1259 default) and TCP (port 5678 default) support
//! - **Async Support**: Modern async/await API with Tokio (enable with `async` feature)
//! - **Thread Safety**: Safe concurrent access from multiple tasks
//! - **Comprehensive Inquiry**: Query camera state for all supported features
//! - **Error Handling**: Detailed error types for all VISCA error conditions
//!
//! ## Supported Commands
//!
//! ### Camera Movement
//! - Pan/Tilt/Zoom control with absolute and relative positioning
//! - Variable speed control for smooth movements
//! - Home position and preset management (up to 90 presets)
//!
//! ### Exposure & Color
//! - Exposure modes: Auto, Manual, Shutter Priority, Iris Priority, Bright
//! - Iris, shutter speed, gain, and brightness control
//! - White balance modes including manual color temperature
//! - Color adjustments: saturation, hue, RGB gain tuning
//!
//! ### Image Control
//! - Focus control with auto/manual modes and zone selection
//! - Sharpness adjustment with auto/manual modes
//! - Noise reduction (2D and 3D)
//! - Image flip (horizontal/vertical)
//! - Black & white mode
//!
//! ## Example Usage
//!
//! ### Using ViscaClient (Recommended for Thread Safety)
//!
//! ```no_run
//! # #[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
//! # {
//! use grafton_visca::{ViscaClient, UdpTransport};
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//!
//! // Create a thread-safe client
//! let transport = UdpTransport::new("192.168.1.100:5678").unwrap();
//! let client = ViscaClient::new(Box::new(transport));
//!
//! // Send commands through the client
//! client.send(&PanTiltCommand::Home).unwrap();
//! client.send(&ZoomCommand::TeleStandard).unwrap();
//! # }
//! ```
//!
//! ### Direct Transport Usage
//!
//! ```no_run
//! use grafton_visca::{UdpTransport, ViscaCommand, ViscaTransport};
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//!
//! // Connect to camera
//! let mut transport = UdpTransport::new("192.168.1.100:5678").unwrap();
//!
//! // Send Pan/Tilt Home command
//! transport.send_command(&PanTiltCommand::Home).unwrap();
//!
//! // Zoom in
//! transport.send_command(&ZoomCommand::TeleStandard).unwrap();
//! ```
//!
//! ## Advanced Camera Control
//!
//! ```no_run
//! use grafton_visca::command::*;
//! use grafton_visca::{UdpTransport, ViscaTransport};
//!
//! let mut transport = UdpTransport::new("192.168.1.100:5678").unwrap();
//!
//! // Adjust exposure compensation
//! transport.send_command(&ExposureCompensationCommand::Direct(3)).unwrap();
//!
//! // Set iris to F4.0
//! transport.send_command(&IrisCommand::Direct(0x06)).unwrap();
//!
//! // Adjust color saturation to 150%
//! transport.send_command(&SaturationCommand { level: 0x0A }).unwrap();
//! ```
//!
//! ## Async Usage (with `async` feature)
//!
//! The library provides async support for non-blocking camera control:
//!
//! ```no_run
//! # #[cfg(feature = "async-client")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use grafton_visca::{AsyncViscaClient, ViscaResponse};
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//! use grafton_visca::command::pan_tilt::{PanTiltDirection, PanSpeed, TiltSpeed};
//!
//! // Connect to camera
//! let camera = AsyncViscaClient::connect_udp("192.168.1.100:5678").await?;
//!
//! // Send multiple commands concurrently
//! let pan_tilt_cmd = PanTiltCommand::Move {
//!     direction: PanTiltDirection::UpRight,
//!     pan_speed: PanSpeed::new(0x10)?,
//!     tilt_speed: TiltSpeed::new(0x10)?,
//! };
//! let pan_tilt = camera.send(&pan_tilt_cmd);
//! let zoom = camera.send(&ZoomCommand::TeleStandard);
//!
//! // Both commands execute concurrently (respecting the 2-socket limit)
//! let (pan_result, zoom_result) = tokio::join!(pan_tilt, zoom);
//! # Ok(())
//! # }
//! ```
//!
//! ## API Overview
//!
//! The crate provides several levels of API for different use cases:
//!
//! ### High-Level Client
//! - [`ViscaClient`] - Thread-safe wrapper for concurrent camera control (recommended)
//!   - Eliminates need for RefCell in user code
//!   - Supports Clone for sharing between threads
//!   - Provides send(), try_send(), and send_with_timeout() methods
//!
//! ### Transport Layer
//! - [`ViscaTransport`] trait - The core abstraction for sending/receiving commands
//! - [`UdpTransport`] - UDP transport (default port 1259 for VISCA over IP)
//! - [`TcpTransport`] - TCP transport (default port 5678 for VISCA over IP)
//!
//! ### Command Layer
//! - [`ViscaCommand`] trait - Implemented by all command types
//! - Command modules in [`command`] - Organized by functionality
//! - [`send_command_and_wait`] - Main synchronous API for sending commands
//!
//! ### Response Handling
//! - [`ViscaResponse`] - Enum for all response types (ACK, Completion, Inquiry, Error)
//! - [`ViscaInquiryResponse`] - Specific inquiry response variants
//! - [`ViscaError`] - Comprehensive error types for all failure modes
//!
//! ### Async Support (with `async` feature)
//! - `AsyncViscaClient` - High-level async client with automatic socket management
//! - `AsyncViscaTransport` trait - Async version of the transport trait
//!
//! ## Connection Setup
//!
//! Cameras typically listen on standard ports:
//! - **UDP**: Port 1259 (PTZOptics default for VISCA over IP)
//! - **TCP**: Port 5678 (Alternative port, check your camera's configuration)
//!
//! Ensure your camera is configured for VISCA over IP and note its IP address.
//!
//! ## Troubleshooting
//!
//! ### Common Errors
//!
//! - **CommandBufferFull**: The camera can only process 2 commands simultaneously.
//!   Solution: Wait for previous commands to complete before sending new ones.
//!
//! - **NoSocket**: No command is currently executing in the requested socket.
//!   This usually indicates a protocol synchronization issue.
//!
//! - **CommandNotExecutable**: The command cannot be executed in the current camera state.
//!   Example: Trying to zoom while the camera is powered off.
//!
//! - **SyntaxError**: The command format is incorrect or parameters are out of range.
//!   Check that speed values and positions are within valid ranges.
//!
//! ### Best Practices
//!
//! 1. **Connection Management**: Reuse transport instances when possible rather than
//!    creating new connections for each command.
//!
//! 2. **Error Handling**: Always handle errors appropriately - cameras may reject
//!    commands due to mechanical limits or current state.
//!
//! 3. **Timing**: Allow time for mechanical movements to complete. The library handles
//!    protocol-level completion, but physical movement takes time.
//!
//! 4. **Concurrent Commands**: When using async, the library automatically manages
//!    the 2-socket limitation, but be aware that commands may queue.
//!
//! ## Feature Flags
//!
//! - `async` - Enables async/await support with Tokio
//! - `sync` - Enables synchronous API (default)
//! - `full` - Enables both sync and async APIs

use log::{debug, error};
use std::{
    io::{self, Read, Write},
    net::{TcpStream, UdpSocket},
    time::Duration,
};

pub mod command;
pub use command::{
    response::{parse_visca_response, ViscaResponse},
    ViscaCommand, ViscaInquiryResponse, ViscaResponseType,
};

pub mod constants;

// New v0.4.0 transport module - will replace the implementations below in Phase B
pub mod transport;
// NOTE: Not exporting new transports yet to avoid breaking changes.
// The old UdpTransport and TcpTransport below are still in use throughout
// the codebase. They will be replaced with the new transport module in Phase B.

mod camera_detection;
pub use camera_detection::detect_camera_model;

mod error;
pub use error::{AppError, ViscaError};

mod session;
pub use session::ViscaSession;

mod transport_ext;
pub use transport_ext::ViscaTransportExt;

mod inquiry_ext;
pub use inquiry_ext::{
    CameraPosition, CameraState, ExposureState, ImageState, OpticsState, ViscaInquiryExt,
    WhiteBalanceState,
};

mod pan_tilt_ext;
pub use command::pan_tilt::PanTiltDirection;
pub use pan_tilt_ext::ViscaPanTiltExt;

mod zoom_ext;
pub use zoom_ext::ViscaZoomExt;

mod focus_ext;
pub use focus_ext::ViscaFocusExt;

mod preset_ext;
pub use preset_ext::ViscaPresetExt;

// New unified client for v0.4.0
mod unified_client;

// Export ViscaClient based on features
#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
compile_error!("At least one of 'blocking-client' or 'async-client' features must be enabled");

// Use the new unified client for all feature combinations
pub use unified_client::ViscaClient;

#[cfg(feature = "async-client")]
mod async_inquiry_ext;

#[cfg(feature = "async-client")]
mod async_control_ext;

pub mod connection;
#[cfg(feature = "async-client")]
pub use connection::AsyncConnectionManagement;
pub use connection::{ConnectionManagement, ConnectionStats, ConnectionStatsSnapshot};

mod reconnecting_transport;
pub use reconnecting_transport::{
    ConnectionEvent, ConnectionEventCallback, ReconnectingTransport, ReconnectionConfig,
};

mod connection_pool;
pub use connection_pool::{
    CameraInfo, PoolConfig, PooledCameraStats, PooledConnectionGuard, ViscaConnectionPool,
};

#[cfg(feature = "async-client")]
mod async_reconnecting_transport;
#[cfg(feature = "async-client")]
pub use async_reconnecting_transport::{
    AsyncReconnectingTransport, ConnectionEvent as AsyncConnectionEvent,
};

#[cfg(feature = "async-client")]
mod async_connection_pool;
#[cfg(feature = "async-client")]
pub use async_connection_pool::{
    AsyncPoolConfig, AsyncPooledCameraStats, AsyncPooledConnectionGuard, AsyncViscaConnectionPool,
    CameraInfo as AsyncCameraInfo,
};

#[cfg(feature = "async-client")]
mod async_client;
#[cfg(feature = "async-client")]
mod async_tcp_transport;
#[cfg(feature = "async-client")]
pub mod async_transport;
#[cfg(feature = "async-client")]
mod async_udp_transport;

#[cfg(feature = "async-client")]
pub use async_client::AsyncViscaClient;
#[cfg(feature = "async-client")]
pub use async_tcp_transport::AsyncTcpTransport;
#[cfg(feature = "async-client")]
pub use async_transport::{AsyncViscaTransport, TransportFuture};
#[cfg(feature = "async-client")]
pub use async_udp_transport::AsyncUdpTransport;

// sync_wrapper is no longer needed with the unified client

pub mod timeout;
pub use timeout::{CommandCategory, TimeoutConfig, TimeoutConfigBuilder};

/// Transport trait for sending and receiving VISCA commands over a network connection.
///
/// This trait abstracts the underlying transport mechanism (UDP or TCP) and provides
/// a uniform interface for VISCA communication.
///
/// **Note**: This trait will be replaced by `transport::Transport` in v0.4.0.
/// New code should prepare for the migration.
///
/// # Example
/// ```no_run
/// # use grafton_visca::{ViscaTransport, UdpTransport, ViscaCommand, ViscaError};
/// # use grafton_visca::command::PowerCommand;
/// # use grafton_visca::command::power::Power;
/// let mut transport = UdpTransport::new("192.168.1.100:5678")?;
/// let command = PowerCommand { power: Power::On };
/// transport.send_command(&command)?;
/// let responses = transport.receive_response()?;
/// # Ok::<(), ViscaError>(())
/// ```
pub trait ViscaTransport {
    /// Sends a VISCA command to the camera.
    ///
    /// The command is serialized to bytes and transmitted over the transport.
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError>;

    /// Receives response frames from the camera.
    ///
    /// Returns a vector of response frames, where each frame is a complete VISCA
    /// response (starts with 0x90 and ends with 0xFF).
    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError>;

    /// Convenience method that sends a command and waits for completion.
    ///
    /// This method combines `send_command` and the response handling logic
    /// to provide a simpler API for common use cases.
    fn send_and_wait(&mut self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError>
    where
        Self: Sized,
    {
        send_command_and_wait(self, command)
    }
}

/// UDP transport for VISCA over IP communication.
///
/// This transport uses UDP sockets for communication with VISCA cameras.
/// It binds to an ephemeral local port and sends commands to the specified camera address.
///
/// **Note**: This implementation will be replaced by `transport::UdpTransport` in v0.4.0.
///
/// # Example
/// ```no_run
/// # use grafton_visca::UdpTransport;
/// let transport = UdpTransport::new("192.168.1.100:5678")?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct UdpTransport {
    socket: UdpSocket,
    address: String,
    stats: ConnectionStats,
    timeout_duration: Option<Duration>,
    timeout_config: Option<TimeoutConfig>,
}

impl UdpTransport {
    /// Creates a new UDP transport connected to the specified camera address.
    ///
    /// Sets read and write timeouts of 10 seconds.
    ///
    /// # Arguments
    /// * `address` - The camera's IP address and port (e.g., "192.168.1.100:5678")
    ///
    /// # Errors
    /// Returns an error if the socket cannot be created or configured.
    pub fn new(address: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        let timeout = Some(Duration::from_secs(10));
        socket.set_read_timeout(timeout)?;
        socket.set_write_timeout(timeout)?;
        Ok(Self {
            socket,
            address: address.to_string(),
            stats: ConnectionStats::new(),
            timeout_duration: timeout,
            timeout_config: None,
        })
    }

    /// Creates a new UDP transport with custom timeout configuration.
    ///
    /// # Arguments
    /// * `address` - The camera's IP address and port (e.g., "192.168.1.100:5678")
    /// * `timeout_config` - Timeout configuration for different command types
    ///
    /// # Errors
    /// Returns an error if the socket cannot be created or configured.
    pub fn with_timeout_config(address: &str, timeout_config: TimeoutConfig) -> io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        // Set initial timeout to the default timeout
        let timeout = Some(timeout_config.default_timeout);
        socket.set_read_timeout(timeout)?;
        socket.set_write_timeout(timeout)?;
        Ok(Self {
            socket,
            address: address.to_string(),
            stats: ConnectionStats::new(),
            timeout_duration: timeout,
            timeout_config: Some(timeout_config),
        })
    }

    /// Get connection statistics
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Get the timeout configuration
    pub fn timeout_config(&self) -> Option<&TimeoutConfig> {
        self.timeout_config.as_ref()
    }
}

/// TCP transport for VISCA over IP communication.
///
/// This transport uses TCP sockets for reliable communication with VISCA cameras.
/// It maintains a persistent connection to the camera.
///
/// **Note**: This implementation will be replaced by `transport::TcpTransport` in v0.4.0.
///
/// # Example
/// ```no_run
/// # use grafton_visca::TcpTransport;
/// let transport = TcpTransport::new("192.168.1.100:5678")?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub struct TcpTransport {
    stream: TcpStream,
    stats: ConnectionStats,
    timeout_duration: Option<Duration>,
    timeout_config: Option<TimeoutConfig>,
}

impl TcpTransport {
    /// Creates a new TCP transport connected to the specified camera address.
    ///
    /// Establishes a TCP connection and sets read/write timeouts of 30 seconds.
    ///
    /// # Arguments
    /// * `address` - The camera's IP address and port (e.g., "192.168.1.100:5678")
    ///
    /// # Errors
    /// Returns an error if the connection cannot be established or configured.
    pub fn new(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        let timeout = Some(Duration::from_secs(30));
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;
        Ok(Self {
            stream,
            stats: ConnectionStats::new(),
            timeout_duration: timeout,
            timeout_config: None,
        })
    }

    /// Creates a new TCP transport with custom timeout configuration.
    ///
    /// # Arguments
    /// * `address` - The camera's IP address and port (e.g., "192.168.1.100:5678")
    /// * `timeout_config` - Timeout configuration for different command types
    ///
    /// # Errors
    /// Returns an error if the connection cannot be established or configured.
    pub fn with_timeout_config(address: &str, timeout_config: TimeoutConfig) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        // Set initial timeout to the default timeout
        let timeout = Some(timeout_config.default_timeout);
        stream.set_read_timeout(timeout)?;
        stream.set_write_timeout(timeout)?;
        Ok(Self {
            stream,
            stats: ConnectionStats::new(),
            timeout_duration: timeout,
            timeout_config: Some(timeout_config),
        })
    }

    /// Get connection statistics
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Get the timeout configuration
    pub fn timeout_config(&self) -> Option<&TimeoutConfig> {
        self.timeout_config.as_ref()
    }
}

fn parse_response(buffer: &[u8]) -> Result<Vec<Vec<u8>>, ViscaError> {
    let mut responses = Vec::new();
    let mut response = Vec::new();
    let mut start_index = false;

    for &byte in buffer {
        response.push(byte);
        if byte == 0x90 {
            start_index = true;
        } else if byte == 0xFF && start_index {
            responses.push(response.clone());
            response.clear();
            start_index = false;
        }
    }

    if start_index {
        // Log an error if the response format is invalid
        error!("Invalid response format detected: {:02X?}", response);
        return Err(ViscaError::InvalidResponseFormat);
    }

    // Log the number of responses parsed
    debug!("Parsed {} responses from buffer", responses.len());

    Ok(responses)
}

impl ViscaTransport for UdpTransport {
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        // Set timeout based on command category if timeout config is available
        if let Some(ref config) = self.timeout_config {
            let timeout = config.get_timeout(command.command_category());
            self.socket
                .set_read_timeout(Some(timeout))
                .map_err(ViscaError::Io)?;
            self.socket
                .set_write_timeout(Some(timeout))
                .map_err(ViscaError::Io)?;
            self.timeout_duration = Some(timeout);
        }

        let command_bytes = command.to_bytes()?;
        match self
            .socket
            .send_to(&command_bytes, &self.address)
            .map_err(ViscaError::Io)
        {
            Ok(_) => {
                self.stats.record_sent(command_bytes.len());
                Ok(())
            }
            Err(e) => {
                self.stats.record_error();
                Err(e)
            }
        }
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut received_data = Vec::new();

        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((bytes_received, src)) => {
                    debug!(
                        "Received {} bytes from {}: {:02X?}",
                        bytes_received,
                        src,
                        &buffer[..bytes_received]
                    );
                    received_data.extend_from_slice(&buffer[..bytes_received]);
                    if buffer[bytes_received - 1] == 0xFF {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to receive response: {}", e);
                    self.stats.record_error();
                    return Err(ViscaError::Io(e));
                }
            }
        }

        match parse_response(&received_data) {
            Ok(responses) => {
                self.stats.record_received(received_data.len());
                Ok(responses)
            }
            Err(e) => {
                self.stats.record_error();
                Err(e)
            }
        }
    }
}

impl ViscaTransport for TcpTransport {
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        // Set timeout based on command category if timeout config is available
        if let Some(ref config) = self.timeout_config {
            let timeout = config.get_timeout(command.command_category());
            self.stream
                .set_read_timeout(Some(timeout))
                .map_err(ViscaError::Io)?;
            self.stream
                .set_write_timeout(Some(timeout))
                .map_err(ViscaError::Io)?;
            self.timeout_duration = Some(timeout);
        }

        let command_bytes = command.to_bytes()?;
        match self
            .stream
            .write_all(&command_bytes)
            .map_err(ViscaError::Io)
        {
            Ok(_) => {
                debug!("Sent {} bytes: {:02X?}", command_bytes.len(), command_bytes);
                self.stats.record_sent(command_bytes.len());
                Ok(())
            }
            Err(e) => {
                self.stats.record_error();
                Err(e)
            }
        }
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut received_data = Vec::new();

        loop {
            match self.stream.read(&mut buffer) {
                Ok(bytes_received) => {
                    debug!(
                        "Received {} bytes: {:02X?}",
                        bytes_received,
                        &buffer[..bytes_received]
                    );
                    received_data.extend_from_slice(&buffer[..bytes_received]);
                    if buffer[bytes_received - 1] == 0xFF {
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to receive response: {}", e);
                    self.stats.record_error();
                    return Err(ViscaError::Io(e));
                }
            }
        }

        match parse_response(&received_data) {
            Ok(responses) => {
                self.stats.record_received(received_data.len());
                Ok(responses)
            }
            Err(e) => {
                self.stats.record_error();
                Err(e)
            }
        }
    }
}

impl ConnectionManagement for UdpTransport {
    fn is_healthy(&mut self) -> Result<bool, ViscaError> {
        use crate::command::InquiryCommand;

        // Check cached health result first
        if let Some(cached_healthy) = self.stats.get_cached_health() {
            return Ok(cached_healthy);
        }

        // Save the current timeout and set a short one for health check
        let original_timeout = self.timeout_duration;
        let health_check_timeout = Some(Duration::from_secs(1));

        // Set temporary timeout for health check
        if let Err(e) = self.socket.set_read_timeout(health_check_timeout) {
            return Err(ViscaError::Io(e));
        }

        // Send the power inquiry command
        let send_result = self.send_command(&InquiryCommand::Power);

        // Restore original timeout regardless of send result
        if let Err(e) = self.socket.set_read_timeout(original_timeout) {
            // Log error but don't fail the health check for this
            log::warn!("Failed to restore socket timeout: {}", e);
        }

        send_result?;

        // Try to receive response
        match self.receive_response() {
            Ok(responses) => {
                // Validate that we got a power inquiry response
                let healthy = responses.iter().any(|response| {
                    // Power inquiry response format: 0x90 0x50 0x0{2,3} 0xFF
                    response.len() == 4
                        && response[0] == 0x90
                        && response[1] == 0x50
                        && (response[2] == 0x02 || response[2] == 0x03)
                        && response[3] == 0xFF
                });
                self.stats.record_health_check(healthy);
                Ok(healthy)
            }
            Err(ViscaError::Timeout) => {
                self.stats.record_health_check(false);
                Ok(false)
            }
            Err(e) => {
                self.stats.record_health_check(false);
                Err(e)
            }
        }
    }

    fn connection_stats(&self) -> &ConnectionStats {
        &self.stats
    }
}

impl ConnectionManagement for TcpTransport {
    fn is_healthy(&mut self) -> Result<bool, ViscaError> {
        use crate::command::InquiryCommand;

        // Check cached health result first
        if let Some(cached_healthy) = self.stats.get_cached_health() {
            return Ok(cached_healthy);
        }

        // Save the current timeout and set a short one for health check
        let original_timeout = self.timeout_duration;
        let health_check_timeout = Some(Duration::from_secs(1));

        // Set temporary timeout for health check
        if let Err(e) = self.stream.set_read_timeout(health_check_timeout) {
            return Err(ViscaError::Io(e));
        }

        // Send the power inquiry command
        let send_result = self.send_command(&InquiryCommand::Power);

        // Restore original timeout regardless of send result
        if let Err(e) = self.stream.set_read_timeout(original_timeout) {
            // Log error but don't fail the health check for this
            log::warn!("Failed to restore stream timeout: {}", e);
        }

        send_result?;

        // Try to receive response
        match self.receive_response() {
            Ok(responses) => {
                // Validate that we got a power inquiry response
                let healthy = responses.iter().any(|response| {
                    // Power inquiry response format: 0x90 0x50 0x0{2,3} 0xFF
                    response.len() == 4
                        && response[0] == 0x90
                        && response[1] == 0x50
                        && (response[2] == 0x02 || response[2] == 0x03)
                        && response[3] == 0xFF
                });
                self.stats.record_health_check(healthy);
                Ok(healthy)
            }
            Err(ViscaError::Timeout) => {
                self.stats.record_health_check(false);
                Ok(false)
            }
            Err(e) => {
                self.stats.record_health_check(false);
                Err(e)
            }
        }
    }

    fn connection_stats(&self) -> &ConnectionStats {
        &self.stats
    }
}

/// Sends a VISCA command and waits for its completion response.\n///
/// This is the main synchronous API for sending commands to a VISCA camera.
/// It handles the complete command lifecycle including:
/// - Sending the command
/// - Receiving and processing ACK response
/// - Waiting for and returning the completion response
///
/// # Arguments
/// * `transport` - The transport to use for communication
/// * `command` - The VISCA command to send
///
/// # Returns
/// Returns the final response which can be:
/// - `ViscaResponse::Completion` for commands with no data response
/// - `ViscaResponse::InquiryResponse(...)` for inquiry commands
/// - `ViscaResponse::Error(...)` if the camera reports an error
///
/// # Example
/// ```no_run
/// # use grafton_visca::{UdpTransport, send_command_and_wait, ViscaResponse};
/// # use grafton_visca::command::{PowerCommand, power::Power};
/// # let mut transport = UdpTransport::new("192.168.1.100:5678")?;
/// let command = PowerCommand { power: Power::On };
/// match send_command_and_wait(&mut transport, &command)? {
///     ViscaResponse::Completion => println!("Power on successful"),
///     ViscaResponse::Error(e) => println!("Error: {:?}", e),
///     _ => println!("Unexpected response"),
/// }
/// # Ok::<(), grafton_visca::ViscaError>(())
/// ```
pub fn send_command_and_wait(
    transport: &mut dyn ViscaTransport,
    command: &dyn ViscaCommand,
) -> Result<ViscaResponse, ViscaError> {
    // Create a session to manage command state
    let mut session = ViscaSession::new();

    // Assign a socket for this command
    let socket_id = session.assign_socket(command.response_type())?;
    debug!("Sending command on socket {}", socket_id);

    // Send the command
    transport.send_command(command)?;

    // Wait for completion
    loop {
        match transport.receive_response() {
            Ok(responses) => {
                for response in responses {
                    match session.process_response(&response) {
                        Ok(Some((resp_socket_id, parsed_response))) => {
                            // Check if this response is for our command
                            if resp_socket_id == socket_id {
                                match parsed_response {
                                    ViscaResponse::Ack => {
                                        debug!("Command acknowledged on socket {}", socket_id);
                                        // Continue waiting for completion
                                    }
                                    ViscaResponse::Completion => {
                                        debug!("Command completed on socket {}", socket_id);
                                        session.release_socket(socket_id);
                                        return Ok(ViscaResponse::Completion);
                                    }
                                    ViscaResponse::InquiryResponse(inquiry) => {
                                        debug!("Inquiry response received on socket {}", socket_id);
                                        log_inquiry_response(&inquiry);
                                        session.release_socket(socket_id);
                                        return Ok(ViscaResponse::InquiryResponse(inquiry));
                                    }
                                    ViscaResponse::Error(err) => {
                                        error!("Command error on socket {}: {:?}", socket_id, err);
                                        session.release_socket(socket_id);
                                        return Err(err);
                                    }
                                    _ => {
                                        debug!(
                                            "Unexpected response on socket {}: {:?}",
                                            socket_id, parsed_response
                                        );
                                    }
                                }
                            } else {
                                // Response for a different command, log and continue
                                debug!(
                                    "Received response for socket {} (not our socket {})",
                                    resp_socket_id, socket_id
                                );
                            }
                        }
                        Ok(None) => {
                            // Response for unknown socket, ignore
                            debug!("Received response for unknown socket");
                        }
                        Err(e) => {
                            error!("Error processing response: {}", e);
                            session.release_socket(socket_id);
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Transport error: {}", e);
                session.release_socket(socket_id);
                return Err(e);
            }
        }
    }
}

#[allow(unreachable_patterns)]
fn log_inquiry_response(inquiry_response: &ViscaInquiryResponse) {
    match inquiry_response {
        ViscaInquiryResponse::Power { on } => {
            debug!("Power: {}", if *on { "On" } else { "Off" });
        }
        ViscaInquiryResponse::PanTiltPosition { pan, tilt } => {
            debug!("Pan: {}, Tilt: {}", pan, tilt);
        }
        ViscaInquiryResponse::Luminance(luminance) => {
            debug!("Luminance: {}", luminance);
        }
        ViscaInquiryResponse::Contrast(contrast) => {
            debug!("Contrast: {}", contrast);
        }
        ViscaInquiryResponse::ZoomPosition { position } => {
            debug!("Zoom Position: {:02X?}", position);
        }
        ViscaInquiryResponse::FocusPosition { position } => {
            debug!("Focus Position: {:02X?}", position);
        }
        ViscaInquiryResponse::Gain { gain } => {
            debug!("Gain: {}", gain);
        }
        ViscaInquiryResponse::WhiteBalance { mode } => {
            debug!("White Balance Mode: {:?}", mode);
        }
        ViscaInquiryResponse::ExposureMode { mode } => {
            debug!("Exposure Mode: {:?}", mode);
        }
        ViscaInquiryResponse::ExposureCompensation { value } => {
            debug!("Exposure Compensation Value: {}", value);
        }
        ViscaInquiryResponse::Backlight { status } => {
            debug!("Backlight Status: {}", status);
        }
        ViscaInquiryResponse::ColorTemperature { temperature } => {
            debug!("Color Temperature: {}", temperature);
        }
        ViscaInquiryResponse::Hue { hue } => {
            debug!("Hue: {}", hue);
        }
        // Wildcard pattern to handle any future additions to the enum
        _ => {
            debug!("Unhandled inquiry response: {:?}", inquiry_response);
        }
    }
}
