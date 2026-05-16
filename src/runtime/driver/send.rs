//! Shared send pipeline implementation for runtime-neutral VISCA communication.
//!
//! This module provides the unified send logic that works across both
//! async and blocking modes, eliminating code duplication.

use tracing::{debug, error, trace};

use super::{hex_bytes, SchedulerLike};
use crate::{
    camera::CommandId,
    command::CommandKind,
    runtime::core::{PendingCommand, SchedulerAction},
    transport::envelope::Envelope,
};

#[cfg(feature = "mode-async")]
use crate::Result;

/// Result of a blocking send operation.
///
/// This enum captures both the success/failure status and any scheduler action
/// that needs to be propagated to the caller.
#[cfg(not(feature = "mode-async"))]
#[derive(Debug)]
pub enum SendResult {
    /// Send succeeded.
    Ok,
    /// Send failed. Contains the original error and the scheduler action
    /// (typically `CommandFailed`) that should be checked by the caller
    /// for immediate error propagation.
    Err {
        /// The transport error that caused the failure.
        error: crate::Error,
        /// The scheduler action to handle (typically `CommandFailed`).
        action: Option<SchedulerAction>,
    },
}

/// RAII guard for automatic rollback of send operations on failure.
///
/// This guard tracks whether a send operation has been committed (successful).
/// If `rollback` is called and the send was not committed, the command is
/// immediately failed via the scheduler.
///
/// With the phase-based command lifecycle design, socket and ACK state cleanup
/// is implicit when the command is removed from the scheduler.
#[derive(Debug, Clone, Copy)]
pub struct SendGuard {
    id: CommandId,
    committed: bool,
}

impl SendGuard {
    /// Create a new send guard for the given command ID.
    pub fn new(id: CommandId) -> Self {
        Self {
            id,
            committed: false,
        }
    }

    /// Mark the send as committed (successful).
    pub fn commit(&mut self) {
        self.committed = true;
    }

    /// Rollback the send operation on failure.
    ///
    /// This method fails the command immediately with the provided error wrapped in context.
    /// With the phase-based design, all cleanup (socket state, pending ACK state) is handled
    /// implicitly when the command is removed from the scheduler during `fail_after_send_error`.
    ///
    /// The `cause` parameter preserves the original error (including timeout semantics)
    /// while the scheduler adds "Send failed" context.
    ///
    /// # Return Value
    ///
    /// - **Async mode**: Returns `None` because `AsyncAdapter::fail_after_send_error`
    ///   handles the action internally (updates metrics and notifies via response channel).
    ///   The caller can safely ignore the return value.
    ///
    /// - **Blocking mode**: Returns `Some(SchedulerAction::CommandFailed)` for the
    ///   caller to propagate the error to clients.
    pub fn rollback<S: SchedulerLike>(
        self,
        scheduler: &mut S,
        cause: crate::Error,
    ) -> Option<SchedulerAction> {
        if !self.committed {
            // Fail immediately for send failure (no retry per documented semantics).
            // Socket and ACK state cleanup is implicit in phase-based design when
            // the command is removed during fail_after_send_error.
            debug!(
                "SendGuard: Failing command {id} after send failure (no retry)",
                id = self.id
            );
            scheduler.fail_after_send_error(self.id, cause)
        } else {
            None
        }
    }
}

/// Async version of send_one.
#[cfg(feature = "mode-async")]
pub(crate) async fn send_one<T, Ex, S, Env>(
    transport: &mut T,
    executor: &Ex,
    scheduler: &mut S,
    cmd: PendingCommand,
    envelope: &Env,
    send_buf: &mut bytes::BytesMut,
    write_timeout: core::time::Duration,
) -> Result<()>
where
    T: crate::transport::AsyncTransport,
    Ex: crate::executor::Executor,
    S: SchedulerLike,
    Env: Envelope,
{
    // Frame the command directly into the reusable send buffer (zero allocation)
    let meta = envelope.frame_into(cmd.command.as_slice(), cmd.kind(), send_buf);

    // Wire-level TX logging for diagnostics
    trace!(
        target: "grafton_visca::wire",
        id = %cmd.id,
        kind = %if cmd.kind() == CommandKind::Inquiry { "INQ" } else { "CMD" },
        len = send_buf.len(),
        hex = %hex_bytes(&send_buf[..]),
        "TX",
    );

    // Create guard for tracking rollback state
    let mut guard = SendGuard::new(cmd.id);

    // For inquiries, start tracking without socket allocation
    // For commands, register as pending ACK
    if cmd.kind() == CommandKind::Inquiry {
        scheduler.start_inquiry(&cmd);
        trace!("Started tracking inquiry {id}", id = cmd.id);
    } else {
        scheduler.register_pending_ack(&cmd);
        trace!("Registered pending ACK for command {id}", id = cmd.id);
    }

    // Try to send the command with timeout using race
    let send_result = {
        use futures_lite::future;

        future::race(
            async { transport.send(&send_buf[..]).await.map(|_| ()) },
            async {
                executor.sleep(write_timeout).await;
                Err(crate::Error::Timeout)
            },
        )
        .await
    };

    match send_result {
        Ok(()) => {
            trace!(
                "Successfully sent {kind} {id}",
                kind = if cmd.kind() == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                id = cmd.id
            );

            // Register Sony sequence if present
            if let Some(seq) = meta.sequence {
                scheduler.register_sequence(cmd.id, seq);
                trace!(
                    "Registered Sony sequence {seq} for command {id}",
                    seq = seq,
                    id = cmd.id
                );
            }

            // Mark as committed to prevent rollback
            guard.commit();
            Ok(())
        }
        Err(e) => {
            let error_type = if e.kind() == crate::ErrorKind::Timeout {
                "timeout"
            } else {
                "failed"
            };

            error!(
                "Send {error_type} for {kind} {id}: {error:?}",
                error_type = error_type,
                kind = if cmd.kind() == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                id = cmd.id,
                error = e
            );

            // Rollback: pass the original error to preserve timeout semantics.
            // For async mode, AsyncAdapter::fail_after_send_error handles
            // the action internally (updates metrics and notifies via response channel),
            // so we can safely ignore the None return value.
            guard.rollback(scheduler, e.clone());

            // Return error to caller
            Err(e)
        }
    }
}

/// Blocking version of send_one for non-async builds.
///
/// Returns a `SendResult` that includes both the error (if any) and the
/// scheduler action that must be checked by the caller for immediate
/// error propagation to the target command.
#[cfg(not(feature = "mode-async"))]
pub(crate) fn send_one<T, S, Env>(
    transport: &mut T,
    scheduler: &mut S,
    cmd: PendingCommand,
    envelope: &Env,
    send_buf: &mut bytes::BytesMut,
    _write_timeout: core::time::Duration,
) -> SendResult
where
    T: crate::transport::BlockingTransport,
    S: SchedulerLike,
    Env: Envelope,
{
    // Frame the command directly into the reusable send buffer (zero allocation)
    let meta = envelope.frame_into(cmd.command.as_slice(), cmd.kind(), send_buf);

    // Wire-level TX logging for diagnostics
    trace!(
        target: "grafton_visca::wire",
        id = %cmd.id,
        kind = %if cmd.kind() == CommandKind::Inquiry { "INQ" } else { "CMD" },
        len = send_buf.len(),
        hex = %hex_bytes(&send_buf[..]),
        "TX",
    );

    // Create guard for tracking rollback state
    let mut guard = SendGuard::new(cmd.id);

    // For inquiries, start tracking without socket allocation
    // For commands, register as pending ACK
    if cmd.kind() == CommandKind::Inquiry {
        scheduler.start_inquiry(&cmd);
        trace!("Started tracking inquiry {id}", id = cmd.id);
    } else {
        scheduler.register_pending_ack(&cmd);
        trace!("Registered pending ACK for command {id}", id = cmd.id);
    }

    // Try to send the command with timeout
    // In blocking mode, we use the transport directly
    let send_result = transport.send_with_kind(&send_buf[..], cmd.kind());

    match send_result {
        Ok(()) => {
            trace!(
                "Successfully sent {kind} {id}",
                kind = if cmd.kind() == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                id = cmd.id
            );

            // Register Sony sequence if present
            if let Some(seq) = meta.sequence {
                scheduler.register_sequence(cmd.id, seq);
                trace!(
                    "Registered Sony sequence {seq} for command {id}",
                    seq = seq,
                    id = cmd.id
                );
            }

            // Mark as committed to prevent rollback
            guard.commit();
            SendResult::Ok
        }
        Err(e) => {
            let error_type = if e.kind() == crate::ErrorKind::Timeout {
                "timeout"
            } else {
                "failed"
            };

            error!(
                "Send {error_type} for {kind} {id}: {error:?}",
                error_type = error_type,
                kind = if cmd.kind() == CommandKind::Inquiry {
                    "inquiry"
                } else {
                    "command"
                },
                id = cmd.id,
                error = e
            );

            // Rollback and get the scheduler action for the caller to handle.
            // Pass the original error to preserve timeout semantics.
            let action = guard.rollback(scheduler, e.clone());

            // Return error with action for caller to check
            SendResult::Err { error: e, action }
        }
    }
}
