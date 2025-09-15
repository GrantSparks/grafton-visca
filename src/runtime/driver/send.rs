//! Shared send pipeline implementation for runtime-neutral VISCA communication.
//!
//! This module provides the unified send logic that works across both
//! async and blocking modes, eliminating code duplication.

use tracing::{debug, error};

use crate::{
    command::CommandKind,
    runtime::core::PendingCommand,
    transport::{buffer::BufferManager, envelope::TransportEnvelope},
    visca_socket::ViscaSocket,
    Result,
};

use super::SchedulerLike;

/// RAII guard for automatic rollback of send operations on failure.
///
/// This guard ensures that if a send operation fails, any reserved resources
/// (sockets, pending ACK registrations) are automatically rolled back.
#[derive(Debug)]
#[allow(missing_copy_implementations)] // Can't copy due to ViscaSocket
pub struct SendGuard {
    id: u32,
    reserved_socket: Option<ViscaSocket>,
    ack_registered: bool,
    committed: bool,
}

impl SendGuard {
    /// Create a new send guard for the given command ID.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            reserved_socket: None,
            ack_registered: false,
            committed: false,
        }
    }

    /// Mark the send as committed (successful).
    pub fn commit(&mut self) {
        self.committed = true;
    }

    /// Rollback the send operation on failure.
    ///
    /// This method automatically unregisters pending ACKs, frees reserved sockets,
    /// and schedules retries according to the retry policy.
    pub fn rollback<S: SchedulerLike>(self, scheduler: &mut S) {
        if !self.committed {
            // Rollback on failure
            if let Some(socket) = self.reserved_socket {
                debug!(
                    "SendGuard: Rolling back inquiry {} socket reservation",
                    self.id
                );
                scheduler.free_socket(socket);
            }
            if self.ack_registered {
                debug!(
                    "SendGuard: Rolling back command {} ACK registration",
                    self.id
                );
                scheduler.unregister_pending_ack(self.id);
            }
            // Fail immediately for send failure (no retry per documented semantics)
            debug!(
                "SendGuard: Failing command {} after send failure (no retry)",
                self.id
            );
            scheduler.fail_after_send_error(self.id);
        }
    }
}

/// Async version of send_one.
#[cfg(feature = "async")]
pub(crate) async fn send_one<T, E, S>(
    transport: &mut T,
    executor: &E,
    scheduler: &mut S,
    cmd: PendingCommand,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
    write_timeout: core::time::Duration,
) -> Result<()>
where
    T: crate::transport::AsyncTransport,
    E: crate::executor::Executor,
    S: SchedulerLike,
{
    // Frame the command using single-allocation path with direct encoding
    let (framed, meta) = envelope
        .frame_encodable_command(&cmd.command, cmd.camera_id, buffer_manager)
        .map_err(|e| {
            error!("Failed to frame command {}: {:?}", cmd.id, e);
            e
        })?;

    // Create guard for tracking rollback state
    let mut guard = SendGuard::new(cmd.id);

    // For inquiries, start tracking without socket allocation
    // For commands, register as pending ACK
    if cmd.kind == CommandKind::Inquiry {
        scheduler.start_inquiry(&cmd);
        debug!("Started tracking inquiry {}", cmd.id);
    } else {
        scheduler.register_pending_ack(&cmd);
        guard.ack_registered = true;
        debug!("Registered pending ACK for command {}", cmd.id);
    }

    // Try to send the command with timeout using race
    let send_result = {
        use futures_lite::future;

        future::race(async { transport.send(&framed).await.map(|_| ()) }, async {
            executor.sleep(write_timeout).await;
            Err(crate::Error::Timeout)
        })
        .await
    };

    match send_result {
        Ok(()) => {
            debug!(
                "Successfully sent {} {}",
                if cmd.kind == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                cmd.id
            );

            // Register Sony sequence if present
            if let Some(seq) = meta.sequence {
                scheduler.register_sequence(cmd.id, seq);
                debug!("Registered Sony sequence {} for command {}", seq, cmd.id);
            }

            // Mark as committed to prevent rollback
            guard.commit();
            Ok(())
        }
        Err(e) => {
            let error_type = if matches!(e, crate::Error::Timeout) {
                "timeout"
            } else {
                "failed"
            };

            error!(
                "Send {} for {} {}: {:?}",
                error_type,
                if cmd.kind == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                cmd.id,
                e
            );

            // Rollback will happen automatically when guard is dropped
            guard.rollback(scheduler);

            // Return error to caller
            Err(e)
        }
    }
}

/// Blocking version of send_one for non-async builds.
#[cfg(not(feature = "async"))]
pub(crate) fn send_one<T, S>(
    transport: &mut T,
    scheduler: &mut S,
    cmd: PendingCommand,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
    _write_timeout: core::time::Duration,
) -> Result<()>
where
    T: crate::transport::SyncTransport,
    S: SchedulerLike,
{
    // Frame the command using single-allocation path with direct encoding
    let (framed, meta) = envelope
        .frame_encodable_command(&cmd.command, cmd.camera_id, buffer_manager)
        .map_err(|e| {
            error!("Failed to frame command {}: {:?}", cmd.id, e);
            e
        })?;

    // Create guard for tracking rollback state
    let mut guard = SendGuard::new(cmd.id);

    // For inquiries, start tracking without socket allocation
    // For commands, register as pending ACK
    if cmd.kind == CommandKind::Inquiry {
        scheduler.start_inquiry(&cmd);
        debug!("Started tracking inquiry {}", cmd.id);
    } else {
        scheduler.register_pending_ack(&cmd);
        guard.ack_registered = true;
        debug!("Registered pending ACK for command {}", cmd.id);
    }

    // Try to send the command with timeout
    // In blocking mode, we use the transport directly
    let send_result = transport.send_with_kind(&framed, cmd.kind);

    match send_result {
        Ok(()) => {
            debug!(
                "Successfully sent {} {}",
                if cmd.kind == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                cmd.id
            );

            // Register Sony sequence if present
            if let Some(seq) = meta.sequence {
                scheduler.register_sequence(cmd.id, seq);
                debug!("Registered Sony sequence {} for command {}", seq, cmd.id);
            }

            // Mark as committed to prevent rollback
            guard.commit();
            Ok(())
        }
        Err(e) => {
            let error_type = if matches!(e, crate::Error::Timeout) {
                "timeout"
            } else {
                "failed"
            };

            error!(
                "Send {} for {} {}: {:?}",
                error_type,
                if cmd.kind == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                cmd.id,
                e
            );

            // Rollback will happen automatically when guard is dropped
            guard.rollback(scheduler);

            // Return error to caller
            Err(e)
        }
    }
}
