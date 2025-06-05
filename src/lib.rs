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
//! # #[cfg(feature = "blocking-client")]
//! # {
//! use grafton_visca::ViscaClient;
//! use grafton_visca::command::{PanTiltCommand, ZoomCommand};
//!
//! // Create a client using the v0.4.0 unified API
//! let client = ViscaClient::connect_udp("192.168.1.100:5678").unwrap();
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
//! ## Creating Custom Commands with Macro
//!
//! For simple commands, you can use the built-in macro to reduce boilerplate:
//!
//! ```no_run
//! use grafton_visca::visca_command;
//!
//! visca_command! {
//!     #[category = "Movement"]
//!     enum CustomCommand {
//!         Home => [0x81, 0x01, 0x06, 0x04, 0xFF],
//!         Reset => [0x81, 0x01, 0x06, 0x05, 0xFF],
//!     }
//! }
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

// Standard library imports
use std::{
    io::{self, Read, Write},
    net::{TcpStream, UdpSocket},
    time::Duration,
};

// Third-party imports
use log::{debug, error};

// Module declarations - Public
pub mod command;
pub mod connection;
pub mod constants;
pub mod macros;
pub mod timeout;
pub mod transport;

// Module declarations - Private
mod camera_detection;
mod connection_pool;
mod error;
mod focus_ext;
mod inquiry_ext;
mod pan_tilt_ext;
mod preset_ext;
mod reconnecting_transport;
mod session;
mod transport_ext;
mod zoom_ext;

// Module declarations - Feature-gated
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod sync_primitives;

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod unified_client;

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
mod ptz_builder;

#[cfg(feature = "async-client")]
mod async_client;

#[cfg(feature = "async-client")]
mod async_connection_pool;

#[cfg(feature = "async-client")]
mod async_control_ext;

#[cfg(feature = "async-client")]
mod async_inquiry_ext;

#[cfg(feature = "async-client")]
mod async_reconnecting_transport;

#[cfg(feature = "async-client")]
mod async_tcp_transport;

#[cfg(feature = "async-client")]
pub mod async_transport;

#[cfg(feature = "async-client")]
mod async_udp_transport;

#[cfg(feature = "async-client")]
mod async_visca_ext;

// Core re-exports
pub use crate::{
    camera_detection::detect_camera_model,
    error::{AppError, ViscaError},
    session::ViscaSession,
};

// Command system re-exports
pub use crate::command::{
    pan_tilt::PanTiltDirection,
    response::{parse_visca_response, ViscaResponse},
    ViscaCommand, ViscaInquiryResponse, ViscaResponseType,
};

// Connection and transport re-exports
pub use crate::{
    connection::{ConnectionManagement, ConnectionStats, ConnectionStatsSnapshot},
    timeout::{CommandCategory, TimeoutConfig, TimeoutConfigBuilder},
};

// Connection pool re-exports
pub use crate::connection_pool::{
    CameraInfo, PoolConfig, PooledCameraStats, PooledConnectionGuard, ViscaConnectionPool,
};

// Extension trait re-exports
pub use crate::{
    focus_ext::ViscaFocusExt,
    inquiry_ext::{
        CameraPosition, CameraState, ExposureState, ImageState, OpticsState, ViscaInquiryExt,
        WhiteBalanceState,
    },
    pan_tilt_ext::ViscaPanTiltExt,
    preset_ext::ViscaPresetExt,
    transport_ext::ViscaTransportExt,
    zoom_ext::ViscaZoomExt,
};

// Reconnecting transport re-exports
pub use crate::reconnecting_transport::{
    ConnectionEvent, ConnectionEventCallback, ReconnectingTransport, ReconnectionConfig,
};

// Feature-gated re-exports - Unified client
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use crate::unified_client::{ViscaClient, ViscaClientPtzExt};

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub use crate::ptz_builder::PtzBuilder;

// Feature-gated re-exports - Async functionality
#[cfg(feature = "async-client")]
pub use crate::{
    async_client::AsyncViscaClient,
    async_tcp_transport::AsyncTcpTransport,
    async_transport::{AsyncViscaTransport, TransportFuture},
    async_udp_transport::AsyncUdpTransport,
    async_visca_ext::{AsyncViscaExt, PanScanDirection},
    connection::AsyncConnectionManagement,
};

// Feature-gated re-exports - Async connection pool
#[cfg(feature = "async-client")]
pub use crate::async_connection_pool::{
    AsyncPoolConfig, AsyncPooledCameraStats, AsyncPooledConnectionGuard, AsyncViscaConnectionPool,
    CameraInfo as AsyncCameraInfo,
};

// Feature-gated re-exports - Async reconnecting transport
#[cfg(feature = "async-client")]
pub use crate::async_reconnecting_transport::{
    AsyncReconnectingTransport, ConnectionEvent as AsyncConnectionEvent,
};

/// Transport trait for sending and receiving VISCA commands over a network connection.
///
/// This trait abstracts the underlying transport mechanism (UDP or TCP) and provides
/// a uniform interface for VISCA communication.
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
    /// Apply timeout configuration for a specific command.
    fn apply_command_timeout(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
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
        Ok(())
    }

    /// Receive data until a VISCA frame end (0xFF) is detected.
    fn receive_until_frame_end(&mut self) -> Result<Vec<u8>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut data = Vec::new();

        loop {
            let (bytes_received, src) = self.socket.recv_from(&mut buffer).map_err(|e| {
                error!("UDP receive error: {}", e);
                self.stats.record_error();
                ViscaError::Io(e)
            })?;

            debug!(
                "Received {} bytes from {}: {:02X?}",
                bytes_received,
                src,
                &buffer[..bytes_received]
            );
            data.extend_from_slice(&buffer[..bytes_received]);

            if bytes_received > 0 && buffer[bytes_received - 1] == 0xFF {
                break;
            }
        }

        Ok(data)
    }
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
    /// Apply timeout configuration for a specific command.
    fn apply_command_timeout(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
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
        Ok(())
    }

    /// Receive data until a VISCA frame end (0xFF) is detected.
    fn receive_until_frame_end(&mut self) -> Result<Vec<u8>, ViscaError> {
        let mut buffer = [0u8; 1024];
        let mut data = Vec::new();

        loop {
            let bytes_received = self.stream.read(&mut buffer).map_err(|e| {
                error!("TCP receive error: {}", e);
                self.stats.record_error();
                ViscaError::Io(e)
            })?;

            debug!(
                "Received {} bytes: {:02X?}",
                bytes_received,
                &buffer[..bytes_received]
            );
            data.extend_from_slice(&buffer[..bytes_received]);

            if bytes_received > 0 && buffer[bytes_received - 1] == 0xFF {
                break;
            }
        }

        Ok(data)
    }
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

/// Parse VISCA response frames from a raw buffer.
/// Each frame starts with 0x90 and ends with 0xFF.
fn parse_response(buffer: &[u8]) -> Result<Vec<Vec<u8>>, ViscaError> {
    let mut responses = Vec::new();
    let mut current_frame = Vec::new();
    let mut in_frame = false;

    for &byte in buffer {
        current_frame.push(byte);

        match byte {
            0x90 => in_frame = true,
            0xFF if in_frame => {
                responses.push(current_frame.clone());
                current_frame.clear();
                in_frame = false;
            }
            _ => {}
        }
    }

    if in_frame {
        error!("Incomplete VISCA frame: {:02X?}", current_frame);
        return Err(ViscaError::InvalidResponseFormat);
    }

    debug!("Parsed {} VISCA frames", responses.len());
    Ok(responses)
}

impl ViscaTransport for UdpTransport {
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        self.apply_command_timeout(command)?;

        let command_bytes = command.to_bytes()?;
        self.socket
            .send_to(&command_bytes, &self.address)
            .map_err(ViscaError::Io)
            .inspect(|_| self.stats.record_sent(command_bytes.len()))
            .inspect_err(|_| self.stats.record_error())
            .map(|_| ())
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let data = self.receive_until_frame_end()?;
        parse_response(&data)
            .inspect(|_| self.stats.record_received(data.len()))
            .inspect_err(|_| self.stats.record_error())
    }
}

impl ViscaTransport for TcpTransport {
    fn send_command(&mut self, command: &dyn ViscaCommand) -> Result<(), ViscaError> {
        self.apply_command_timeout(command)?;

        let command_bytes = command.to_bytes()?;
        self.stream
            .write_all(&command_bytes)
            .map_err(ViscaError::Io)
            .inspect(|_| {
                debug!("Sent {} bytes: {:02X?}", command_bytes.len(), command_bytes);
                self.stats.record_sent(command_bytes.len());
            })
            .inspect_err(|_| self.stats.record_error())
    }

    fn receive_response(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
        let data = self.receive_until_frame_end()?;
        parse_response(&data)
            .inspect(|_| self.stats.record_received(data.len()))
            .inspect_err(|_| self.stats.record_error())
    }
}

impl ConnectionManagement for UdpTransport {
    fn is_healthy(&mut self) -> Result<bool, ViscaError> {
        use crate::command::InquiryCommand;

        // Check cached health result first
        if let Some(cached) = self.stats.get_cached_health() {
            return Ok(cached);
        }

        // Send power inquiry and check response
        self.send_command(&InquiryCommand::Power)?;

        match self.receive_response() {
            Ok(responses) => {
                // Validate power inquiry response: 0x90 0x50 0x0{2,3} 0xFF
                let healthy = responses.iter().any(|r| {
                    r.len() == 4
                        && r[0] == 0x90
                        && r[1] == 0x50
                        && (r[2] == 0x02 || r[2] == 0x03)
                        && r[3] == 0xFF
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
        if let Some(cached) = self.stats.get_cached_health() {
            return Ok(cached);
        }

        // Send power inquiry and check response
        self.send_command(&InquiryCommand::Power)?;

        match self.receive_response() {
            Ok(responses) => {
                // Validate power inquiry response: 0x90 0x50 0x0{2,3} 0xFF
                let healthy = responses.iter().any(|r| {
                    r.len() == 4
                        && r[0] == 0x90
                        && r[1] == 0x50
                        && (r[2] == 0x02 || r[2] == 0x03)
                        && r[3] == 0xFF
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

/// Sends a VISCA command and waits for its completion response.
///
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
    let mut session = ViscaSession::new();
    let socket_id = session.assign_socket(command.response_type())?;

    debug!("Sending command on socket {}", socket_id);
    transport.send_command(command)?;

    let result = wait_for_response(transport, &mut session, socket_id);
    session.release_socket(socket_id);
    result
}

/// Wait for a response on a specific socket.
fn wait_for_response(
    transport: &mut dyn ViscaTransport,
    session: &mut ViscaSession,
    socket_id: u8,
) -> Result<ViscaResponse, ViscaError> {
    use ViscaResponse::*;

    loop {
        let responses = transport.receive_response().map_err(|e| {
            error!("Transport error: {}", e);
            e
        })?;

        for response in responses {
            if let Some((resp_socket_id, parsed_response)) = session.process_response(&response)? {
                if resp_socket_id != socket_id {
                    debug!(
                        "Response for socket {} (expected {})",
                        resp_socket_id, socket_id
                    );
                    continue;
                }

                match parsed_response {
                    Ack => debug!("Command acknowledged on socket {}", socket_id),
                    Completion => {
                        debug!("Command completed on socket {}", socket_id);
                        return Ok(Completion);
                    }
                    InquiryResponse(inquiry) => {
                        debug!("Inquiry response on socket {}", socket_id);
                        log_inquiry_response(&inquiry);
                        return Ok(InquiryResponse(inquiry));
                    }
                    Error(err) => {
                        error!("Command error on socket {}: {:?}", socket_id, err);
                        return Err(err);
                    }
                    _ => debug!("Unexpected response: {:?}", parsed_response),
                }
            }
        }
    }
}

#[allow(unreachable_patterns)]
fn log_inquiry_response(inquiry_response: &ViscaInquiryResponse) {
    use ViscaInquiryResponse::*;

    match inquiry_response {
        Power { on } => debug!("Power: {}", if *on { "On" } else { "Off" }),
        PanTiltPosition { pan, tilt } => debug!("Pan: {}, Tilt: {}", pan, tilt),
        Luminance(val) => debug!("Luminance: {}", val),
        Contrast(val) => debug!("Contrast: {}", val),
        ZoomPosition { position } => debug!("Zoom Position: {:02X?}", position),
        FocusPosition { position } => debug!("Focus Position: {:02X?}", position),
        Gain { gain } => debug!("Gain: {}", gain),
        WhiteBalance { mode } => debug!("White Balance Mode: {:?}", mode),
        ExposureMode { mode } => debug!("Exposure Mode: {:?}", mode),
        ExposureCompensation { value } => debug!("Exposure Compensation: {}", value),
        Backlight { status } => debug!("Backlight: {}", status),
        ColorTemperature { temperature } => debug!("Color Temperature: {}", temperature),
        Hue { hue } => debug!("Hue: {}", hue),
        _ => debug!("Unhandled inquiry response: {:?}", inquiry_response),
    }
}
