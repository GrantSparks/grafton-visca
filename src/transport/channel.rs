//! Channel-based transport wrapper for thread-safe VISCA communication.
//!
//! This module provides a channel-based transport that allows for safe sharing
//! across threads without requiring explicit locking. Commands are sent through
//! channels to a worker task that manages the underlying transport.

use tokio::sync::{mpsc, oneshot};

use crate::{
    error::Error,
    transport::{Transport, TransportFuture},
    Command, Response,
};

/// Configuration for the channel transport.
#[derive(Debug, Clone, Copy)]
pub struct ChannelTransportConfig {
    /// Maximum number of commands that can be queued
    pub queue_size: usize,
}

impl Default for ChannelTransportConfig {
    fn default() -> Self {
        Self { queue_size: 100 }
    }
}

/// Command request sent through the channel.
struct CommandRequest {
    /// The command to send
    command: Box<dyn Command + Send>,
    /// Channel to send the result
    response_tx: oneshot::Sender<Result<Response, Error>>,
}

/// Channel-based transport that can be safely shared across threads.
///
/// This transport wrapper uses channels to communicate with a worker task
/// that manages the underlying transport. This allows the transport to be
/// cloned and shared without requiring explicit locking.
#[derive(Clone, Debug)]
pub struct ChannelTransport {
    /// Sender for command requests
    command_tx: mpsc::Sender<CommandRequest>,
}

impl ChannelTransport {
    /// Create a new channel transport wrapping an existing transport.
    ///
    /// This spawns a worker task that manages the underlying transport.
    pub fn new<T>(transport: T, config: ChannelTransportConfig) -> Self
    where
        T: Transport + 'static,
    {
        let (command_tx, mut command_rx) = mpsc::channel::<CommandRequest>(config.queue_size);

        // Spawn the worker task
        tokio::spawn(async move {
            let mut transport = transport;

            while let Some(request) = command_rx.recv().await {
                // Send command and get response
                let result = transport.send_command(request.command.as_ref()).await;

                // Send result back (ignore if receiver is dropped)
                let _ = request.response_tx.send(result);
            }
        });

        Self { command_tx }
    }
}

impl Transport for ChannelTransport {
    /// Send a VISCA command and wait for its complete response.
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, Response> {
        // We need to clone the command data since we can't send a reference through channels
        let command_bytes = match command.to_bytes() {
            Ok(bytes) => bytes,
            Err(e) => return Box::pin(async move { Err(e) }),
        };

        let response_type = command.response_type();
        let category = command.command_category();

        // Create a wrapper command that owns the data
        struct OwnedCommand {
            bytes: Vec<u8>,
            response_type: Option<crate::command::ResponseType>,
            category: crate::timeout::CommandCategory,
        }

        impl Command for OwnedCommand {
            fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                Ok(self.bytes.clone())
            }

            fn response_type(&self) -> Option<crate::command::ResponseType> {
                self.response_type
            }

            fn command_category(&self) -> crate::timeout::CommandCategory {
                self.category
            }
        }

        let owned_command = Box::new(OwnedCommand {
            bytes: command_bytes,
            response_type,
            category,
        });

        let command_tx = self.command_tx.clone();

        Box::pin(async move {
            let (response_tx, response_rx) = oneshot::channel();

            let request = CommandRequest {
                command: owned_command,
                response_tx,
            };

            // Send command to worker
            command_tx.send(request).await.map_err(|_| {
                Error::TransportError("Channel transport worker disconnected".to_string())
            })?;

            // Wait for response
            response_rx.await.map_err(|_| {
                Error::TransportError("Failed to receive response from worker".to_string())
            })?
        })
    }
}

/// Builder for creating channel transports with custom configuration.
#[derive(Debug)]
pub struct ChannelTransportBuilder<T> {
    transport: T,
    config: ChannelTransportConfig,
}

impl<T: Transport + 'static> ChannelTransportBuilder<T> {
    /// Create a new builder with default configuration.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            config: ChannelTransportConfig::default(),
        }
    }

    /// Set the queue size for pending commands.
    pub fn queue_size(mut self, size: usize) -> Self {
        self.config.queue_size = size;
        self
    }

    /// Build the channel transport.
    pub fn build(self) -> ChannelTransport {
        ChannelTransport::new(self.transport, self.config)
    }
}
