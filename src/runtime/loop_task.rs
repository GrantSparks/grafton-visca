//! Main runtime event loop for VISCA communication.

use flume::{Receiver, Sender};
use tracing::{debug, error, instrument, trace, warn};

use std::{collections::HashSet, sync::Arc};

use crate::{
    command::CommandKind,
    error::{Error, Result},
    protocol::framer::ProtocolFramer,
    runtime::{
        async_adapter::{AsyncAdapter, MetricsSummary, TxItem},
        core::PendingCommand,
    },
    timeout::TimeoutConfig,
    transport::{buffer::BufferManager, envelope::TransportEnvelope, AsyncTransport, RetryConfig},
};

/// Configuration for the runtime loop.
pub struct RuntimeLoopConfig {
    pub tick_interval_ms: Option<u64>,
    pub envelope: TransportEnvelope,
    pub buffer_manager: BufferManager,
    pub timeout_config: TimeoutConfig,
    pub retry_config: RetryConfig,
    /// Write timeout from transport config
    pub write_timeout: std::time::Duration,
}

/// RAII guard for automatic rollback of send operations on failure.
///
/// This guard ensures that if a send operation fails, any reserved resources
/// (sockets, pending ACK registrations) are automatically rolled back.
struct SendGuard {
    id: u32,
    reserved_socket: Option<crate::visca_socket::ViscaSocket>,
    ack_registered: bool,
    committed: bool,
}

impl SendGuard {
    fn new(id: u32) -> Self {
        Self {
            id,
            reserved_socket: None,
            ack_registered: false,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }

    fn rollback<E: crate::executor::Executor>(self, adapter: &mut AsyncAdapter<E>) {
        if !self.committed {
            // Rollback on failure
            if let Some(socket) = self.reserved_socket {
                debug!(
                    "SendGuard: Rolling back inquiry {} socket reservation",
                    self.id
                );
                adapter.free_socket(socket);
            }
            if self.ack_registered {
                debug!(
                    "SendGuard: Rolling back command {} ACK registration",
                    self.id
                );
                adapter.unregister_pending_ack(self.id);
            }
            // Handle send failure - fails immediately with transport error
            debug!("SendGuard: Handling send failure for {}", self.id);
            adapter.fail_after_send_error(self.id);
        }
    }
}

/// Main runtime loop with configurable tick interval.
#[instrument(level = "debug", name = "visca_runtime_loop", skip(transport, submit_rx, metrics_rx, executor, config), fields(tick_ms = config.tick_interval_ms))]
pub async fn runtime_loop_with_config<
    T: AsyncTransport + Send + 'static,
    E: crate::executor::Executor + Send + Sync + 'static,
>(
    mut transport: T,
    submit_rx: Receiver<TxItem>,
    metrics_rx: Receiver<Sender<MetricsSummary>>,
    executor: Arc<E>,
    config: RuntimeLoopConfig,
) -> Result<()> {
    let mut adapter =
        AsyncAdapter::new(config.timeout_config, config.retry_config, executor.clone());
    let mut protocol_framer = ProtocolFramer::new_with_config(config.buffer_manager.config());
    // Track cancel requests that arrived before the command was bound to a socket
    let mut pending_cancel_ids: HashSet<u32> = HashSet::new();

    // Allocate a single reusable buffer for receiving data
    let mut read_buf = vec![0u8; config.buffer_manager.config().recv_buffer_size];

    debug!("VISCA runtime started");

    // Create a timer interval for periodic checks
    let tick_ms = config.tick_interval_ms.unwrap_or(50);
    let tick_duration = std::time::Duration::from_millis(tick_ms);

    loop {
        // Use select! style approach with explicit enum
        enum Operation {
            RecvOk(usize),
            RecvErr(Error),
            Tick,
        }

        // Check for submit items non-blockingly first
        if let Ok(item) = submit_rx.try_recv() {
            match item {
                TxItem::Command { .. } | TxItem::Inquiry { .. } => {
                    // Submit command or inquiry to adapter
                    adapter.submit(item);

                    // Try to send immediately if possible
                    if let Some(cmd) = adapter.next_command_to_send() {
                        send_command(&mut transport, &mut adapter, cmd, &config, &executor).await?;
                    }
                }
                TxItem::Cancel { socket } => {
                    // Send cancel command
                    use crate::camera_id::CameraId;
                    use crate::command::{encode_visca::ViscaEncode, system::CommandCancelCommand};

                    let cancel_cmd = CommandCancelCommand::new(socket);
                    let mut cancel_bytes = [0u8; 16];

                    // Cancel commands are simple and should always encode successfully
                    // Use unwrap_or to provide a fallback in the extremely unlikely case of failure
                    let len = cancel_cmd
                        .encode_into(CameraId::CAMERA_1, &mut cancel_bytes)
                        .unwrap_or_else(|e| {
                            error!("Failed to encode cancel command: {e}");
                            // Return a minimal valid length to avoid panic
                            3 // Minimum VISCA command length
                        });

                    let kind = CommandKind::Command;
                    let (framed, _meta) = config.envelope.frame_bytes_with_kind_owned(
                        bytes::Bytes::copy_from_slice(&cancel_bytes[..len]),
                        kind,
                        &config.buffer_manager,
                    );
                    transport.send(&framed).await?;
                    debug!("Sent cancel for socket {:?}", socket);
                }
                TxItem::CancelById { id } => {
                    // Find the socket for this command and send cancel
                    if let Some(socket) = adapter.socket_for_command(id) {
                        use crate::camera_id::CameraId;
                        use crate::command::{
                            encode_visca::ViscaEncode, system::CommandCancelCommand,
                        };

                        let cancel_cmd = CommandCancelCommand::new(socket);
                        let mut cancel_bytes = [0u8; 16];

                        let len = cancel_cmd
                            .encode_into(CameraId::CAMERA_1, &mut cancel_bytes)
                            .unwrap_or_else(|e| {
                                error!("Failed to encode cancel command: {e}");
                                3 // Minimum VISCA command length
                            });

                        let kind = CommandKind::Command;
                        let (framed, _meta) = config.envelope.frame_bytes_with_kind_owned(
                            bytes::Bytes::copy_from_slice(&cancel_bytes[..len]),
                            kind,
                            &config.buffer_manager,
                        );
                        transport.send(&framed).await?;
                        debug!("Sent cancel for command {} on socket {:?}", id, socket);
                    } else {
                        debug!("No socket for command {} yet; queuing cancel", id);
                        pending_cancel_ids.insert(id);
                    }
                }
            }
            continue;
        }

        // Check for metrics requests non-blockingly
        if let Ok(response_tx) = metrics_rx.try_recv() {
            let summary = adapter.metrics_summary();
            let _ = response_tx.send(summary);
            continue;
        }

        // Process any ready retries
        let ready_retries = adapter.get_ready_retries();
        for retry in ready_retries {
            debug!(
                "Processing retry for command {} (attempt {})",
                retry.id, retry.attempt
            );

            // Create pending command from retry
            // Determine kind from bytes
            let kind = if retry.bytes.len() > 1 && retry.bytes[1] == 0x09 {
                CommandKind::Inquiry
            } else {
                CommandKind::Command
            };
            let pending_cmd = PendingCommand {
                id: retry.id,
                bytes: retry.bytes,
                priority: retry.priority,
                category: retry.category,
                camera_id: retry.camera_id,
                submitted_at: executor.now(),
                kind,
            };

            send_command(
                &mut transport,
                &mut adapter,
                pending_cmd,
                &config,
                &executor,
            )
            .await?;
        }

        // Dynamic tick scheduling: sleep until housekeeping tick
        let sleep_dur = tick_duration;

        // Use select to handle both recv and tick operations
        let operation = {
            use futures_lite::future;

            // Simplified race: only between recv and tick
            // If a transport chooses to emit Error::Timeout, we'll treat it as idle
            future::race(
                async {
                    match transport.recv_into(&mut read_buf).await {
                        Ok(n) => Operation::RecvOk(n),
                        // If transport emits timeout, treat as idle tick
                        Err(Error::Timeout) => Operation::Tick,
                        Err(err) => Operation::RecvErr(err),
                    }
                },
                async {
                    executor.sleep(sleep_dur).await;
                    Operation::Tick
                },
            )
            .await
        };

        match operation {
            Operation::RecvOk(n) => {
                // Handle received data
                if n == 0 {
                    error!("Connection closed by peer");
                    return Err(Error::ConnectionClosed {
                        reason: Some(std::borrow::Cow::Borrowed("peer closed connection")),
                    });
                }

                trace!("Received {} bytes from transport", n);
                // Push received bytes into the protocol-aware framer using slice
                if let Err(e) = protocol_framer.push_slice(&read_buf[..n]) {
                    warn!("Framer buffer exceeded limits: {e}");
                    continue;
                }

                // Drain complete frames without copying
                for frame_result in protocol_framer.drain_frames() {
                    let frame = match frame_result {
                        Ok(frame) => frame,
                        Err(e) => {
                            warn!("Failed to extract frame: {e}");
                            continue;
                        }
                    };
                    // Extract the VISCA payload and metadata using zero-copy method
                    let (payload, meta) = match config.envelope.extract_with_meta_owned(frame) {
                        Ok(result) => result,
                        Err(e) => {
                            warn!("Failed to extract response from frame: {e}");
                            continue;
                        }
                    };

                    // Process the response through the adapter
                    if let Err(e) = adapter.process_response(&payload, meta.sequence).await {
                        error!("Error processing response: {e}");
                    }
                }

                // Try to send more commands if we can
                while adapter.can_send_command() {
                    if let Some(cmd) = adapter.next_command_to_send() {
                        send_command(&mut transport, &mut adapter, cmd, &config, &executor).await?;
                    } else {
                        break;
                    }
                }

                // After processing responses (e.g., ACKs that bind commands to sockets),
                // flush any queued cancels whose sockets are now known.
                if !pending_cancel_ids.is_empty() {
                    // Collect first to avoid holding a mutable borrow during iteration
                    let ready: Vec<u32> = pending_cancel_ids
                        .iter()
                        .copied()
                        .filter(|id| adapter.socket_for_command(*id).is_some())
                        .collect();

                    for id in ready {
                        if let Some(socket) = adapter.socket_for_command(id) {
                            use crate::camera_id::CameraId;
                            use crate::command::{
                                encode_visca::ViscaEncode, system::CommandCancelCommand,
                            };

                            let cancel_cmd = CommandCancelCommand::new(socket);
                            let mut cancel_bytes = [0u8; 16];
                            let len = cancel_cmd
                                .encode_into(CameraId::CAMERA_1, &mut cancel_bytes)
                                .unwrap_or(3);

                            let kind = CommandKind::Command;
                            let (framed, _meta) = config.envelope.frame_bytes_with_kind_owned(
                                bytes::Bytes::copy_from_slice(&cancel_bytes[..len]),
                                kind,
                                &config.buffer_manager,
                            );
                            transport.send(&framed).await?;
                            debug!(
                                "Sent queued cancel for command {} on socket {:?}",
                                id, socket
                            );

                            // Remove from pending set
                            pending_cancel_ids.remove(&id);
                        }
                    }
                }
            }
            Operation::Tick => {
                // Handle idle tick - check timeouts and send pending commands
                adapter.check_timeouts().await?;

                // Try to send more commands if we have room
                while adapter.can_send_command() {
                    if let Some(cmd) = adapter.next_command_to_send() {
                        send_command(&mut transport, &mut adapter, cmd, &config, &executor).await?;
                    } else {
                        break;
                    }
                }
            }
            Operation::RecvErr(e) => {
                error!("Error receiving from transport: {e}");
                // Handle network error - all pending commands will be retried or failed
                adapter.handle_network_error().await?;
            }
        }
    }
}

/// Helper function to send a command.
async fn send_command<T: AsyncTransport, E: crate::executor::Executor>(
    transport: &mut T,
    adapter: &mut AsyncAdapter<E>,
    cmd: PendingCommand,
    config: &RuntimeLoopConfig,
    executor: &E,
) -> Result<()> {
    // Use command kind from PendingCommand
    let kind = cmd.kind;

    // Frame the command
    let (framed, meta) = config.envelope.frame_bytes_with_kind_owned(
        cmd.bytes.clone(),
        kind,
        &config.buffer_manager,
    );

    // Track reservation state for rollback
    let reserved_socket = None;
    let mut registered_ack = false;

    // For inquiries, start tracking without socket allocation
    // For commands, register as pending ACK
    if kind == CommandKind::Inquiry {
        // Start tracking the inquiry (no socket allocation)
        adapter.start_inquiry(&cmd);
        debug!("Started tracking inquiry {}", cmd.id);
    } else {
        // Register as pending ACK for commands
        adapter.register_pending_ack(&cmd);
        registered_ack = true;
    }

    // Create guard for tracking rollback state
    let mut guard = SendGuard::new(cmd.id);
    guard.reserved_socket = reserved_socket;
    guard.ack_registered = registered_ack;

    // Try to send the command with timeout using manual race
    let write_timeout = config.write_timeout;
    let send_result = {
        use futures_lite::future;

        future::race(async { transport.send(&framed).await.map(|_| ()) }, async {
            executor.sleep(write_timeout).await;
            Err(Error::Timeout)
        })
        .await
    };

    match send_result {
        Ok(()) => {
            // Send succeeded
        }
        Err(e) => {
            // Send failed or timed out
            let error_type = if matches!(e, Error::Timeout) {
                "timeout"
            } else {
                "failed"
            };
            warn!(
                "Send {} for {} {}: {:?}",
                error_type,
                if kind == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                cmd.id,
                e
            );
            guard.rollback(adapter);
            return Ok(());
        }
    }

    // Send succeeded - commit post-send side effects

    // Register Sony sequence if applicable
    if let Some(sequence) = meta.sequence {
        adapter.register_sequence(cmd.id, sequence);
    }

    // Core handles inquiry tracking now

    // Mark as committed to prevent rollback
    guard.commit();

    trace!(
        "Sent {} {} with sequence {:?}",
        if kind == CommandKind::Inquiry {
            "inquiry"
        } else {
            "command"
        },
        cmd.id,
        meta.sequence
    );

    Ok(())
}
