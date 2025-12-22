//! Scheduler abstraction for runtime-neutral command management.
//!
//! This module provides the `SchedulerLike` trait that abstracts over
//! the async adapter and blocking scheduler core, providing a unified
//! interface for command state management.

use crate::runtime::core::PendingCommand;

/// Runtime-neutral scheduler abstraction.
///
/// This trait provides a unified interface for managing command state
/// across both async and blocking runtimes. It abstracts the common
/// operations needed for the send/recv pipeline while allowing each
/// implementation to maintain its specific state management approach.
pub trait SchedulerLike {
    /// Start tracking an inquiry command.
    ///
    /// This method begins tracking an inquiry without allocating a socket.
    /// The socket allocation happens later when the first ACK is received.
    fn start_inquiry(&mut self, cmd: &PendingCommand);

    /// Register a command as pending ACK.
    ///
    /// This method tracks that a command has been sent and is awaiting
    /// an ACK response from the device.
    fn register_pending_ack(&mut self, cmd: &PendingCommand);

    /// Unregister a pending ACK.
    ///
    /// This method removes a command from the pending ACK tracking,
    /// typically called during rollback on send failure.
    fn unregister_pending_ack(&mut self, id: u32);

    /// Fail a command immediately after send error.
    ///
    /// This method handles send failures by immediately failing the command
    /// with the original error wrapped in "Send failed" context.
    ///
    /// The `cause` parameter preserves the original error (including timeout semantics)
    /// while adding context about when the failure occurred.
    ///
    /// # Implementation Notes
    ///
    /// - **Async**: The `AsyncAdapter` handles the action internally by routing
    ///   it through the unified `apply_action` path, which updates metrics and
    ///   notifies the waiting future via the response channel. Returns `None`
    ///   since no further caller action is required.
    ///
    /// - **Blocking**: Returns `Some(SchedulerAction::CommandFailed)` for the
    ///   caller to propagate the error to clients via the return value.
    fn fail_after_send_error(
        &mut self,
        id: u32,
        cause: crate::Error,
    ) -> Option<crate::runtime::core::SchedulerAction>;

    /// Register Sony sequence number for a command.
    ///
    /// This method associates a Sony protocol sequence number with a command ID,
    /// enabling proper response routing in Sony-encapsulated mode.
    fn register_sequence(&mut self, id: u32, seq: u32);

    /// Free a reserved socket.
    ///
    /// This method returns a socket to the pool, typically called during
    /// rollback when an inquiry send fails.
    fn free_socket(&mut self, socket: crate::visca_socket::ViscaSocket);
}

// Feature-gated implementation for async adapter
#[cfg(feature = "mode-async")]
mod async_impl {
    use super::*;
    use crate::{capabilities::Profile, executor::Executor, runtime::async_adapter::AsyncAdapter};

    impl<P, E> SchedulerLike for AsyncAdapter<P, E>
    where
        P: Profile,
        E: Executor,
    {
        fn start_inquiry(&mut self, cmd: &PendingCommand) {
            self.start_inquiry(cmd);
        }

        fn register_pending_ack(&mut self, cmd: &PendingCommand) {
            self.register_pending_ack(cmd);
        }

        fn unregister_pending_ack(&mut self, id: u32) {
            self.unregister_pending_ack(id);
        }

        fn fail_after_send_error(
            &mut self,
            id: u32,
            cause: crate::Error,
        ) -> Option<crate::runtime::core::SchedulerAction> {
            // AsyncAdapter handles the action internally by sending to response channel,
            // so we call it for the side effect and return None to indicate no further
            // action needed by the caller
            self.fail_after_send_error(id, cause);
            None
        }

        fn register_sequence(&mut self, id: u32, seq: u32) {
            self.register_sequence(id, seq);
        }

        fn free_socket(&mut self, socket: crate::visca_socket::ViscaSocket) {
            self.free_socket(socket);
        }
    }
}

// Feature-gated implementation for blocking scheduler core
#[cfg(not(feature = "mode-async"))]
pub use blocking_impl::BlockingScheduler;

#[cfg(not(feature = "mode-async"))]
mod blocking_impl {
    use std::time::Instant;

    use super::*;
    use crate::runtime::core::SchedulerCore;

    /// Wrapper for SchedulerCore to implement SchedulerLike.
    ///
    /// This wrapper provides the unified interface while mapping operations
    /// to the appropriate SchedulerCore methods.
    #[derive(Debug)]
    pub struct BlockingScheduler<'a> {
        /// Reference to the core scheduler.
        pub core: &'a mut SchedulerCore,
        /// Current timestamp for time-based operations.
        pub now: Instant,
    }

    impl<'a> SchedulerLike for BlockingScheduler<'a> {
        fn start_inquiry(&mut self, cmd: &PendingCommand) {
            self.core.start_inquiry(
                cmd.id,
                cmd.command.clone(),
                cmd.priority,
                cmd.category,
                cmd.camera_id,
                cmd.kind,
                self.now,
            );
        }

        fn register_pending_ack(&mut self, cmd: &PendingCommand) {
            self.core.register_pending_ack(
                cmd.id,
                cmd.command.clone(),
                cmd.priority,
                cmd.category,
                cmd.camera_id,
                cmd.kind,
                self.now,
            );
        }

        fn unregister_pending_ack(&mut self, id: u32) {
            self.core.unregister_pending_ack(id);
        }

        fn fail_after_send_error(
            &mut self,
            id: u32,
            cause: crate::Error,
        ) -> Option<crate::runtime::core::SchedulerAction> {
            // Return the action from core so caller can propagate it
            self.core.fail_after_send_error(id, cause)
        }

        fn register_sequence(&mut self, id: u32, seq: u32) {
            self.core.register_sequence(id, seq);
        }

        fn free_socket(&mut self, socket: crate::visca_socket::ViscaSocket) {
            self.core.free_socket(socket);
        }
    }
}
