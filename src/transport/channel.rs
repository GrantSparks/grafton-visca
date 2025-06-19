//! Channel-based transport wrapper for thread-safe VISCA communication.
//!
//! This module provides a channel-based transport that allows for safe sharing
//! across threads without requiring explicit locking. Commands are sent through
//! channels to a worker task that manages the underlying transport.

use std::{sync::Arc, time::Duration};

use tokio::sync::{mpsc, oneshot};

use crate::{
    error::Error,
    transport::{Transport, TransportFuture},
    types::SocketId,
    Command,
};

/// Priority level for VISCA commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Low priority - used for non-critical queries
    Low = 0,
    /// Normal priority - default for most commands
    Normal = 1,
    /// High priority - used for time-sensitive operations
    High = 2,
    /// Critical priority - used for emergency stops or safety operations
    Critical = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Configuration for the channel transport.
#[derive(Debug, Clone, Copy)]
pub struct ChannelTransportConfig {
    /// Maximum number of commands that can be queued
    pub queue_size: usize,
    /// Timeout for worker operations
    pub operation_timeout: Duration,
    /// Whether to automatically manage socket IDs
    pub auto_socket_management: bool,
    /// Maximum concurrent commands (typically 2 for VISCA)
    pub max_concurrent_commands: usize,
}

impl Default for ChannelTransportConfig {
    fn default() -> Self {
        Self {
            queue_size: 100,
            operation_timeout: Duration::from_secs(5),
            auto_socket_management: true,
            max_concurrent_commands: 2,
        }
    }
}

/// Command request sent through the channel.
struct CommandRequest {
    /// The command to send
    command: Vec<u8>,
    /// Socket ID to use (or None for automatic assignment)
    socket_id: Option<SocketId>,
    /// Priority of the command
    priority: Priority,
    /// Channel to send the result
    response_tx: oneshot::Sender<Result<(), Error>>,
}

/// Response request sent through the channel.
struct ResponseRequest {
    /// Channel to send the response
    response_tx: oneshot::Sender<Result<(SocketId, Vec<u8>), Error>>,
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
    /// Sender for response requests
    response_tx: mpsc::Sender<ResponseRequest>,
    /// Configuration (retained for future use)
    _config: Arc<ChannelTransportConfig>,
}

impl ChannelTransport {
    /// Create a new channel transport wrapping an existing transport.
    ///
    /// This spawns a worker task that manages the underlying transport.
    pub fn new<T>(transport: T, config: ChannelTransportConfig) -> Self
    where
        T: Transport + 'static,
    {
        let (command_tx, command_rx) = mpsc::channel(config.queue_size);
        let (response_tx, response_rx) = mpsc::channel(config.queue_size);
        let config = Arc::new(config);

        // Spawn the worker task
        let worker_config = config.clone();
        tokio::spawn(async move {
            let worker = TransportWorker::new(transport, worker_config);
            worker.run(command_rx, response_rx).await;
        });

        Self {
            command_tx,
            response_tx,
            _config: config,
        }
    }

    /// Create a new channel transport with default configuration.
    pub fn with_default_config<T>(transport: T) -> Self
    where
        T: Transport + 'static,
    {
        Self::new(transport, ChannelTransportConfig::default())
    }

    /// Set the priority for the next command.
    ///
    /// This returns a `PriorityChannelTransport` that will use the specified
    /// priority for the next command only.
    pub fn with_priority(&self, priority: Priority) -> PriorityChannelTransport {
        PriorityChannelTransport {
            inner: self.clone(),
            priority,
        }
    }
}

/// A channel transport with a specific priority set for the next command.
#[derive(Debug)]
pub struct PriorityChannelTransport {
    inner: ChannelTransport,
    priority: Priority,
}

/// Worker task that manages the underlying transport.
struct TransportWorker<T> {
    /// The underlying transport
    transport: T,
    /// Configuration
    config: Arc<ChannelTransportConfig>,
    /// Currently active commands (socket_id -> command)
    active_commands: std::collections::HashMap<SocketId, Vec<u8>>,
    /// Available socket IDs
    available_sockets: Vec<SocketId>,
}

impl<T: Transport> TransportWorker<T> {
    fn new(transport: T, config: Arc<ChannelTransportConfig>) -> Self {
        let available_sockets = if config.auto_socket_management {
            vec![SocketId::SOCKET_0, SocketId::SOCKET_1]
        } else {
            vec![]
        };

        Self {
            transport,
            config,
            active_commands: std::collections::HashMap::new(),
            available_sockets,
        }
    }

    async fn run(
        mut self,
        mut command_rx: mpsc::Receiver<CommandRequest>,
        mut response_rx: mpsc::Receiver<ResponseRequest>,
    ) {
        // Priority queue for commands
        let mut command_queue = std::collections::BinaryHeap::new();

        loop {
            tokio::select! {
                // Handle command requests
                Some(request) = command_rx.recv() => {
                    self.handle_command_request(request, &mut command_queue).await;
                }

                // Handle response requests
                Some(request) = response_rx.recv() => {
                    self.handle_response_request(request).await;
                }

                // Process queued commands if we have available sockets
                _ = async {}, if !command_queue.is_empty() && self.has_available_socket() => {
                    if let Some(request) = command_queue.pop() {
                        self.process_queued_command(request).await;
                    }
                }

                // Exit if all channels are closed
                else => break,
            }
        }
    }

    async fn handle_command_request(
        &mut self,
        request: CommandRequest,
        queue: &mut std::collections::BinaryHeap<QueuedCommand>,
    ) {
        // Check if we can send immediately or need to queue
        if self.active_commands.len() < self.config.max_concurrent_commands {
            // Get socket ID
            let socket_id = if let Some(id) = request.socket_id {
                id
            } else if let Some(id) = self.get_available_socket() {
                id
            } else {
                // No available socket, queue the command
                queue.push(QueuedCommand {
                    request,
                    sequence: queue.len(),
                });
                return;
            };

            // Send the command
            self.send_command_internal(request.command, socket_id, request.response_tx)
                .await;
        } else {
            // Queue the command
            queue.push(QueuedCommand {
                request,
                sequence: queue.len(),
            });
        }
    }

    async fn handle_response_request(&mut self, request: ResponseRequest) {
        let result = self.transport.receive_response().await;

        // Update active commands based on response
        if let Ok((socket_id, _)) = &result {
            self.active_commands.remove(socket_id);
            if self.config.auto_socket_management {
                self.available_sockets.push(*socket_id);
            }
        }

        let _ = request.response_tx.send(result);
    }

    async fn process_queued_command(&mut self, queued: QueuedCommand) {
        let socket_id = if let Some(id) = queued.request.socket_id {
            id
        } else if let Some(id) = self.get_available_socket() {
            id
        } else {
            let _ = queued.request.response_tx.send(Err(Error::TransportError(
                "No available socket for command".into(),
            )));
            return;
        };

        self.send_command_internal(
            queued.request.command,
            socket_id,
            queued.request.response_tx,
        )
        .await;
    }

    async fn send_command_internal(
        &mut self,
        command: Vec<u8>,
        socket_id: SocketId,
        response_tx: oneshot::Sender<Result<(), Error>>,
    ) {
        // Mark socket as in use
        self.active_commands.insert(socket_id, command.clone());
        if self.config.auto_socket_management {
            self.available_sockets.retain(|&id| id != socket_id);
        }

        // Create a temporary command struct
        struct TempCommand(Vec<u8>);
        impl Command for TempCommand {
            fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                Ok(self.0.clone())
            }
            fn response_type(&self) -> Option<crate::command::ResponseType> {
                None
            }
            fn command_category(&self) -> crate::timeout::CommandCategory {
                crate::timeout::CommandCategory::Quick
            }
        }

        let result = self
            .transport
            .send_command(&TempCommand(command), socket_id)
            .await;
        let _ = response_tx.send(result);
    }

    fn has_available_socket(&self) -> bool {
        self.active_commands.len() < self.config.max_concurrent_commands
    }

    fn get_available_socket(&mut self) -> Option<SocketId> {
        if self.config.auto_socket_management {
            self.available_sockets.pop()
        } else {
            None
        }
    }
}

/// A queued command with priority ordering.
struct QueuedCommand {
    request: CommandRequest,
    sequence: usize,
}

impl PartialEq for QueuedCommand {
    fn eq(&self, other: &Self) -> bool {
        self.request.priority == other.request.priority && self.sequence == other.sequence
    }
}

impl Eq for QueuedCommand {}

impl PartialOrd for QueuedCommand {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedCommand {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first, then earlier sequence
        match self.request.priority.cmp(&other.request.priority) {
            std::cmp::Ordering::Equal => other.sequence.cmp(&self.sequence),
            ordering => ordering,
        }
    }
}

// Transport implementation for ChannelTransport
impl Transport for ChannelTransport {
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
        socket_id: SocketId,
    ) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let command_bytes = command.to_bytes()?;
            let (response_tx, response_rx) = oneshot::channel();

            let request = CommandRequest {
                command: command_bytes,
                socket_id: Some(socket_id),
                priority: Priority::default(),
                response_tx,
            };

            self.command_tx.send(request).await.map_err(|_| {
                Error::TransportError("Channel transport worker disconnected".into())
            })?;

            response_rx
                .await
                .map_err(|_| Error::TransportError("Failed to receive command response".into()))?
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, (SocketId, Vec<u8>)> {
        Box::pin(async move {
            let (response_tx, response_rx) = oneshot::channel();

            let request = ResponseRequest { response_tx };

            self.response_tx.send(request).await.map_err(|_| {
                Error::TransportError("Channel transport worker disconnected".into())
            })?;

            response_rx
                .await
                .map_err(|_| Error::TransportError("Failed to receive response".into()))?
        })
    }
}

// Transport implementation for PriorityChannelTransport
impl Transport for PriorityChannelTransport {
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
        socket_id: SocketId,
    ) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let command_bytes = command.to_bytes()?;
            let (response_tx, response_rx) = oneshot::channel();

            let request = CommandRequest {
                command: command_bytes,
                socket_id: Some(socket_id),
                priority: self.priority,
                response_tx,
            };

            self.inner.command_tx.send(request).await.map_err(|_| {
                Error::TransportError("Channel transport worker disconnected".into())
            })?;

            response_rx
                .await
                .map_err(|_| Error::TransportError("Failed to receive command response".into()))?
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, (SocketId, Vec<u8>)> {
        self.inner.receive_response()
    }
}

// Blanket implementation for Arc<ChannelTransport>
impl Transport for Arc<ChannelTransport> {
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
        socket_id: SocketId,
    ) -> TransportFuture<'a, ()> {
        Box::pin(async move {
            let command_bytes = command.to_bytes()?;
            let (response_tx, response_rx) = oneshot::channel();

            let request = CommandRequest {
                command: command_bytes,
                socket_id: Some(socket_id),
                priority: Priority::default(),
                response_tx,
            };

            self.command_tx.send(request).await.map_err(|_| {
                Error::TransportError("Channel transport worker disconnected".into())
            })?;

            response_rx
                .await
                .map_err(|_| Error::TransportError("Failed to receive command response".into()))?
        })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, (SocketId, Vec<u8>)> {
        Box::pin(async move {
            let (response_tx, response_rx) = oneshot::channel();

            let request = ResponseRequest { response_tx };

            self.response_tx.send(request).await.map_err(|_| {
                Error::TransportError("Channel transport worker disconnected".into())
            })?;

            response_rx
                .await
                .map_err(|_| Error::TransportError("Failed to receive response".into()))?
        })
    }
}

/// Builder for creating a configured ChannelTransport.
#[derive(Debug)]
pub struct ChannelTransportBuilder<T> {
    transport: T,
    config: ChannelTransportConfig,
}

impl<T: Transport + 'static> ChannelTransportBuilder<T> {
    /// Create a new builder with the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            config: ChannelTransportConfig::default(),
        }
    }

    /// Set the queue size.
    pub fn queue_size(mut self, size: usize) -> Self {
        self.config.queue_size = size;
        self
    }

    /// Set the operation timeout.
    pub fn operation_timeout(mut self, timeout: Duration) -> Self {
        self.config.operation_timeout = timeout;
        self
    }

    /// Enable or disable automatic socket management.
    pub fn auto_socket_management(mut self, enabled: bool) -> Self {
        self.config.auto_socket_management = enabled;
        self
    }

    /// Set the maximum number of concurrent commands.
    pub fn max_concurrent_commands(mut self, max: usize) -> Self {
        self.config.max_concurrent_commands = max;
        self
    }

    /// Build the ChannelTransport.
    pub fn build(self) -> ChannelTransport {
        ChannelTransport::new(self.transport, self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Mock transport for testing
    struct MockTransport {
        send_count: Arc<AtomicUsize>,
        receive_count: Arc<AtomicUsize>,
    }

    impl Transport for MockTransport {
        fn send_command<'a>(
            &'a mut self,
            _command: &'a dyn Command,
            _socket_id: SocketId,
        ) -> TransportFuture<'a, ()> {
            self.send_count.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok(()) })
        }

        fn receive_response(&mut self) -> TransportFuture<'_, (SocketId, Vec<u8>)> {
            self.receive_count.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok((SocketId::SOCKET_0, vec![0x90, 0x50, 0xFF])) })
        }
    }

    #[tokio::test]
    async fn test_channel_transport_basic() -> Result<(), Error> {
        let send_count = Arc::new(AtomicUsize::new(0));
        let receive_count = Arc::new(AtomicUsize::new(0));

        let mock = MockTransport {
            send_count: send_count.clone(),
            receive_count: receive_count.clone(),
        };

        let mut transport = ChannelTransport::with_default_config(mock);

        // Test command
        struct TestCommand;
        impl Command for TestCommand {
            fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
            }
            fn response_type(&self) -> Option<crate::command::ResponseType> {
                None
            }
            fn command_category(&self) -> crate::timeout::CommandCategory {
                crate::timeout::CommandCategory::Quick
            }
        }

        let cmd = TestCommand;
        transport.send_command(&cmd, SocketId::SOCKET_0).await?;

        // Allow some time for the worker to process
        tokio::time::sleep(Duration::from_millis(10)).await;

        assert_eq!(send_count.load(Ordering::Relaxed), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_channel_transport_clone() -> Result<(), Error> {
        let mock = MockTransport {
            send_count: Arc::new(AtomicUsize::new(0)),
            receive_count: Arc::new(AtomicUsize::new(0)),
        };

        let transport = ChannelTransport::with_default_config(mock);
        let mut transport1 = transport.clone();
        let mut transport2 = transport;

        // Both clones should work
        struct TestCommand;
        impl Command for TestCommand {
            fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
            }
            fn response_type(&self) -> Option<crate::command::ResponseType> {
                None
            }
            fn command_category(&self) -> crate::timeout::CommandCategory {
                crate::timeout::CommandCategory::Quick
            }
        }

        let cmd = TestCommand;
        transport1.send_command(&cmd, SocketId::SOCKET_0).await?;
        transport2.send_command(&cmd, SocketId::SOCKET_1).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_arc_channel_transport() -> Result<(), Error> {
        let mock = MockTransport {
            send_count: Arc::new(AtomicUsize::new(0)),
            receive_count: Arc::new(AtomicUsize::new(0)),
        };

        let transport = Arc::new(ChannelTransport::with_default_config(mock));
        let mut transport_mut = transport.clone();

        // Arc<ChannelTransport> should implement Transport
        struct TestCommand;
        impl Command for TestCommand {
            fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
            }
            fn response_type(&self) -> Option<crate::command::ResponseType> {
                None
            }
            fn command_category(&self) -> crate::timeout::CommandCategory {
                crate::timeout::CommandCategory::Quick
            }
        }

        let cmd = TestCommand;
        transport_mut.send_command(&cmd, SocketId::SOCKET_0).await?;
        Ok(())
    }
}
