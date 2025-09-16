//! Main runtime event loop for VISCA communication.

use flume::{Receiver, Sender};
use tracing::{debug, error, instrument, trace, warn};

/// Runtime trace logging controlled by RUNTIME_TRACE environment variable
macro_rules! runtime_trace {
    ($($arg:tt)*) => {
        if std::env::var("RUNTIME_TRACE").is_ok() {
            eprintln!("[RUNTIME_TRACE] {}", format!($($arg)*));
        }
    };
}

use std::{collections::HashSet, sync::Arc};

use crate::{
    capabilities::Profile,
    command::{encode_visca::ViscaEncode, CommandKind},
    error::{Error, Result},
    protocol::framer::ProtocolFramer,
    runtime::{
        async_adapter::{AsyncAdapter, CompletionEvent, MetricsSummary, TxItem},
        core::PendingCommand,
        driver::send_one,
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

/// Main runtime loop with configurable tick interval.
#[instrument(level = "debug", name = "visca_runtime_loop", skip(transport, submit_rx, metrics_rx, completions_rx, shutdown_rx, executor, config), fields(tick_ms = config.tick_interval_ms))]
pub async fn runtime_loop_with_config<
    P: Profile + 'static,
    T: AsyncTransport + Send + 'static,
    E: crate::executor::Executor + Send + Sync + 'static,
>(
    mut transport: T,
    submit_rx: Receiver<TxItem>,
    metrics_rx: Receiver<Sender<MetricsSummary>>,
    completions_rx: Receiver<Sender<Receiver<CompletionEvent>>>,
    shutdown_rx: Receiver<()>,
    executor: Arc<E>,
    config: RuntimeLoopConfig,
) -> Result<()> {
    let mut adapter =
        AsyncAdapter::<P, E>::new(config.timeout_config, config.retry_config, executor.clone());
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
            Shutdown,
        }

        // Add runtime trace at loop start
        runtime_trace!(
            "Loop iteration start - pending_ack: {}, can_send: {}, now: {:?}",
            adapter.pending_ack_count(),
            adapter.can_send_command(),
            executor.now()
        );

        // Check for submit items non-blockingly first
        if let Ok(item) = submit_rx.try_recv() {
            match item {
                TxItem::Command { .. } | TxItem::Inquiry { .. } => {
                    // Submit command or inquiry to adapter
                    adapter.submit(item);

                    // Try to send immediately if possible
                    if let Some(cmd) = adapter.next_command_to_send() {
                        // Use the shared driver for sending
                        if let Err(e) = send_one(
                            &mut transport,
                            &executor,
                            &mut adapter,
                            cmd,
                            &config.envelope,
                            &config.buffer_manager,
                            config.write_timeout,
                        )
                        .await
                        {
                            // send_one already rolled back and failed the command via scheduler
                            debug!("Send failed during submit: {e}");
                            // Continue loop; do not stop runtime
                        }
                    }
                }
                TxItem::Cancel { socket } => {
                    // Send cancel command
                    use crate::command::system::CommandCancelCommand;

                    // Get the camera ID for the command on this socket
                    let camera_id = adapter.camera_id_for_socket(socket).unwrap_or_else(|| {
                        warn!("No camera ID found for socket {:?}, using CAMERA_1", socket);
                        crate::camera_id::CameraId::CAMERA_1
                    });

                    let cancel_cmd = CommandCancelCommand::new(socket);
                    // Encode directly to bytes
                    let cancel_bytes = cancel_cmd.try_into_bytes(camera_id).map_err(|e| {
                        error!("Failed to encode cancel command: {e}");
                        e
                    })?;
                    let kind = CommandKind::Command;
                    let (framed, _meta) = config.envelope.frame_bytes_into(
                        &cancel_bytes,
                        kind,
                        &config.buffer_manager,
                    );
                    // Best effort for cancel - don't abort runtime on failure
                    if let Err(e) = transport.send(&framed).await {
                        debug!("Failed to send cancel for socket {:?}: {e}", socket);
                    } else {
                        debug!(
                            "Sent cancel for socket {:?} with camera_id {:?}",
                            socket, camera_id
                        );
                    }
                }
                TxItem::CancelById { id } => {
                    // Find the socket for this command and send cancel
                    if let Some(socket) = adapter.socket_for_command(id) {
                        use crate::command::system::CommandCancelCommand;

                        // Get the camera ID for this specific command
                        let camera_id = adapter.camera_id_for_command(id).unwrap_or_else(|| {
                            warn!("No camera ID found for command {}, using CAMERA_1", id);
                            crate::camera_id::CameraId::CAMERA_1
                        });

                        let cancel_cmd = CommandCancelCommand::new(socket);
                        // Encode directly to bytes
                        let cancel_bytes = cancel_cmd.try_into_bytes(camera_id).map_err(|e| {
                            error!("Failed to encode cancel command: {e}");
                            e
                        })?;
                        let kind = CommandKind::Command;
                        let (framed, _meta) = config.envelope.frame_bytes_into(
                            &cancel_bytes,
                            kind,
                            &config.buffer_manager,
                        );
                        // Best effort for cancel - don't abort runtime on failure
                        if let Err(e) = transport.send(&framed).await {
                            debug!("Failed to send cancel for command {}: {e}", id);
                        } else {
                            debug!(
                                "Sent cancel for command {} on socket {:?} with camera_id {:?}",
                                id, socket, camera_id
                            );
                        }
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

        // Check for completion subscription requests non-blockingly
        if let Ok(response_tx) = completions_rx.try_recv() {
            let completion_rx = adapter.subscribe_completions();
            let _ = response_tx.send(completion_rx);
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
            // Use the kind preserved from the original command
            let kind = retry.kind;
            let pending_cmd = PendingCommand {
                id: retry.id,
                command: retry.command,
                priority: retry.priority,
                category: retry.category,
                camera_id: retry.camera_id,
                submitted_at: executor.now(),
                kind,
            };

            // Use the shared driver for sending retries
            if let Err(e) = send_one(
                &mut transport,
                &executor,
                &mut adapter,
                pending_cmd,
                &config.envelope,
                &config.buffer_manager,
                config.write_timeout,
            )
            .await
            {
                // send_one already rolled back and failed the command via scheduler
                debug!("Send failed during retry: {e}");
                // Continue loop; do not stop runtime
            }
        }

        // Dynamic tick scheduling: sleep until housekeeping tick
        let sleep_dur = tick_duration;

        // Use select to handle recv, tick, and shutdown operations
        let operation = {
            use futures_lite::future;

            // First create the two-way race between recv and sleep
            let recv_or_sleep = future::race(
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
            );

            // Now race shutdown against (recv | sleep). This is the crucial bit.
            future::race(
                async {
                    // This future *registers a waker* and will wake the loop as soon as a
                    // shutdown signal is sent *or* the sender side is dropped.
                    let _ = shutdown_rx.recv_async().await;
                    Operation::Shutdown
                },
                recv_or_sleep,
            )
            .await
        };

        runtime_trace!(
            "Race winner: {:?}",
            match &operation {
                Operation::RecvOk(n) => format!("RecvOk({})", n),
                Operation::RecvErr(_) => "RecvErr".to_string(),
                Operation::Tick => "Tick".to_string(),
                Operation::Shutdown => "Shutdown".to_string(),
            }
        );

        match operation {
            Operation::Shutdown => {
                debug!("Shutdown won race; exiting runtime loop");
                runtime_trace!("Shutdown won race; exiting runtime loop cleanly");
                return Ok(());
            }
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
                        // Use the shared driver for sending
                        if let Err(e) = send_one(
                            &mut transport,
                            &executor,
                            &mut adapter,
                            cmd,
                            &config.envelope,
                            &config.buffer_manager,
                            config.write_timeout,
                        )
                        .await
                        {
                            // send_one already rolled back and failed the command via scheduler
                            debug!("Send failed while draining pending: {e}");
                            // Continue loop; do not stop runtime
                        }
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
                            use crate::command::{
                                encode_visca::ViscaEncode, system::CommandCancelCommand,
                            };

                            // Get the camera ID for this specific command
                            let camera_id = adapter.camera_id_for_command(id)
                                .unwrap_or_else(|| {
                                    warn!("No camera ID found for queued cancel command {}, using CAMERA_1", id);
                                    crate::camera_id::CameraId::CAMERA_1
                                });

                            let cancel_cmd = CommandCancelCommand::new(socket);

                            // Encode with the actual camera ID
                            let cancel_bytes =
                                cancel_cmd.try_into_bytes(camera_id).map_err(|e| {
                                    error!("Failed to encode queued cancel command: {e}");
                                    e
                                })?;

                            let kind = CommandKind::Command;
                            let (framed, _meta) = config.envelope.frame_bytes_into(
                                &cancel_bytes,
                                kind,
                                &config.buffer_manager,
                            );
                            // Best effort for cancel - don't abort runtime on failure
                            if let Err(e) = transport.send(&framed).await {
                                debug!("Failed to send queued cancel for command {}: {e}", id);
                            } else {
                                debug!(
                                    "Sent queued cancel for command {} on socket {:?} with camera_id {:?}",
                                    id, socket, camera_id
                                );
                            }

                            // Remove from pending set
                            pending_cancel_ids.remove(&id);
                        }
                    }
                }
            }
            Operation::Tick => {
                // Handle idle tick - check timeouts and send pending commands
                runtime_trace!(
                    "Before check_timeouts: pending_ack={}",
                    adapter.pending_ack_count()
                );
                adapter.check_timeouts().await?;
                runtime_trace!(
                    "After check_timeouts: pending_ack={}",
                    adapter.pending_ack_count()
                );

                // Try to send more commands if we have room
                while adapter.can_send_command() {
                    if let Some(cmd) = adapter.next_command_to_send() {
                        // Use the shared driver for sending
                        if let Err(e) = send_one(
                            &mut transport,
                            &executor,
                            &mut adapter,
                            cmd,
                            &config.envelope,
                            &config.buffer_manager,
                            config.write_timeout,
                        )
                        .await
                        {
                            // send_one already rolled back and failed the command via scheduler
                            debug!("Send failed during tick: {e}");
                            // Continue loop; do not stop runtime
                        }
                    } else {
                        break;
                    }
                }

                // NOTE: No sleep here! The tick came from the timeout above, sleeping again
                // would cause the observed 2× slowdown (double-sleep issue).
            }
            Operation::RecvErr(e) => {
                error!("Error receiving from transport: {e}");
                // Handle network error - all pending commands will be retried or failed
                adapter.on_network_error(e).await?;

                // Avoid hot-looping on immediate errors; give timers a chance to fire.
                // This sleep prevents the loop from immediately re-entering the race and
                // canceling any pending sleep futures, ensuring virtual time can advance.
                executor.sleep(tick_duration).await;
            }
        }
    }
}
