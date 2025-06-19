//! Channel-based transport wrapper for thread-safe VISCA communication.
//!
//! This module provides a channel-based transport that allows for safe sharing
//! across threads without requiring explicit locking.

#[cfg(feature = "async-client")]
use tokio::sync::{mpsc, oneshot};

#[cfg(feature = "async-client")]
use super::{RawTransport, ViscaTransport};
#[cfg(feature = "async-client")]
use crate::{error::Error, Command, Response};

#[cfg(feature = "async-client")]
/// Configuration for the channel transport.
#[derive(Debug, Clone, Copy)]
pub struct ChannelConfig {
    /// Maximum number of commands that can be queued
    pub queue_size: usize,
}

#[cfg(feature = "async-client")]
impl Default for ChannelConfig {
    fn default() -> Self {
        Self { queue_size: 100 }
    }
}

#[cfg(feature = "async-client")]
/// Command request sent through the channel.
struct CommandRequest {
    /// The command to send
    command: Box<dyn Command + Send>,
    /// Channel to send the result
    response_tx: oneshot::Sender<Result<Response, Error>>,
}

#[cfg(feature = "async-client")]
/// Channel-based transport that can be safely shared across threads.
///
/// This transport wrapper uses channels to communicate with a worker task
/// that manages the underlying VISCA transport.
#[derive(Clone, Debug)]
pub struct ChannelTransport {
    /// Sender for command requests
    command_tx: mpsc::Sender<CommandRequest>,
}

#[cfg(feature = "async-client")]
impl ChannelTransport {
    /// Create a new channel transport wrapping an existing VISCA transport.
    ///
    /// This spawns a worker task that manages the underlying transport.
    pub fn new<T: RawTransport + 'static>(
        mut transport: ViscaTransport<T>,
        config: ChannelConfig,
    ) -> Self {
        let (command_tx, mut command_rx) = mpsc::channel::<CommandRequest>(config.queue_size);

        // Spawn the worker task
        tokio::spawn(async move {
            while let Some(request) = command_rx.recv().await {
                // Send command and get response
                let result = transport.send_command(request.command.as_ref()).await;

                // Send result back (ignore if receiver is dropped)
                let _ = request.response_tx.send(result);
            }
        });

        Self { command_tx }
    }

    /// Send a VISCA command and wait for its complete response.
    pub async fn send_command(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // We need to clone the command data since we can't send a reference through channels
        let command_bytes = command.to_bytes()?;
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

        let (response_tx, response_rx) = oneshot::channel();

        let request = CommandRequest {
            command: owned_command,
            response_tx,
        };

        // Send command to worker
        self.command_tx.send(request).await.map_err(|_| {
            Error::TransportError("Channel transport worker disconnected".to_string())
        })?;

        // Wait for response
        response_rx.await.map_err(|_| {
            Error::TransportError("Failed to receive response from worker".to_string())
        })?
    }
}
