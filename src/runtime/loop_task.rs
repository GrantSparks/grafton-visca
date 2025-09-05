//! Main runtime event loop for VISCA communication.

use flume::{Receiver, Sender};
use tracing::{debug, error, instrument, trace, warn};

use std::sync::Arc;

use super::{
    queue::process_command_queue,
    rx::handle_response,
    scheduler::{MetricsSummary, Scheduler, TxItem},
    tx::handle_tx_item,
};
use crate::{
    error::{Error, Result},
    protocol::framer::ProtocolFramer,
    timeout::TimeoutConfig,
    transport::{buffer::BufferManager, envelope::TransportEnvelope, AsyncTransport},
};

/// Configuration for the runtime loop.
pub struct RuntimeLoopConfig {
    pub tick_interval_ms: Option<u64>,
    pub envelope: TransportEnvelope,
    pub buffer_manager: BufferManager,
    pub timeout_config: TimeoutConfig,
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
    let mut scheduler = Scheduler::with_timeout_config(submit_rx.clone(), config.timeout_config);
    let mut protocol_framer = ProtocolFramer::new(4096); // Default buffer size
    let mut consecutive_retries = 0usize;

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
            match handle_tx_item(
                &mut transport,
                &mut scheduler,
                item,
                executor.as_ref(),
                &config.envelope,
                &config.buffer_manager,
            )
            .await
            {
                Ok(_) => {
                    // Successfully handled a TX item - reset consecutive retries
                    consecutive_retries = 0;

                    // Process command queue with retry budget check
                    let allow_retry_defer = consecutive_retries < 8;
                    if let Err(e) = process_command_queue(
                        &mut transport,
                        &mut scheduler,
                        executor.as_ref(),
                        allow_retry_defer,
                        &config.envelope,
                        &config.buffer_manager,
                    )
                    .await
                    {
                        error!("Error processing command queue after TX item: {e}");
                    }
                }
                Err(e) => {
                    error!("Error handling TX item: {e}");
                }
            }
            continue;
        }

        // Check for metrics requests non-blockingly
        if let Ok(response_tx) = metrics_rx.try_recv() {
            let summary = scheduler.metrics.summary();
            let _ = response_tx.send(summary);
            continue;
        }

        // Pre-drain any retries that are due now to avoid race conditions
        // This ensures deterministic behavior when retry deadline == now
        let now = executor.as_ref().now();
        if scheduler.can_send_command() {
            while let Some(deadline) = scheduler.next_retry_deadline() {
                if deadline > now {
                    break; // No more retries due now
                }

                // Get the next retry that's due now
                if let Some(retry_cmd) = scheduler.get_next_retry(now) {
                    debug!(
                        "Pre-draining retry for command {id} (attempt {attempt})",
                        id = retry_cmd.id,
                        attempt = retry_cmd.attempt
                    );

                    // Re-submit the command for retry
                    let response_tx = if let Some(tx) =
                        scheduler.peek_response_channel(retry_cmd.id)
                    {
                        tx.clone()
                    } else {
                        warn!(
                            "Missing response channel for retry of command {id} - this indicates a bug",
                            id = retry_cmd.id
                        );
                        let (tx, _rx) = flume::bounded(1);
                        tx
                    };

                    // Create TxItem for the retry
                    let tx_item = TxItem::Command {
                        id: retry_cmd.id,
                        bytes: retry_cmd.bytes.clone(),
                        priority: retry_cmd.priority,
                        category: retry_cmd.category,
                        camera_id: retry_cmd.camera_id,
                        response_tx,
                    };

                    // Send the retry immediately
                    if let Err(e) = handle_tx_item(
                        &mut transport,
                        &mut scheduler,
                        tx_item,
                        executor.as_ref(),
                        &config.envelope,
                        &config.buffer_manager,
                    )
                    .await
                    {
                        error!("Error handling retry TX item: {e}");
                    }

                    // If can't send more commands, stop draining
                    if !scheduler.can_send_command() {
                        break;
                    }
                } else {
                    break; // No retry ready (shouldn't happen since we checked deadline)
                }
            }
        }

        // Dynamic tick scheduling: sleep until the earliest retry deadline or housekeeping tick
        let now = executor.as_ref().now();
        let sleep_dur = if let Some(deadline) = scheduler.next_retry_deadline() {
            // Wake exactly when a retry becomes eligible (but never later than the housekeeping tick)
            let until_retry = deadline.saturating_duration_since(now);
            std::cmp::min(until_retry, tick_duration)
        } else {
            tick_duration
        };

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

                            // For Sony encapsulated protocols, validate sequence
                            if let Some(seq) = meta.sequence {
                                // Check if this sequence is known (not a duplicate/late frame)
                                if !scheduler.observe_sequence(seq) {
                                    trace!("Dropping duplicate/late frame with sequence {seq}");
                                    continue;
                                }
                            }

                            if let Err(e) = handle_response(
                                &mut transport,
                                &mut scheduler,
                                &payload,
                                executor.as_ref(),
                                &config.envelope,
                                &config.buffer_manager,
                            )
                            .await
                            {
                                error!("Error handling response: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving from transport: {e}");

                        // When transport recv fails, we should fail any pending commands
                        // as they won't receive responses. This prevents infinite waiting.

                        // Get all pending ACK commands and fail them
                        let pending_acks = scheduler.get_all_pending_ack_commands();
                        for cmd_id in pending_acks {
                            debug!("Failing command {cmd_id} due to transport recv error");
                            if let Some(tx) = scheduler.get_response_channel(cmd_id) {
                                let _ = tx.send(Err(Error::TransportError(
                                    format!("Transport recv failed: {}", e).into(),
                                )));
                            }
                            // Remove from pending to prevent repeated failures
                            scheduler.remove_pending_ack(cmd_id);
                        }

                        // Also fail any commands in sockets (waiting for completion)
                        let sockets = [crate::ViscaSocket::S1, crate::ViscaSocket::S2];
                        for socket in sockets {
                            if let Some(cmd_id) = scheduler.get_socket_command(socket) {
                                debug!("Failing command {cmd_id} in socket {socket:?} due to transport recv error");
                                if let Some(tx) = scheduler.get_response_channel(cmd_id) {
                                    let _ = tx.send(Err(Error::TransportError(
                                        format!("Transport recv failed: {}", e).into(),
                                    )));
                                }
                                scheduler.free_socket(socket);
                            }
                        }
                    }
                }
            }
            Operation::Tick => {
                // Handle tick - check for timeouts and retries
                let now = executor.as_ref().now();

                // Check for commands that have timed out waiting for ACK
                let pending_ack_timeouts = scheduler.check_pending_ack_timeouts(now);
                for cmd_id in pending_ack_timeouts {
                    debug!("Command {cmd_id} timed out waiting for ACK");
                    // Notify the waiting high-level caller
                    if let Some(tx) = scheduler.get_response_channel(cmd_id) {
                        let _ = tx.send(Err(Error::Timeout));
                    }
                }

                // Check for commands in sockets that have timed out
                let timed_out = scheduler.check_timeouts(now);
                for (socket, cmd_id) in timed_out {
                    // First, notify the waiting high-level caller
                    if let Some(tx) = scheduler.get_response_channel(cmd_id) {
                        let _ = tx.send(Err(Error::Timeout));
                    }

                    // Finally, free the socket & clean up metadata
                    scheduler.free_socket(socket);
                }

                // Then check if we have retries to process
                if scheduler.has_retries() {
                    debug!(
                        "Has {retry_count} retries pending, free socket: {has_free}, can_send: {can_send}, retry queue: {retry_queue:?}",
                        retry_count = scheduler.retry_queue.len(),
                        has_free = scheduler.has_free_socket(),
                        can_send = scheduler.can_send_command(),
                        retry_queue = scheduler
                            .retry_queue
                            .iter()
                            .map(|r| r.id)
                            .collect::<Vec<_>>()
                    );
                }
                if scheduler.can_send_command() && scheduler.has_retries() {
                    debug!("Has free socket and retries to process");
                    // get_next_retry() now handles exhausted retries internally
                    let now = executor.as_ref().now();
                    if let Some(retry_cmd) = scheduler.get_next_retry(now) {
                        debug!(
                            "Retrying command {id} (attempt {attempt})",
                            id = retry_cmd.id,
                            attempt = retry_cmd.attempt
                        );

                        // Re-submit the command for retry
                        let response_tx = if let Some(tx) =
                            scheduler.peek_response_channel(retry_cmd.id)
                        {
                            tx.clone()
                        } else {
                            warn!(
                                "Missing response channel for retry of command {id} - this indicates a bug",
                                id = retry_cmd.id
                            );
                            continue;
                        };
                        debug!(
                            "Using existing response channel for retry of command {id}",
                            id = retry_cmd.id
                        );

                        let item = TxItem::Command {
                            id: retry_cmd.id,
                            bytes: retry_cmd.bytes.clone(),
                            priority: retry_cmd.priority,
                            category: retry_cmd.category,
                            camera_id: retry_cmd.camera_id,
                            response_tx,
                        };

                        if let Err(e) = handle_tx_item(
                            &mut transport,
                            &mut scheduler,
                            item,
                            executor.as_ref(),
                            &config.envelope,
                            &config.buffer_manager,
                        )
                        .await
                        {
                            error!("Error retrying command {id}: {e}", id = retry_cmd.id);
                        } else {
                            // Successfully submitted a retry
                            consecutive_retries = consecutive_retries.saturating_add(1);
                        }
                    }
                }
            }
        }

        // Check if submit channel is closed and scheduler is idle for shutdown
        let disconnected = submit_rx.is_disconnected();
        let idle = scheduler.is_idle();
        if disconnected && idle {
            debug!("Submit channel closed and scheduler is idle, shutting down runtime");
            break;
        }
    }

    // The loop above never exits normally
    #[allow(unreachable_code)]
    {
        debug!("VISCA runtime stopped");
    }
    Ok(())
}
