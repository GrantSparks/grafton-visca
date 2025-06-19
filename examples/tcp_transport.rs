//! TCP transport implementation for VISCA over IP with integrated session management.
//!
//! This example shows how to implement the Transport trait for TCP connections.
//! The implementation handles VISCA socket management and response correlation.

// Standard library imports
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use std::{io, time::Duration};

#[cfg(feature = "async-client")]
use std::sync::Arc;

// Crate imports
use grafton_visca::{
    command::response::{parse_response as parse_response_typed, Response, ResponseType},
    types::SocketId,
    Command, Error,
};

#[cfg(feature = "blocking-client")]
use std::{
    io::{Read, Write},
    net::TcpStream,
};

#[cfg(feature = "async-client")]
use grafton_visca::ConnectionStats;

#[cfg(feature = "blocking-client")]
use grafton_visca::transport::BlockingTransport;

// Simple ConnectionStats for blocking implementation since it's not exported for blocking-client
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
#[derive(Debug, Default)]
struct ConnectionStats {
    // Basic stats tracking - you can extend this as needed
    commands_sent: usize,
    responses_received: usize,
}

#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
impl ConnectionStats {
    fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "async-client")]
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream as TokioTcpStream,
};

#[cfg(feature = "async-client")]
use grafton_visca::transport::{Transport, TransportFuture};

/// Tracks pending commands for each socket
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
#[derive(Debug, Clone, Copy)]
struct PendingCommand {
    response_type: Option<ResponseType>,
    acknowledged: bool,
}

/// Blocking TCP transport for VISCA communication with integrated session management.
#[cfg(feature = "blocking-client")]
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
    stats: ConnectionStats,
    // Use fixed array instead of HashMap for the 2 VISCA sockets
    pending_commands: [Option<PendingCommand>; 2],
}

#[cfg(feature = "blocking-client")]
impl TcpTransport {
    /// Creates a new TCP transport connected to the specified camera address.
    ///
    /// Sets read and write timeouts of 10 seconds by default.
    ///
    /// # Arguments
    /// * `address` - The camera's IP address and port (e.g., "192.168.1.100:5678")
    ///
    /// # Errors
    /// Returns an error if the connection cannot be established or configured.
    pub fn new(address: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        Ok(Self {
            stream,
            stats: ConnectionStats::new(),
            pending_commands: [None; 2],
        })
    }

    /// Creates a new TCP transport with a custom timeout.
    ///
    /// # Errors
    /// Returns an error if the connection cannot be established or if the address cannot be parsed.
    pub fn with_timeout(address: &str, timeout: Duration) -> io::Result<Self> {
        let socket_addr = address.parse().map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid address: {e}"))
        })?;
        let stream = TcpStream::connect_timeout(&socket_addr, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(Self {
            stream,
            stats: ConnectionStats::new(),
            pending_commands: [None; 2],
        })
    }

    /// Returns the connection statistics.
    #[must_use]
    pub const fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Assigns an available socket for a new command.
    fn assign_socket(&mut self, response_type: Option<ResponseType>) -> Result<SocketId, Error> {
        // Try socket 0 first, then socket 1
        for (idx, slot) in self.pending_commands.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(PendingCommand {
                    response_type,
                    acknowledged: false,
                });
                let socket_id = if idx == 0 {
                    SocketId::SOCKET_0
                } else {
                    SocketId::SOCKET_1
                };
                log::debug!("Assigned {socket_id} for new command (type: {response_type:?})");
                return Ok(socket_id);
            }
        }
        log::debug!("Cannot assign socket - both sockets are in use");
        Err(Error::CommandBufferFull)
    }

    /// Releases a socket after command completion.
    fn release_socket(&mut self, socket_id: SocketId) {
        let idx = socket_id.value() as usize;
        if idx < 2 {
            self.pending_commands[idx] = None;
            log::debug!("Released {socket_id}");
        }
    }

    /// Process a response and match it to pending commands.
    fn process_response(&mut self, response: &[u8]) -> Result<Option<(SocketId, Response)>, Error> {
        // Validate basic response format
        if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
            return Err(Error::InvalidResponseFormat);
        }

        let response_type = response[1] & 0xF0;
        let socket_raw = response[1] & 0x0F;
        let socket_id = SocketId::new(socket_raw)?;
        let idx = socket_id.value() as usize;

        match response_type {
            // ACK response
            0x40 => {
                if idx < 2 {
                    if let Some(pending) = &mut self.pending_commands[idx] {
                        pending.acknowledged = true;
                        log::debug!("ACK received for {socket_id}");
                        Ok(Some((socket_id, Response::Ack)))
                    } else {
                        log::error!("Received ACK for unknown {socket_id}");
                        Ok(None)
                    }
                } else {
                    log::error!("Received ACK for unknown {socket_id}");
                    Ok(None)
                }
            }

            // Completion response
            0x50 => {
                if idx < 2 {
                    if let Some(pending) = &self.pending_commands[idx] {
                        if response.len() == 3 {
                            log::debug!("Completion received for {socket_id}");
                            Ok(Some((socket_id, Response::Completion)))
                        } else {
                            // Completion with data (inquiry response)
                            match pending.response_type {
                                None => {
                                    log::error!(
                                        "Received data response for non-inquiry command on {socket_id}"
                                    );
                                    Err(Error::UnexpectedResponseType)
                                }
                                Some(resp_type) => match parse_response_typed(response, &resp_type)
                                {
                                    Ok(parsed) => {
                                        log::debug!(
                                            "Inquiry response received for {socket_id}: {parsed:?}"
                                        );
                                        Ok(Some((socket_id, parsed)))
                                    }
                                    Err(e) => {
                                        log::error!("Failed to parse inquiry response: {e}");
                                        Err(e)
                                    }
                                },
                            }
                        }
                    } else {
                        log::error!("Received completion for unknown {socket_id}");
                        Ok(None)
                    }
                } else {
                    log::error!("Received completion for unknown {socket_id}");
                    Ok(None)
                }
            }

            // Error response
            0x60 => {
                if response.len() >= 4 {
                    let error_code = response[2];
                    let error = Error::from_code(error_code);
                    log::error!("Error response for {socket_id}: {error}");
                    Ok(Some((socket_id, Response::Error(error))))
                } else {
                    Err(Error::InvalidResponseFormat)
                }
            }

            _ => {
                log::error!("Unknown response type: {:#02X}", response[1]);
                Err(Error::InvalidResponseFormat)
            }
        }
    }

    /// Internal method to send command with socket ID.
    fn send_with_socket(
        &mut self,
        command: &dyn Command,
        socket_id: SocketId,
    ) -> Result<(), Error> {
        let mut bytes = command.to_bytes()?;

        // Encode socket ID in the command header
        if !bytes.is_empty() && bytes[0] == 0x81 {
            bytes[0] = 0x80 | socket_id.value();
        }

        log::debug!(
            "Sending command with socket {}: {bytes:02X?}",
            socket_id.value()
        );

        self.stream.write_all(&bytes).map_err(Error::Io)?;
        self.stream.flush().map_err(Error::Io)?;
        self.stats.record_sent(bytes.len());
        Ok(())
    }

    /// Internal method to receive a single response.
    fn receive_single_response(&mut self) -> Result<Vec<u8>, Error> {
        let mut buffer = [0u8; 1024];
        let mut current_response = Vec::new();

        loop {
            match self.stream.read(&mut buffer) {
                Ok(0) => {
                    self.stats.record_error();
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Connection closed by camera",
                    )));
                }
                Ok(size) => {
                    for &byte in &buffer[..size] {
                        current_response.push(byte);

                        // Check for end of VISCA frame
                        if byte == 0xFF
                            && current_response.len() >= 3
                            && current_response[0] == 0x90
                        {
                            log::debug!("Received response: {current_response:02X?}");
                            self.stats.record_received(current_response.len());
                            return Ok(current_response);
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if current_response.is_empty() {
                        self.stats.record_error();
                        return Err(Error::CommandTimeout {
                            duration: Duration::from_secs(10),
                            command: "receive_response".to_string(),
                        });
                    }
                    // Incomplete frame
                    self.stats.record_error();
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Incomplete VISCA frame",
                    )));
                }
                Err(e) => {
                    self.stats.record_error();
                    return Err(Error::Io(e));
                }
            }
        }
    }
}

#[cfg(feature = "blocking-client")]
impl BlockingTransport for TcpTransport {
    /// Send a VISCA command and wait for its complete response.
    fn send_command_blocking(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // Assign socket
        let socket_id = self.assign_socket(command.response_type())?;

        // Send command
        if let Err(e) = self.send_with_socket(command, socket_id) {
            self.release_socket(socket_id);
            return Err(e);
        }

        // Wait for complete response
        let mut _received_ack = false;
        let final_response = loop {
            let response_data = match self.receive_single_response() {
                Ok(data) => data,
                Err(e) => {
                    self.release_socket(socket_id);
                    return Err(e);
                }
            };

            match self.process_response(&response_data) {
                Ok(Some((resp_socket_id, response))) if resp_socket_id == socket_id => {
                    match response {
                        Response::Ack => {
                            _received_ack = true;
                            // Continue waiting for completion
                        }
                        Response::Completion | Response::Error(_) => {
                            break Some(response);
                        }
                        // Inquiry responses with data
                        _ => {
                            break Some(response);
                        }
                    }
                }
                Ok(_) => {
                    // Response for different socket or unknown command, ignore
                    continue;
                }
                Err(e) => {
                    self.release_socket(socket_id);
                    return Err(e);
                }
            }
        };

        self.release_socket(socket_id);
        final_response.ok_or(Error::InvalidResponse {
            expected: "Valid response".to_string(),
            actual: vec![],
        })
    }
}

/// Async TCP transport for VISCA communication with integrated session management.
#[cfg(feature = "async-client")]
#[derive(Debug, Clone)]
pub struct AsyncTcpTransport {
    stream: Arc<tokio::sync::Mutex<TokioTcpStream>>,
    stats: Arc<std::sync::Mutex<ConnectionStats>>,
    // Use Arc<Mutex> for async shared state
    pending_commands: Arc<tokio::sync::Mutex<[Option<PendingCommand>; 2]>>,
}

#[cfg(feature = "async-client")]
impl AsyncTcpTransport {
    /// Creates a new async TCP transport.
    ///
    /// # Errors
    /// Returns an error if the connection cannot be established.
    pub async fn new(address: &str) -> io::Result<Self> {
        let stream = TokioTcpStream::connect(address).await?;
        Ok(Self {
            stream: Arc::new(tokio::sync::Mutex::new(stream)),
            stats: Arc::new(std::sync::Mutex::new(ConnectionStats::new())),
            pending_commands: Arc::new(tokio::sync::Mutex::new([None; 2])),
        })
    }

    /// Returns a clone of the connection statistics.
    #[must_use]
    pub fn stats(&self) -> ConnectionStats {
        self.stats.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Assigns an available socket for a new command.
    async fn assign_socket(&self, response_type: Option<ResponseType>) -> Result<SocketId, Error> {
        let mut pending = self.pending_commands.lock().await;

        // Try socket 0 first, then socket 1
        for (idx, slot) in pending.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(PendingCommand {
                    response_type,
                    acknowledged: false,
                });
                let socket_id = if idx == 0 {
                    SocketId::SOCKET_0
                } else {
                    SocketId::SOCKET_1
                };
                log::debug!("Assigned {socket_id} for new command (type: {response_type:?})");
                return Ok(socket_id);
            }
        }
        log::debug!("Cannot assign socket - both sockets are in use");
        Err(Error::CommandBufferFull)
    }

    /// Releases a socket after command completion.
    async fn release_socket(&self, socket_id: SocketId) {
        let mut pending = self.pending_commands.lock().await;
        let idx = socket_id.value() as usize;
        if idx < 2 {
            pending[idx] = None;
            log::debug!("Released {socket_id}");
        }
    }

    /// Process a response and match it to pending commands.
    async fn process_response(
        &self,
        response: &[u8],
    ) -> Result<Option<(SocketId, Response)>, Error> {
        // Validate basic response format
        if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
            return Err(Error::InvalidResponseFormat);
        }

        let response_type = response[1] & 0xF0;
        let socket_raw = response[1] & 0x0F;
        let socket_id = SocketId::new(socket_raw)?;
        let idx = socket_id.value() as usize;

        let mut pending = self.pending_commands.lock().await;

        match response_type {
            // ACK response
            0x40 => {
                if idx < 2 {
                    if let Some(pending_cmd) = &mut pending[idx] {
                        pending_cmd.acknowledged = true;
                        log::debug!("ACK received for {socket_id}");
                        Ok(Some((socket_id, Response::Ack)))
                    } else {
                        log::error!("Received ACK for unknown {socket_id}");
                        Ok(None)
                    }
                } else {
                    log::error!("Received ACK for unknown {socket_id}");
                    Ok(None)
                }
            }

            // Completion response
            0x50 => {
                if idx < 2 {
                    if let Some(pending_cmd) = &pending[idx] {
                        if response.len() == 3 {
                            log::debug!("Completion received for {socket_id}");
                            Ok(Some((socket_id, Response::Completion)))
                        } else {
                            // Completion with data (inquiry response)
                            match pending_cmd.response_type {
                                None => {
                                    log::error!(
                                        "Received data response for non-inquiry command on {socket_id}"
                                    );
                                    Err(Error::UnexpectedResponseType)
                                }
                                Some(resp_type) => match parse_response_typed(response, &resp_type)
                                {
                                    Ok(parsed) => {
                                        log::debug!(
                                            "Inquiry response received for {socket_id}: {parsed:?}"
                                        );
                                        Ok(Some((socket_id, parsed)))
                                    }
                                    Err(e) => {
                                        log::error!("Failed to parse inquiry response: {e}");
                                        Err(e)
                                    }
                                },
                            }
                        }
                    } else {
                        log::error!("Received completion for unknown {socket_id}");
                        Ok(None)
                    }
                } else {
                    log::error!("Received completion for unknown {socket_id}");
                    Ok(None)
                }
            }

            // Error response
            0x60 => {
                if response.len() >= 4 {
                    let error_code = response[2];
                    let error = Error::from_code(error_code);
                    log::error!("Error response for {socket_id}: {error}");
                    Ok(Some((socket_id, Response::Error(error))))
                } else {
                    Err(Error::InvalidResponseFormat)
                }
            }

            _ => {
                log::error!("Unknown response type: {:#02X}", response[1]);
                Err(Error::InvalidResponseFormat)
            }
        }
    }

    /// Internal method to send command with socket ID.
    async fn send_with_socket(
        &self,
        command: &dyn Command,
        socket_id: SocketId,
    ) -> Result<(), Error> {
        let mut bytes = command.to_bytes()?;

        // Encode socket ID in the command header
        if !bytes.is_empty() && bytes[0] == 0x81 {
            bytes[0] = 0x80 | socket_id.value();
        }

        log::debug!(
            "Sending command with socket {}: {bytes:02X?}",
            socket_id.value()
        );

        let mut stream = self.stream.lock().await;
        stream.write_all(&bytes).await.map_err(Error::Io)?;
        stream.flush().await.map_err(Error::Io)?;
        drop(stream);

        if let Ok(stats_guard) = self.stats.lock() {
            stats_guard.record_sent(bytes.len());
        }
        Ok(())
    }

    /// Internal method to receive a single response.
    async fn receive_single_response(&self) -> Result<Vec<u8>, Error> {
        let mut buffer = [0u8; 1024];
        let mut current_response = Vec::new();

        loop {
            let mut stream = self.stream.lock().await;
            match tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buffer)).await {
                Ok(Ok(0)) => {
                    drop(stream);
                    if let Ok(stats_guard) = self.stats.lock() {
                        stats_guard.record_error();
                    }
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Connection closed by camera",
                    )));
                }
                Ok(Ok(size)) => {
                    drop(stream);
                    for &byte in &buffer[..size] {
                        current_response.push(byte);

                        // Check for end of VISCA frame
                        if byte == 0xFF
                            && current_response.len() >= 3
                            && current_response[0] == 0x90
                        {
                            log::debug!("Received response: {current_response:02X?}");

                            if let Ok(stats_guard) = self.stats.lock() {
                                stats_guard.record_received(current_response.len());
                            }
                            return Ok(current_response);
                        }
                    }
                }
                Ok(Err(e)) => {
                    drop(stream);
                    if let Ok(stats_guard) = self.stats.lock() {
                        stats_guard.record_error();
                    }
                    return Err(Error::Io(e));
                }
                Err(_) => {
                    drop(stream);
                    if current_response.is_empty() {
                        if let Ok(stats_guard) = self.stats.lock() {
                            stats_guard.record_error();
                        }
                        return Err(Error::CommandTimeout {
                            duration: Duration::from_secs(10),
                            command: "receive_response".to_string(),
                        });
                    }
                    // Incomplete frame
                    if let Ok(stats_guard) = self.stats.lock() {
                        stats_guard.record_error();
                    }
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Incomplete VISCA frame",
                    )));
                }
            }
        }
    }
}

#[cfg(feature = "async-client")]
impl Transport for AsyncTcpTransport {
    /// Send a VISCA command and wait for its complete response.
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, Response> {
        Box::pin(async move {
            // Assign socket
            let socket_id = self.assign_socket(command.response_type()).await?;

            // Send command
            if let Err(e) = self.send_with_socket(command, socket_id).await {
                self.release_socket(socket_id).await;
                return Err(e);
            }

            // Wait for complete response
            let mut _received_ack = false;
            let final_response = loop {
                let response_data = match self.receive_single_response().await {
                    Ok(data) => data,
                    Err(e) => {
                        self.release_socket(socket_id).await;
                        return Err(e);
                    }
                };

                match self.process_response(&response_data).await {
                    Ok(Some((resp_socket_id, response))) if resp_socket_id == socket_id => {
                        match response {
                            Response::Ack => {
                                _received_ack = true;
                                // Continue waiting for completion
                            }
                            Response::Completion | Response::Error(_) => {
                                break Some(response);
                            }
                            // Inquiry responses with data
                            _ => {
                                break Some(response);
                            }
                        }
                    }
                    Ok(_) => {
                        // Response for different socket or unknown command, ignore
                        continue;
                    }
                    Err(e) => {
                        self.release_socket(socket_id).await;
                        return Err(e);
                    }
                }
            };

            self.release_socket(socket_id).await;
            final_response.ok_or(Error::InvalidResponse {
                expected: "Valid response".to_string(),
                actual: vec![],
            })
        })
    }
}

#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
fn main() {
    eprintln!("This example requires either the 'blocking-client' or 'async-client' feature to be enabled.");
    eprintln!("Run with: cargo run --example tcp_transport --features blocking-client");
    eprintln!("Or:       cargo run --example tcp_transport --features async-client");
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    use grafton_visca::command::zoom::ZoomCommand;
    
    // Create transport
    let mut transport = TcpTransport::new("192.168.1.100:5678")?;
    
    // Send a command
    let command = ZoomCommand::Stop;
    let response = transport.send_command_blocking(&command)?;
    
    println!("Response: {response:?}");
    
    Ok(())
}

#[cfg(all(feature = "async-client", not(feature = "blocking-client")))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    use grafton_visca::command::zoom::ZoomCommand;
    
    // Create transport
    let mut transport = AsyncTcpTransport::new("192.168.1.100:5678").await?;
    
    // Send a command
    let command = ZoomCommand::Stop;
    let response = transport.send_command(&command).await?;
    
    println!("Response: {response:?}");
    
    Ok(())
}