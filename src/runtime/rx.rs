//! Response handling for VISCA communication.

use tracing::{debug, error, instrument, trace, warn};

use std::sync::atomic::Ordering;

use super::{queue::process_command_queue, scheduler::Scheduler};
use crate::{
    command::response::{lift_inquiry, ViscaResponse},
    error::{Error, Result},
    protocol::response::{decode_basic, BasicKind},
    runtime::scheduler::ViscaError,
    transport::{buffer::BufferManager, envelope::TransportEnvelope, AsyncTransport},
};

/// Handle a received VISCA response.
#[instrument(
    level = "trace",
    skip(transport, scheduler, frame, executor, envelope, buffer_manager)
)]
pub async fn handle_response<T: AsyncTransport + Send, E: crate::executor::Executor>(
    transport: &mut T,
    scheduler: &mut Scheduler,
    frame: &[u8],
    executor: &E,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
) -> Result<()> {
    let basic_response = match decode_basic(frame) {
        Some(resp) => resp,
        None => {
            warn!("Failed to decode VISCA frame: {frame:02X?}");
            return Ok(());
        }
    };
    debug!("Parsed response: {basic_response:?}");
    trace!("Parsed response: {basic_response:?}");

    match basic_response.kind {
        BasicKind::Ack => {
            let Some(socket) = basic_response.socket else {
                warn!("Received ACK without socket information");
                return Ok(());
            };
            // Camera has assigned a socket - handle the ACK
            let now = executor.now();
            if let Some(cmd_id) = scheduler.handle_ack(socket, now) {
                debug!("ACK received - command {cmd_id} assigned to {socket:?} by camera");

                // Don't send ACK to the response channel - wait for Completion
                // The response channel is expecting the final result, not intermediate ACKs
            } else {
                warn!("Received ACK for {socket:?} but no pending commands awaiting ACK");
            }
        }

        BasicKind::Completion => {
            let Some(socket) = basic_response.socket else {
                warn!("Received Completion without socket information");
                return Ok(());
            };
            if let Some(cmd_id) = scheduler.socket_command(socket) {
                debug!("Completion received for command {cmd_id} on {socket:?}");

                // Track successful completion
                scheduler
                    .metrics
                    .commands_completed
                    .fetch_add(1, Ordering::Relaxed);

                // Remove from retry queue if it was being retried
                scheduler.remove_from_retry_queue(cmd_id);

                // Notify the waiting command and free the socket
                if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                    debug!("Sending completion to response channel for command {cmd_id}");
                    if let Err(e) = response_tx.send(Ok(ViscaResponse::Completion {
                        socket: Some(socket),
                    })) {
                        warn!("Failed to send completion to response channel: {e:?}");
                    }
                } else {
                    warn!("No response channel found for command {cmd_id} completion");
                }
                scheduler.free_socket(socket);

                // Process any queued commands now that a socket is free
                if let Err(e) = process_command_queue(
                    transport,
                    scheduler,
                    executor,
                    true,
                    envelope,
                    buffer_manager,
                )
                .await
                {
                    error!("Error processing command queue after completion: {e}");
                }
            } else {
                warn!("Received completion for {socket:?} with no pending command");
            }
        }

        BasicKind::DataReply => {
            let data = basic_response.payload;
            debug!("Data reply received: {data:02X?}");

            // For inquiries, we need to match this with the pending inquiry
            // Since inquiries don't use sockets, we need a different mechanism
            // For now, assume the most recent inquiry is the one being responded to
            if let Some((_inquiry_id, response_tx, response_type)) = scheduler.get_pending_inquiry()
            {
                // Use the unified lift_inquiry function to parse the response
                let response = match lift_inquiry(&basic_response, response_type.as_ref()) {
                    Ok(parsed) => Ok(parsed),
                    Err(e) => {
                        warn!("Failed to parse inquiry response: {e}");
                        Ok(ViscaResponse::Unknown {
                            response_type,
                            data: data.to_vec(),
                        })
                    }
                };

                let _ = response_tx.send(response);
            } else {
                warn!("Received data reply with no pending inquiry");
            }
        }

        BasicKind::Error(error_code) => {
            let socket = basic_response.socket;
            let error = ViscaError::from_byte(error_code);
            warn!("Error response: {error:?} on socket {socket:?}");

            // First check if this is an error for a pending inquiry
            // Inquiries don't have sockets, so if there's no socket or no command on the socket,
            // and we have a pending inquiry, this error is for the inquiry
            let is_inquiry_error = socket.map_or(true, |s| scheduler.socket_command(s).is_none());

            if is_inquiry_error && scheduler.has_pending_inquiry() {
                // This error is for a pending inquiry
                if let Some((inquiry_id, response_tx, _response_type)) =
                    scheduler.get_pending_inquiry()
                {
                    debug!("Error {error:?} for inquiry {inquiry_id}");
                    let error_code = error.as_byte();
                    let _ = response_tx.send(Err(Error::from_code(error_code)));
                    scheduler
                        .metrics
                        .commands_failed
                        .fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                }
            }

            // Handle errors for commands that haven't received ACK yet
            // Check if we have pending ACK commands and no command assigned to the socket yet
            let is_pending_ack_error = if let Some(sock) = socket {
                // If socket has error but no command assigned, it's a pending ACK error
                scheduler.socket_command(sock).is_none() && scheduler.pending_ack_count() > 0
            } else {
                // No socket specified - always check pending ACK
                scheduler.pending_ack_count() > 0
            };

            if is_pending_ack_error {
                if let Some((cmd_id, priority, category, bytes, camera_id)) =
                    scheduler.handle_pending_ack_error_with_bytes(error)
                {
                    debug!("{error:?} for pending ACK command {cmd_id}");

                    // Store metadata for potential retry (it wasn't stored since we never got ACK)
                    scheduler.store_command_metadata(
                        cmd_id,
                        bytes.clone(),
                        priority,
                        category,
                        camera_id,
                    );

                    // Queue for retry if it's a retryable error
                    let retryable = error.is_retryable(Some(category));

                    if retryable {
                        let now = executor.now();
                        let queued = scheduler
                            .queue_for_retry(cmd_id, bytes, priority, category, camera_id, now);
                        if !queued {
                            // Retries exhausted - error already sent to response channel by queue_for_retry
                            debug!("Command {cmd_id} exhausted retries");
                        } else {
                            // Successfully queued for retry
                            debug!("Command {cmd_id} queued for retry: {error:?}");
                        }
                    } else {
                        // Non-retryable error - send error response immediately
                        debug!("Non-retryable error {error:?} for command {cmd_id}");
                        if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                            // Convert ViscaError to Error using the byte code
                            let error_code = error.as_byte();
                            let _ = response_tx.send(Err(Error::from_code(error_code)));
                            scheduler
                                .metrics
                                .commands_failed
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    return Ok(());
                }
            }

            // Check if error is retryable based on error type and command context
            let should_retry = if let Some(sock) = socket {
                if let Some(cmd_id) = scheduler.socket_command(sock) {
                    // Get command category to determine if NotExecutable (0x41) is retryable
                    let category = scheduler
                        .get_command_for_retry(cmd_id)
                        .map(|(_, _, cat, _)| cat);
                    error.is_retryable(category)
                } else {
                    false
                }
            } else {
                false
            };

            if should_retry {
                if let Some(sock) = socket {
                    if let Some(cmd_id) = scheduler.socket_command(sock) {
                        debug!("Camera busy for command {cmd_id} on {sock:?}, will retry");

                        // Get command metadata for retry and queue it BEFORE freeing socket
                        if let Some((bytes, priority, category, camera_id)) =
                            scheduler.get_command_for_retry(cmd_id)
                        {
                            debug!(
                                "Queueing command {cmd_id} for retry with priority {priority:?}"
                            );
                            // Queue the command for retry (returns false if exhausted)
                            let now = executor.now();
                            let queued = scheduler
                                .queue_for_retry(cmd_id, bytes, priority, category, camera_id, now);

                            if !queued {
                                debug!("Command {cmd_id} exhausted retries, not queuing");
                                // The queue_for_retry method has already sent the error response
                            }
                        } else {
                            warn!("No metadata found for command {cmd_id} to retry");
                        }

                        // Free the socket so it can be reused
                        // The free_socket method will check if command is queued for retry
                        scheduler.free_socket(sock);

                        // Don't send error to response channel if queued for retry
                        // The command will be retried automatically when a socket becomes available
                        // The response channel remains stored in the scheduler

                        // Process any queued commands now that a socket is free
                        // Keep consecutive_retries count, as this wasn't a successful queue operation
                        if let Err(e) = process_command_queue(
                            transport,
                            scheduler,
                            executor,
                            true,
                            envelope,
                            buffer_manager,
                        )
                        .await
                        {
                            error!("Error processing command queue after busy: {e}");
                        }
                    }
                }
            } else {
                // Handle other errors normally

                // Track command failure
                scheduler
                    .metrics
                    .commands_failed
                    .fetch_add(1, Ordering::Relaxed);

                if let Some(sock) = socket {
                    if let Some(cmd_id) = scheduler.socket_command(sock) {
                        // Remove from retry queue if it was being retried
                        scheduler.remove_from_retry_queue(cmd_id);

                        // Notify the waiting command and free the socket
                        if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                            // Convert ViscaError to Error using the byte code
                            let error_code = error.as_byte();
                            let _ = response_tx.send(Err(Error::from_code(error_code)));
                        }
                        scheduler.free_socket(sock);

                        // Process any queued commands now that a socket is free
                        if let Err(e) = process_command_queue(
                            transport,
                            scheduler,
                            executor,
                            true,
                            envelope,
                            buffer_manager,
                        )
                        .await
                        {
                            error!("Error processing command queue after error: {e}");
                        }
                    }
                } else {
                    // Broadcast error with no specific socket
                    // Try to route to the most recently sent command
                    // (VISCA cameras typically send broadcast errors for the last command)

                    // Get the most recent command ID from any socket
                    let recent_cmd_id = scheduler.most_recent_command();

                    if let Some(cmd_id) = recent_cmd_id {
                        debug!("Routing broadcast error to command {cmd_id}");

                        // Notify the waiting command
                        if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                            let error_code = error.as_byte();
                            let _ = response_tx.send(Err(Error::from_code(error_code)));
                        }

                        // Free any socket that might be associated with this command
                        scheduler.free_command_socket(cmd_id);
                    } else {
                        // No pending commands, just broadcast the error
                    }
                }
            }
        }

        BasicKind::NetworkChange => {
            debug!("Network change notification received");
            // Could trigger a re-initialization or status check
        }

        BasicKind::Unknown => {
            warn!(
                "Unknown response received: {payload:02X?}",
                payload = basic_response.payload
            );
        }
    }

    Ok(())
}
