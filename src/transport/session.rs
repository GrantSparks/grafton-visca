//! ViscaTransport with interior mutability for async operations.
//!
//! This implementation allows async Camera methods to take &self instead of &mut self,
//! which solves the concurrent execution problem and provides a more ergonomic API.

use crate::{
    command::response::{parse_response as parse_response_typed, Response, ResponseType},
    error::Error,
    sync_primitives::Mutex,
    timeout::{CommandCategory, TimeoutConfig},
    types::SocketId,
    Command,
};
use std::sync::Arc;

/// VISCA transport with interior mutability for async operations.
#[derive(Debug)]
pub struct ViscaTransport<T> {
    inner: Arc<Mutex<T>>,
    state: Arc<Mutex<TransportState>>,
    timeout_config: TimeoutConfig,
}

#[derive(Debug)]
struct TransportState {
    pending_commands: [Option<PendingCommand>; 2],
}

#[derive(Debug, Clone, Copy)]
struct PendingCommand {
    response_type: Option<ResponseType>,
    acknowledged: bool,
}

impl<T> Clone for ViscaTransport<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            state: self.state.clone(),
            timeout_config: self.timeout_config,
        }
    }
}

impl<T> ViscaTransport<T> {
    /// Create a new VISCA transport wrapping a transport.
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
            state: Arc::new(Mutex::new(TransportState {
                pending_commands: [None; 2],
            })),
            timeout_config: TimeoutConfig::default(),
        }
    }

    /// Set custom timeout configuration.
    pub fn with_timeout_config(mut self, config: TimeoutConfig) -> Self {
        self.timeout_config = config;
        self
    }

    /// Get the timeout for a command category.
    pub fn timeout_for(&self, category: CommandCategory) -> std::time::Duration {
        self.timeout_config.get_timeout(category)
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<T> ViscaTransport<T>
where
    T: crate::transport::blocking::Transport,
{
    /// Send a VISCA command and wait for the response.
    pub fn send_command(&self, command: &dyn Command) -> Result<Response, Error> {
        let mut inner = self.inner.lock();
        let mut state = self.state.lock();

        // Prepare command bytes
        let mut data = command.to_bytes()?;

        // Assign socket ID
        let socket_id = assign_socket(&mut state.pending_commands, command.response_type())?;

        // Insert socket ID into command
        if data.len() >= 2 && data[1] == 0x01 {
            data[1] = 0x10 | socket_id.value();
        }

        // Send command
        inner.send(&data)?;

        // Handle response based on type
        let response = if command.response_type().is_some() {
            // Wait for data response
            wait_for_response_blocking(
                &mut *inner,
                &mut state,
                socket_id,
                &self.timeout_config,
                command.command_category(),
            )?
        } else {
            // Wait for completion
            wait_for_completion_blocking(
                &mut *inner,
                &mut state,
                socket_id,
                &self.timeout_config,
                command.command_category(),
            )?
        };

        Ok(response)
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<T> ViscaTransport<T>
where
    T: crate::transport::AsyncTransport,
{
    /// Send a VISCA command and wait for the response.
    pub async fn send_command(&self, command: &dyn Command) -> Result<Response, Error> {
        // Prepare command bytes first (before locking)
        let mut data = command.to_bytes()?;
        let response_type = command.response_type();
        let category = command.command_category();

        // Assign socket ID
        let socket_id = {
            let mut state = self.state.lock().await;
            assign_socket(&mut state.pending_commands, response_type)?
        };

        // Insert socket ID into command
        if data.len() >= 2 && data[1] == 0x01 {
            data[1] = 0x10 | socket_id.value();
        }

        // Send command
        {
            let inner = self.inner.lock().await;
            inner.send(&data).await?;
        }

        // Handle response based on type
        let response = if response_type.is_some() {
            // Wait for data response
            wait_for_response_async(
                self.inner.clone(),
                self.state.clone(),
                socket_id,
                &self.timeout_config,
                category,
            )
            .await?
        } else {
            // Wait for completion
            wait_for_completion_async(
                self.inner.clone(),
                self.state.clone(),
                socket_id,
                &self.timeout_config,
                category,
            )
            .await?
        };

        Ok(response)
    }
}

// Helper functions

fn assign_socket(
    pending_commands: &mut [Option<PendingCommand>; 2],
    response_type: Option<ResponseType>,
) -> Result<SocketId, Error> {
    for (idx, slot) in pending_commands.iter_mut().enumerate() {
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

fn release_socket(pending_commands: &mut [Option<PendingCommand>; 2], socket_id: SocketId) {
    let idx = socket_id.value() as usize;
    if idx < 2 {
        pending_commands[idx] = None;
        log::debug!("Released {socket_id}");
    }
}

#[cfg(not(feature = "async"))]
fn wait_for_response_blocking<T>(
    transport: &mut T,
    state: &mut TransportState,
    socket_id: SocketId,
    timeout_config: &TimeoutConfig,
    category: CommandCategory,
) -> Result<Response, Error>
where
    T: crate::transport::blocking::Transport,
{
    let timeout = timeout_config.get_timeout(category);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            release_socket(&mut state.pending_commands, socket_id);
            return Err(Error::Timeout);
        }

        let data = transport.receive()?;
        match process_response(&mut state.pending_commands, &data)? {
            Some((sid, response)) if sid == socket_id => {
                release_socket(&mut state.pending_commands, socket_id);
                return Ok(response);
            }
            _ => continue,
        }
    }
}

#[cfg(not(feature = "async"))]
fn wait_for_completion_blocking<T>(
    transport: &mut T,
    state: &mut TransportState,
    socket_id: SocketId,
    timeout_config: &TimeoutConfig,
    category: CommandCategory,
) -> Result<Response, Error>
where
    T: crate::transport::blocking::Transport,
{
    let timeout = timeout_config.get_timeout(category);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            release_socket(&mut state.pending_commands, socket_id);
            return Err(Error::Timeout);
        }

        let data = transport.receive()?;
        match process_response(&mut state.pending_commands, &data)? {
            Some((sid, Response::Completion)) if sid == socket_id => {
                release_socket(&mut state.pending_commands, socket_id);
                return Ok(Response::Completion);
            }
            Some((sid, Response::Error(e))) if sid == socket_id => {
                release_socket(&mut state.pending_commands, socket_id);
                return Err(e);
            }
            _ => continue,
        }
    }
}

#[cfg(feature = "async")]
async fn wait_for_response_async<T>(
    transport: Arc<Mutex<T>>,
    state: Arc<Mutex<TransportState>>,
    socket_id: SocketId,
    timeout_config: &TimeoutConfig,
    category: CommandCategory,
) -> Result<Response, Error>
where
    T: crate::transport::AsyncTransport,
{
    // Timeout handled inline using tokio

    let timeout = timeout_config.get_timeout(category);

    #[cfg(feature = "tokio")]
    {
        tokio::time::timeout(timeout, async {
            loop {
                let data = {
                    let transport = transport.lock().await;
                    transport.receive().await?
                };

                let result = {
                    let mut state = state.lock().await;
                    process_response(&mut state.pending_commands, &data)?
                };

                match result {
                    Some((sid, response)) if sid == socket_id => {
                        let mut state = state.lock().await;
                        release_socket(&mut state.pending_commands, socket_id);
                        return Ok(response);
                    }
                    _ => continue,
                }
            }
        })
        .await
        .map_err(|_| Error::Timeout)?
    }

    #[cfg(not(feature = "tokio"))]
    panic!("Async operations require tokio feature")
}

#[cfg(feature = "async")]
async fn wait_for_completion_async<T>(
    transport: Arc<Mutex<T>>,
    state: Arc<Mutex<TransportState>>,
    socket_id: SocketId,
    timeout_config: &TimeoutConfig,
    category: CommandCategory,
) -> Result<Response, Error>
where
    T: crate::transport::AsyncTransport,
{
    // Timeout handled inline using tokio

    let timeout = timeout_config.get_timeout(category);

    #[cfg(feature = "tokio")]
    {
        tokio::time::timeout(timeout, async {
            loop {
                let data = {
                    let transport = transport.lock().await;
                    transport.receive().await?
                };

                let result = {
                    let mut state = state.lock().await;
                    process_response(&mut state.pending_commands, &data)?
                };

                match result {
                    Some((sid, Response::Completion)) if sid == socket_id => {
                        let mut state = state.lock().await;
                        release_socket(&mut state.pending_commands, socket_id);
                        return Ok(Response::Completion);
                    }
                    Some((sid, Response::Error(e))) if sid == socket_id => {
                        let mut state = state.lock().await;
                        release_socket(&mut state.pending_commands, socket_id);
                        return Err(e);
                    }
                    _ => continue,
                }
            }
        })
        .await
        .map_err(|_| Error::Timeout)?
    }

    #[cfg(not(feature = "tokio"))]
    panic!("Async operations require tokio feature")
}

fn process_response(
    pending_commands: &mut [Option<PendingCommand>; 2],
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

    match response_type {
        // ACK response
        0x40 => {
            if idx < 2 {
                if let Some(pending) = &mut pending_commands[idx] {
                    pending.acknowledged = true;
                    log::debug!("Received ACK on {socket_id}");
                }
            }
            Ok(Some((socket_id, Response::Ack)))
        }
        // Completion or Data response
        0x50 => {
            // Check if this is a completion (3 bytes) or data response (more than 3 bytes)
            if response.len() == 3 {
                // Completion response
                log::debug!("Received completion on {socket_id}");
                Ok(Some((socket_id, Response::Completion)))
            } else {
                // Data response
                if idx < 2 {
                    if let Some(pending) = &pending_commands[idx] {
                        if let Some(expected_type) = pending.response_type {
                            match parse_response_typed(
                                &response[2..response.len() - 1],
                                &expected_type,
                            ) {
                                Ok(parsed) => {
                                    log::debug!(
                                        "Received inquiry response on {socket_id}: {parsed:?}"
                                    );
                                    Ok(Some((socket_id, parsed)))
                                }
                                Err(e) => {
                                    log::error!("Failed to parse response: {e:?}");
                                    Err(e)
                                }
                            }
                        } else {
                            log::warn!("Received data response but no type expected");
                            Err(Error::UnexpectedResponseType)
                        }
                    } else {
                        log::warn!("Received response on untracked socket");
                        Ok(None)
                    }
                } else {
                    Err(Error::InvalidParameter("Invalid socket ID".to_string()))
                }
            }
        }
        // Error response
        0x60 => {
            if response.len() >= 4 {
                let error_code = response[2];
                let error = match error_code {
                    0x01 => Error::InvalidResponseLength,
                    0x02 => Error::SyntaxError,
                    0x03 => Error::CommandBufferFull,
                    0x04 => Error::CommandCanceled,
                    0x05 => Error::NoSocket,
                    0x41 => Error::CommandNotExecutable,
                    _ => Error::Unknown(error_code),
                };
                log::debug!("Received error on {socket_id}: {error:?}");
                Ok(Some((socket_id, Response::Error(error))))
            } else {
                Err(Error::InvalidResponseFormat)
            }
        }
        _ => {
            log::warn!("Unknown response type: 0x{response_type:02X}");
            Err(Error::InvalidResponseFormat)
        }
    }
}
