//! Transmission handling for VISCA commands.

use tracing::{debug, error, instrument, trace, warn};

use std::sync::atomic::Ordering;

use super::scheduler::{Scheduler, SchedulerMetrics, TxItem};
use crate::{
    camera_id::CameraId,
    command::{encode_visca::ViscaEncode, system::CommandCancelCommand},
    error::{Error, Result},
    transport::{buffer::BufferManager, envelope::TransportEnvelope, AsyncTransport},
    ViscaSocket,
};

/// Handle a submitted TX item.
#[instrument(level = "trace", skip(transport, scheduler, executor, envelope, buffer_manager), fields(item = ?item))]
pub async fn handle_tx_item<T: AsyncTransport + Send, E: crate::executor::Executor>(
    transport: &mut T,
    scheduler: &mut Scheduler,
    item: TxItem,
    executor: &E,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
) -> Result<()> {
    match item {
        TxItem::Command {
            mut id,
            bytes,
            priority,
            category,
            camera_id,
            response_tx,
        } => {
            // Assign ID if not set
            if id == 0 {
                id = scheduler.next_id();
            }

            trace!("Processing command {id} with priority {priority:?}");

            // Track metrics for command submission
            scheduler
                .metrics
                .commands_submitted
                .fetch_add(1, Ordering::Relaxed);
            let idx = SchedulerMetrics::priority_index(priority);
            scheduler.metrics.priority_counts[idx].fetch_add(1, Ordering::Relaxed);

            // Check if we can send immediately (not at 2-command limit)
            // NOTE: We don't allocate socket yet - camera assigns it in ACK
            let now = executor.now();
            if scheduler.can_send_command() {
                // Enforce command spacing
                scheduler.enforce_spacing_with(executor, now).await;

                // Frame and send command
                let framed_bytes = envelope.frame_command(&bytes, false, buffer_manager);
                debug!(
                    "Sending command {id} (awaiting ACK): {bytes:02X?} (framed: {framed_bytes:02X?})"
                );
                trace!("Sending command {id} (awaiting ACK): {bytes:02X?}");
                if let Err(e) = transport.send(&framed_bytes).await {
                    error!("Failed to send command {id}: {e}");
                    scheduler
                        .metrics
                        .commands_failed
                        .fetch_add(1, Ordering::Relaxed);
                    let _ = response_tx.send(Err(Error::TransportError(e.to_string().into())));
                    return Ok(());
                }

                // Add to pending ACK list - socket will be assigned when ACK arrives
                scheduler.add_pending_ack(id, bytes.clone(), priority, category, now, camera_id);
                scheduler.store_command_channel(id, response_tx);
                debug!("Command {id} added to pending ACK list");
            } else {
                // No socket available, add to priority queue
                debug!(
                    "No socket available for command {id}, adding to queue with priority {priority:?}"
                );

                // Store the response channel for when the command is eventually sent
                scheduler.store_command_channel(id, response_tx.clone());

                // Enqueue the command for later processing
                let now = executor.now();
                scheduler.enqueue_command(
                    TxItem::Command {
                        id,
                        bytes,
                        priority,
                        category,
                        camera_id,
                        response_tx,
                    },
                    now,
                );
            }
        }

        TxItem::Inquiry {
            mut id,
            bytes,
            camera_id: _,
            response_type,
            response_tx,
        } => {
            // Assign ID if not set
            if id == 0 {
                id = scheduler.next_id();
            }

            trace!("Processing inquiry {id}");

            // Track metrics for inquiry submission
            scheduler
                .metrics
                .inquiries_submitted
                .fetch_add(1, Ordering::Relaxed);

            // Inquiries don't need sockets
            let now = executor.now();
            scheduler.enforce_spacing_with(executor, now).await;

            // Frame and send inquiry
            let framed_bytes = envelope.frame_command(&bytes, true, buffer_manager);
            trace!("Sending inquiry {id}: {bytes:02X?} (framed: {framed_bytes:02X?})");
            if let Err(e) = transport.send(&framed_bytes).await {
                error!("Failed to send inquiry {id}: {e}");
                scheduler
                    .metrics
                    .commands_failed
                    .fetch_add(1, Ordering::Relaxed);
                let _ = response_tx.send(Err(Error::TransportError(e.to_string().into())));
                return Ok(());
            }

            // Store the response channel in the scheduler for later use
            scheduler.store_pending_inquiry(id, response_tx, response_type);
        }

        TxItem::Cancel { socket } => {
            trace!("Processing cancel for {socket:?}");

            // Send cancel command using typed command
            let cancel_cmd = CommandCancelCommand::new(socket);
            let mut cancel_bytes = [0u8; 16];
            // CommandCancelCommand is const-constructed and guaranteed to encode
            let len = cancel_cmd
                .encode_into(CameraId::CAMERA_1, &mut cancel_bytes)
                .map_err(|e| {
                    Error::TransportError(format!("Failed to encode cancel command: {e}").into())
                })?;
            let cancel_bytes = bytes::Bytes::copy_from_slice(&cancel_bytes[..len]);

            // Frame the cancel command as a regular command (not an inquiry)
            let framed_cancel = envelope.frame_command(&cancel_bytes, false, buffer_manager);
            if let Err(e) = transport.send(&framed_cancel).await {
                error!("Failed to send cancel: {e}");
                return Ok(());
            }

            // Free the socket and notify
            if let Some(_cmd_id) = scheduler.socket_command(socket) {
                scheduler.free_socket(socket);
            }
        }

        TxItem::CancelById { id } => {
            trace!("Processing cancel by ID for command {id}");

            // Check if command is in pending_ack - if so, just remove it without sending cancel
            if scheduler.is_pending_ack(id) {
                // Command hasn't been ACK'd yet - no socket assigned, no cancel to send
                scheduler.remove_pending_ack(id);
                if let Some(response_tx) = scheduler.get_response_channel(id) {
                    let _ = response_tx.send(Err(Error::CommandCanceled));
                }
                scheduler.remove_command_metadata(id);
                debug!("Canceled command {id} that was pending ACK");
                return Ok(());
            }

            // Check if command has been assigned a socket
            let mut found_socket = None;
            let mut found_camera_id = CameraId::CAMERA_1; // Default fallback

            for socket in [ViscaSocket::S1, ViscaSocket::S2] {
                if scheduler.socket_command(socket) == Some(id) {
                    found_socket = Some(socket);
                    // Get camera ID from metadata
                    if let Some((_, _, _, camera_id)) = scheduler.get_command_metadata(id) {
                        found_camera_id = *camera_id;
                    }
                    break;
                }
            }

            if let Some(socket) = found_socket {
                // Build cancel command with the correct camera ID
                let cancel_cmd = CommandCancelCommand::new(socket);
                let mut cancel_bytes = [0u8; 16];
                let len = cancel_cmd
                    .encode_into(found_camera_id, &mut cancel_bytes)
                    .map_err(|e| {
                        Error::TransportError(
                            format!("Failed to encode cancel command: {e}").into(),
                        )
                    })?;
                let cancel_bytes = bytes::Bytes::copy_from_slice(&cancel_bytes[..len]);

                // Frame the cancel command as a regular command (not an inquiry)
                let framed_cancel = envelope.frame_command(&cancel_bytes, false, buffer_manager);
                if let Err(e) = transport.send(&framed_cancel).await {
                    error!("Failed to send cancel: {e}");
                    return Ok(());
                }

                // Free the socket and notify
                scheduler.free_socket(socket);
                if let Some(response_tx) = scheduler.get_response_channel(id) {
                    let _ = response_tx.send(Err(Error::CommandCanceled));
                }
                scheduler.remove_command_metadata(id);
                debug!("Canceled command {id} on socket {socket:?}");
            } else {
                // Command not found or already completed
                warn!("Command {id} not found or already completed");
                if let Some(response_tx) = scheduler.get_response_channel(id) {
                    let _ = response_tx.send(Err(Error::CommandCanceled));
                }
            }
        }
    }

    Ok(())
}
