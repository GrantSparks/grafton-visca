//! Main runtime event loop for VISCA communication.

use flume::{Receiver, Sender};
use futures_lite::future;
use tracing::{debug, error, instrument, trace, warn};

use std::sync::Arc;

use crate::{
    capabilities::Profile,
    command::{encode::ViscaCommand, system::CommandCancelCommand, CommandKind},
    error::{Error, Result},
    protocol::framer::ProtocolFramer,
    runtime::{
        async_adapter::{AsyncAdapter, CompletionEvent, MetricsSummary, TxItem},
        core::PendingCommand,
        driver::send_one,
    },
    timeout::TimeoutConfig,
    transport::{
        buffer::BufferManager, envelope::Envelope, AsyncTransport, RetryConfig, SendSemantics,
    },
};

/// Event variants representing all possible wake sources for the runtime loop.
///
/// This enum centralizes all wake sources in a type-safe manner, preventing
/// future regressions where a new control path might be added but not included
/// in the wake set.
enum LoopEvent {
    /// A command, inquiry, or cancel was submitted.
    Submit(TxItem),
    /// A metrics request was received.
    Metrics(Sender<MetricsSummary>),
    /// A completion subscription request was received.
    SubscribeCompletions(Sender<Receiver<CompletionEvent>>),
    /// Data was received from the transport.
    TransportRecv(usize),
    /// Transport receive returned an error.
    TransportErr(Error),
    /// A timer tick occurred (deadline elapsed or idle sleep).
    Tick,
    /// Shutdown was requested.
    Shutdown,
}

/// Runtime trace logging controlled by RUNTIME_TRACE environment variable
macro_rules! runtime_trace {
    ($($arg:tt)*) => {
        if std::env::var("RUNTIME_TRACE").is_ok() {
            eprintln!("[RUNTIME_TRACE] {}", format!($($arg)*));
        }
    };
}

/// Handle a send failure based on transport semantics.
///
/// For stream transports (TCP, Serial), a send failure can leave the byte stream
/// in an unknown state (partial write). This macro poisons the transport and
/// returns an error from the enclosing function.
///
/// For datagram transports (UDP), the individual command fails but the transport
/// can continue operating.
///
/// # Usage
/// ```ignore
/// handle_send_failure!(transport, adapter, error, "context message");
/// ```
macro_rules! handle_send_failure {
    ($transport:expr, $adapter:expr, $error:expr, $context:expr) => {
        if $transport.send_semantics() == SendSemantics::Stream {
            let reason = format!("{}: {}", $context, $error);
            error!(
                send_semantics = ?$transport.send_semantics(),
                reason = %reason,
                "Stream transport poisoned - failing all pending commands and exiting"
            );
            let failed_count = $adapter.poison_transport(reason.clone());
            debug!(failed_count, "Poisoned transport and failed pending commands");
            return Err(Error::StreamPoisoned {
                reason: reason.into(),
            });
        } else {
            debug!(
                send_semantics = ?$transport.send_semantics(),
                error = %$error,
                "Datagram send failed - continuing with next command"
            );
        }
    };
}

/// Configuration for the runtime loop.
pub struct RuntimeLoopConfig<E: Envelope> {
    pub envelope: E,
    pub buffer_manager: BufferManager,
    pub timeout_config: TimeoutConfig,
    pub retry_config: RetryConfig,
    /// Write timeout from transport config
    pub write_timeout: std::time::Duration,
    /// Maximum concurrent inquiries.
    ///
    /// For envelopes without sequence correlation (e.g., Raw VISCA), this should
    /// be set to 1 to ensure responses can be reliably matched to requests.
    /// For envelopes with sequence correlation (e.g., Sony Encapsulated), higher
    /// values enable concurrent inquiry execution.
    pub max_concurrent_inquiries: usize,
    /// Minimum time spacing between consecutive inquiry sends.
    ///
    /// Some cameras (e.g., PTZOptics) cannot process inquiries faster than
    /// ~125-150ms apart. Setting this enforces a minimum delay between sends.
    /// Default: Duration::ZERO (no artificial spacing)
    pub min_inquiry_spacing: std::time::Duration,
    /// Minimum time spacing between consecutive sends of any kind.
    ///
    /// Consumer PTZ cameras have small internal command buffers that overflow
    /// when commands are sent back-to-back at wire speed, causing spurious
    /// syntax errors and dropped completions. This interval is enforced at
    /// the transport layer for all sends (commands and inquiries).
    /// Default: Duration::ZERO (no artificial spacing)
    pub min_command_spacing: std::time::Duration,
    /// Maximum pending queue depth for admission control.
    ///
    /// This bounds the number of commands/inquiries that can be queued
    /// in the scheduler. When at capacity, new submissions are rejected
    /// with a retryable `RuntimeQueueFull` error.
    pub max_pending_queue_depth: usize,
}

/// Main runtime loop with configurable tick interval.
#[instrument(
    level = "debug",
    name = "visca_runtime_loop",
    skip(
        transport,
        submit_rx,
        metrics_rx,
        completions_rx,
        shutdown_rx,
        executor,
        config
    )
)]
pub async fn runtime_loop_with_config<
    P: Profile + 'static,
    T: AsyncTransport + Send + 'static,
    Ex: crate::executor::Executor + Send + Sync + 'static,
>(
    mut transport: T,
    submit_rx: Receiver<TxItem>,
    metrics_rx: Receiver<Sender<MetricsSummary>>,
    completions_rx: Receiver<Sender<Receiver<CompletionEvent>>>,
    shutdown_rx: Receiver<()>,
    executor: Arc<Ex>,
    config: RuntimeLoopConfig<P::Envelope>,
) -> Result<()> {
    let mut adapter = AsyncAdapter::<P, Ex>::new(
        config.timeout_config,
        config.retry_config,
        executor.clone(),
        config.max_pending_queue_depth,
    );
    adapter.set_max_inquiries_inflight(config.max_concurrent_inquiries);
    adapter.set_min_inquiry_spacing(config.min_inquiry_spacing);
    adapter.set_min_command_spacing(config.min_command_spacing);
    let mut protocol_framer = ProtocolFramer::new_with_config(config.buffer_manager.config());

    // Allocate a single reusable buffer for receiving data
    let mut read_buf = vec![0u8; config.buffer_manager.config().recv_buffer_size];

    // Allocate a single reusable buffer for sending data (zero allocation per send)
    let mut send_buf =
        bytes::BytesMut::with_capacity(config.buffer_manager.config().send_buffer_size);

    debug!("VISCA runtime started");

    // Default idle sleep duration when no deadlines are pending
    const DEFAULT_IDLE_SLEEP: std::time::Duration = std::time::Duration::from_millis(100);

    loop {
        runtime_trace!(
            "Loop iteration start - pending_ack: {}, can_send: {}, now: {:?}",
            adapter.pending_ack_count(),
            adapter.can_send_command(),
            executor.now()
        );

        // Compute the next deadline for time-based operations
        let now = executor.now();
        let until = adapter.next_deadline().unwrap_or(now + DEFAULT_IDLE_SLEEP);
        let sleep_dur = until.saturating_duration_since(now);

        // Await ALL wake sources in a single select point:
        // - shutdown_rx: shutdown signal
        // - submit_rx: new commands/inquiries/cancels
        // - metrics_rx: metrics requests
        // - completions_rx: completion subscription requests
        // - transport recv or deadline tick
        //
        // This ensures new submissions/control requests wake the loop immediately,
        // regardless of how far away the next deadline is.
        let event = {
            // Build transport recv or sleep future
            let recv_or_tick = future::race(
                async {
                    match transport.recv_into(&mut read_buf).await {
                        Ok(n) => LoopEvent::TransportRecv(n),
                        // If transport emits timeout, treat as idle tick
                        Err(Error::Timeout) => LoopEvent::Tick,
                        Err(err) => LoopEvent::TransportErr(err),
                    }
                },
                async {
                    executor.sleep(sleep_dur).await;
                    LoopEvent::Tick
                },
            );

            // Race all control channels + shutdown + transport
            // Use nested races since futures_lite::future::race only takes two futures
            let shutdown_future = async {
                let _ = shutdown_rx.recv_async().await;
                LoopEvent::Shutdown
            };
            let submit_future = async {
                match submit_rx.recv_async().await {
                    Ok(item) => LoopEvent::Submit(item),
                    // Channel closed - treat as shutdown
                    Err(_) => LoopEvent::Shutdown,
                }
            };
            let metrics_future = async {
                match metrics_rx.recv_async().await {
                    Ok(tx) => LoopEvent::Metrics(tx),
                    // Channel closed - treat as tick (non-fatal)
                    Err(_) => LoopEvent::Tick,
                }
            };
            let completions_future = async {
                match completions_rx.recv_async().await {
                    Ok(tx) => LoopEvent::SubscribeCompletions(tx),
                    // Channel closed - treat as tick (non-fatal)
                    Err(_) => LoopEvent::Tick,
                }
            };

            // Combine all futures using nested races
            // Priority order matters for ties: shutdown > submit > metrics > completions > recv/tick
            future::race(
                shutdown_future,
                future::race(
                    submit_future,
                    future::race(
                        metrics_future,
                        future::race(completions_future, recv_or_tick),
                    ),
                ),
            )
            .await
        };

        runtime_trace!(
            "Event received: {}",
            match &event {
                LoopEvent::Submit(item) => format!("Submit({:?})", item),
                LoopEvent::Metrics(_) => "Metrics".to_string(),
                LoopEvent::SubscribeCompletions(_) => "SubscribeCompletions".to_string(),
                LoopEvent::TransportRecv(n) => format!("TransportRecv({n})"),
                LoopEvent::TransportErr(_) => "TransportErr".to_string(),
                LoopEvent::Tick => "Tick".to_string(),
                LoopEvent::Shutdown => "Shutdown".to_string(),
            }
        );

        // Handle the event - note: NO early `continue` statements here
        // All paths fall through to the housekeeping section at the end
        match event {
            LoopEvent::Shutdown => {
                debug!("Shutdown event received; exiting runtime loop");
                runtime_trace!("Shutdown event received; exiting runtime loop cleanly");
                return Ok(());
            }

            LoopEvent::Submit(item) => {
                match item {
                    TxItem::Command { .. } | TxItem::Inquiry { .. } => {
                        adapter.submit(item);

                        // Try to send immediately if possible
                        if let Some(cmd) = adapter.next_item_to_send() {
                            if let Err(e) = send_one(
                                &mut transport,
                                &executor,
                                &mut adapter,
                                cmd,
                                &config.envelope,
                                &mut send_buf,
                                config.write_timeout,
                            )
                            .await
                            {
                                handle_send_failure!(
                                    transport,
                                    adapter,
                                    e,
                                    "Send failed during submit"
                                );
                            }
                        }
                    }
                    TxItem::Cancel { camera_id, socket } => {
                        let cancel_cmd = CommandCancelCommand::new(socket);
                        let mut temp_buf = [0u8; CommandCancelCommand::MAX_SIZE];
                        let len = cancel_cmd
                            .write_into(camera_id, &mut temp_buf)
                            .map_err(|e| {
                                error!("Failed to encode cancel command: {e}");
                                e
                            })?;

                        let kind = CommandKind::Command;
                        config
                            .envelope
                            .frame_into(&temp_buf[..len], kind, &mut send_buf);

                        if let Err(e) = transport.send(&send_buf[..]).await {
                            handle_send_failure!(
                                transport,
                                adapter,
                                e,
                                "Failed to send cancel for socket"
                            );
                        } else {
                            debug!(
                                "Sent cancel for socket {socket:?} with camera_id {camera_id:?}"
                            );
                        }
                    }
                    TxItem::CancelById { camera_id: _, id } => {
                        // Route cancel through the scheduler for lifecycle-aware handling.
                        // The scheduler returns Some((camera_id, socket)) if the command has
                        // a socket already assigned (send immediately). Otherwise, the cancel
                        // is either deferred until ACK or a no-op for inactive commands.
                        if let Some((camera_id, socket)) = adapter.request_cancel_by_id(id) {
                            let cancel_cmd = CommandCancelCommand::new(socket);
                            let mut temp_buf = [0u8; CommandCancelCommand::MAX_SIZE];
                            let len =
                                cancel_cmd
                                    .write_into(camera_id, &mut temp_buf)
                                    .map_err(|e| {
                                        error!("Failed to encode cancel command: {e}");
                                        e
                                    })?;

                            let kind = CommandKind::Command;
                            config
                                .envelope
                                .frame_into(&temp_buf[..len], kind, &mut send_buf);

                            if let Err(e) = transport.send(&send_buf[..]).await {
                                handle_send_failure!(
                                    transport,
                                    adapter,
                                    e,
                                    "Failed to send cancel for command"
                                );
                            } else {
                                debug!("Sent cancel for command {id} on socket {socket:?} with camera_id {camera_id:?}");
                            }
                        }
                        // If request_cancel_by_id returns None, either:
                        // - The command is inactive (completed/timed out/unknown): no-op
                        // - The command is awaiting ACK: flagged for cancel-on-ACK
                    }
                }
                // Fall through to housekeeping
            }

            LoopEvent::Metrics(response_tx) => {
                let summary = adapter.metrics_summary();
                let _ = response_tx.send(summary);
                // Fall through to housekeeping
            }

            LoopEvent::SubscribeCompletions(response_tx) => {
                let completion_rx = adapter.subscribe_completions();
                let _ = response_tx.send(completion_rx);
                // Fall through to housekeeping
            }

            LoopEvent::TransportRecv(n) => {
                if n == 0 {
                    error!("Connection closed by peer; exiting runtime loop");
                    return Err(Error::ConnectionClosed {
                        reason: Some(std::borrow::Cow::Borrowed("peer closed connection")),
                    });
                }

                trace!("Received {n} bytes from transport");

                // Wire-level RX logging for diagnostics
                trace!(
                    target: "grafton_visca::wire",
                    len = n,
                    hex = %crate::runtime::driver::hex_bytes(&read_buf[..n]),
                    "RX",
                );

                // Use push_slice_with_resync to handle buffer overflow gracefully.
                // This clears the buffer and retries if overflow occurs, preventing
                // permanent runtime stalls from un-framable data accumulation.
                match protocol_framer.push_slice_with_resync(&read_buf[..n]) {
                    Ok(normal_push) => {
                        if !normal_push {
                            debug!("Framer resynced after buffer overflow");
                        }
                    }
                    Err(e) => {
                        // Resync failed - chunk alone exceeds max_buffer_size.
                        // This indicates a configuration issue but we continue
                        // to allow housekeeping and future data processing.
                        warn!("Framer resync failed (chunk exceeds max_buffer_size): {e}");
                    }
                }

                // Always drain frames after push attempt (even after resync)
                for frame_result in protocol_framer.drain_frames() {
                    let frame = match frame_result {
                        Ok(frame) => frame,
                        Err(e) => {
                            warn!("Failed to extract frame: {e}");
                            continue;
                        }
                    };
                    let (payload, meta) = match config.envelope.extract_with_meta(frame) {
                        Ok(result) => result,
                        Err(e) => {
                            warn!("Failed to extract response from frame: {e}");
                            continue;
                        }
                    };

                    if let Err(e) = adapter.process_response(&payload, meta.sequence).await {
                        error!("Error processing response: {e}");
                    }
                }

                // Try to send more items after processing responses
                while let Some(cmd) = adapter.next_item_to_send() {
                    if let Err(e) = send_one(
                        &mut transport,
                        &executor,
                        &mut adapter,
                        cmd,
                        &config.envelope,
                        &mut send_buf,
                        config.write_timeout,
                    )
                    .await
                    {
                        handle_send_failure!(
                            transport,
                            adapter,
                            e,
                            "Send failed while draining pending"
                        );
                    }
                }

                // Drain cancel outbox: send cancels that were queued via cancel-on-ACK
                // (when a cancel was requested before the socket was assigned, and an
                // ACK subsequently assigned the socket)
                for (camera_id, socket) in adapter.drain_cancel_outbox() {
                    let cancel_cmd = CommandCancelCommand::new(socket);
                    let mut temp_buf = [0u8; CommandCancelCommand::MAX_SIZE];
                    let len = cancel_cmd
                        .write_into(camera_id, &mut temp_buf)
                        .map_err(|e| {
                            error!("Failed to encode cancel-on-ACK command: {e}");
                            e
                        })?;

                    let kind = CommandKind::Command;
                    config
                        .envelope
                        .frame_into(&temp_buf[..len], kind, &mut send_buf);

                    if let Err(e) = transport.send(&send_buf[..]).await {
                        handle_send_failure!(
                            transport,
                            adapter,
                            e,
                            "Failed to send cancel-on-ACK for socket"
                        );
                    } else {
                        debug!(
                            "Sent cancel-on-ACK for socket {socket:?} with camera_id {camera_id:?}"
                        );
                    }
                }
                // Fall through to housekeeping
            }

            LoopEvent::TransportErr(e) => {
                if matches!(e, Error::ConnectionClosed { .. }) {
                    error!("Connection closed by peer; exiting runtime loop");
                    return Err(e);
                }

                error!("Error receiving from transport: {e}");
                adapter.on_network_error(e).await?;

                // Avoid hot-looping on immediate errors
                executor.sleep(std::time::Duration::from_millis(10)).await;
                // Fall through to housekeeping
            }

            LoopEvent::Tick => {
                runtime_trace!("Deadline tick with no data");
                // Fall through to housekeeping
            }
        }

        // ============================================================
        // HOUSEKEEPING: Always runs after every event (no early returns)
        // This ensures timeout/retry logic is never bypassed.
        // ============================================================

        runtime_trace!(
            "Before check_timeouts: pending_ack={}",
            adapter.pending_ack_count()
        );
        adapter.check_timeouts().await?;
        runtime_trace!(
            "After check_timeouts: pending_ack={}",
            adapter.pending_ack_count()
        );

        // Process any newly ready retries
        let ready_retries = adapter.get_ready_retries();
        for retry in ready_retries {
            debug!(
                "Processing retry for command {} (attempt {})",
                retry.id, retry.attempt
            );

            // Category and kind are derived from EncodedCommand
            let pending_cmd = PendingCommand {
                id: retry.id,
                command: retry.command,
                priority: retry.priority,
                camera_id: retry.camera_id,
                submitted_at: executor.now(),
            };

            if let Err(e) = send_one(
                &mut transport,
                &executor,
                &mut adapter,
                pending_cmd,
                &config.envelope,
                &mut send_buf,
                config.write_timeout,
            )
            .await
            {
                handle_send_failure!(transport, adapter, e, "Send failed during retry");
            }
        }

        // Try to send any queued items
        while let Some(cmd) = adapter.next_item_to_send() {
            if let Err(e) = send_one(
                &mut transport,
                &executor,
                &mut adapter,
                cmd,
                &config.envelope,
                &mut send_buf,
                config.write_timeout,
            )
            .await
            {
                handle_send_failure!(
                    transport,
                    adapter,
                    e,
                    "Send failed while draining pending after tick"
                );
            }
        }
    }
}
