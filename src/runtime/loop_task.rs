//! Main runtime event loop for VISCA communication.

use flume::{Receiver, Sender};
use tracing::{debug, error, instrument, trace, warn};

use std::sync::Arc;

use super::async_adapter::{process_inquiry, AsyncAdapter, MetricsSummary, TxItem};
use crate::{
    command::CommandKind,
    error::{Error, Result},
    protocol::framer::ProtocolFramer,
    runtime::core::PendingCommand,
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
}

/// Main runtime loop with configurable tick interval.
#[instrument(level = "debug", name = "visca_runtime_loop", skip(transport, submit_rx, metrics_rx, executor, config), fields(tick_ms = config.tick_interval_ms))]
pub async fn runtime_loop_with_config<
    T: AsyncTransport + Send + 'static,
    E: crate::executor::Executor,
>(
    mut transport: T,
    submit_rx: Receiver<TxItem>,
    metrics_rx: Receiver<Sender<MetricsSummary>>,
    executor: Arc<E>,
    config: RuntimeLoopConfig,
) -> Result<()> {
    let mut adapter =
        AsyncAdapter::new(config.timeout_config, config.retry_config, executor.clone());
    let mut protocol_framer = ProtocolFramer::new(4096); // Default buffer size

    debug!("VISCA runtime started");

    // Create a timer interval for periodic checks
    let tick_ms = config.tick_interval_ms.unwrap_or(50);
    let tick_duration = std::time::Duration::from_millis(tick_ms);

    loop {
        // Use select! style approach with explicit enum
        enum Operation {
            Recv(Result<bytes::Bytes, Error>),
            Tick,
        }

        // Check for submit items non-blockingly first
        if let Ok(item) = submit_rx.try_recv() {
            match item {
                TxItem::Command { .. } => {
                    // Submit command to adapter
                    adapter.submit(item);

                    // Try to send immediately if possible
                    if let Some(cmd) = adapter.next_command_to_send() {
                        send_command(&mut transport, &mut adapter, cmd, &config).await?;
                    }
                }
                TxItem::Inquiry { .. } => {
                    // Process inquiry directly without scheduler
                    if let Err(e) = process_inquiry(
                        &mut transport,
                        item,
                        &config.envelope,
                        &config.buffer_manager,
                    )
                    .await
                    {
                        error!("Error processing inquiry: {e}");
                    } else {
                        trace!("Inquiry processed successfully");
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
                    // TODO: Implement cancel by ID when core supports it
                    warn!("Cancel by ID not yet implemented for command {}", id);
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
            let pending_cmd = PendingCommand {
                id: retry.id,
                bytes: retry.bytes,
                priority: retry.priority,
                category: retry.category,
                camera_id: retry.camera_id,
                submitted_at: executor.now(),
            };

            send_command(&mut transport, &mut adapter, pending_cmd, &config).await?;
        }

        // Dynamic tick scheduling: sleep until housekeeping tick
        let sleep_dur = tick_duration;

        // Use select to handle both recv and tick operations
        let operation = {
            use futures_lite::future;

            future::or(async { Operation::Recv(transport.recv().await) }, async {
                executor.sleep(sleep_dur).await;
                Operation::Tick
            })
            .await
        };

        match operation {
            Operation::Recv(recv_result) => {
                // Handle received data
                match recv_result {
                    Ok(bytes) => {
                        trace!("Received bytes from transport: {bytes:02X?}");
                        // Push received bytes into the protocol-aware framer
                        protocol_framer.push(bytes);

                        // Drain complete frames without copying
                        for frame in protocol_framer.drain_frames() {
                            // Extract the VISCA payload and metadata using zero-copy method
                            let (payload, meta) =
                                match config.envelope.extract_with_meta_owned(frame) {
                                    Ok(result) => result,
                                    Err(e) => {
                                        warn!("Failed to extract response from frame: {e}");
                                        continue;
                                    }
                                };

                            // Process the response through the adapter
                            if let Err(e) = adapter.process_response(&payload, meta.sequence).await
                            {
                                error!("Error processing response: {e}");
                            }
                        }

                        // Try to send more commands if we can
                        while adapter.can_send_command() {
                            if let Some(cmd) = adapter.next_command_to_send() {
                                send_command(&mut transport, &mut adapter, cmd, &config).await?;
                            } else {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving from transport: {e}");
                        // Handle network error - all pending commands will be retried or failed
                        adapter.handle_network_error().await?;
                    }
                }
            }
            Operation::Tick => {
                // Handle tick - check for timeouts
                adapter.check_timeouts().await?;

                // Try to send more commands if we have room
                while adapter.can_send_command() {
                    if let Some(cmd) = adapter.next_command_to_send() {
                        send_command(&mut transport, &mut adapter, cmd, &config).await?;
                    } else {
                        break;
                    }
                }
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
) -> Result<()> {
    // Determine command kind
    let kind = if cmd.bytes.len() > 1 && cmd.bytes[1] == 0x09 {
        CommandKind::Inquiry
    } else {
        CommandKind::Command
    };

    // Frame the command
    let (framed, meta) = config.envelope.frame_bytes_with_kind_owned(
        cmd.bytes.clone(),
        kind,
        &config.buffer_manager,
    );

    // Register as pending ACK
    adapter.register_pending_ack(&cmd);

    // Register Sony sequence if applicable
    if let Some(sequence) = meta.sequence {
        adapter.register_sequence(cmd.id, sequence);
    }

    // Send the command
    transport.send(&framed).await?;
    trace!("Sent command {} with sequence {:?}", cmd.id, meta.sequence);

    Ok(())
}
