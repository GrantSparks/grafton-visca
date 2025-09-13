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

    /// Schedule a retry after send error.
    ///
    /// This method handles send failures by scheduling the command for
    /// retry according to the configured retry policy.
    ///
    /// # Implementation Notes
    ///
    /// - Async: calls adapter's schedule_retry_after_send_error
    /// - Blocking: maps to mark_retry_as_transport_error + queue_retry_for_command
    fn schedule_retry_after_send_error(&mut self, id: u32);

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
#[cfg(feature = "async")]
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

        fn schedule_retry_after_send_error(&mut self, id: u32) {
            self.schedule_retry_after_send_error(id);
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
#[cfg(not(feature = "async"))]
pub use blocking_impl::BlockingScheduler;

#[cfg(not(feature = "async"))]
mod blocking_impl {
    use super::*;
    use crate::runtime::core::SchedulerCore;
    use std::time::Instant;

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
                cmd.bytes.clone(),
                cmd.priority,
                cmd.category,
                cmd.camera_id,
                self.now,
            );
        }

        fn register_pending_ack(&mut self, cmd: &PendingCommand) {
            self.core.register_pending_ack(
                cmd.id,
                cmd.bytes.clone(),
                cmd.priority,
                cmd.category,
                cmd.camera_id,
                self.now,
            );
        }

        fn unregister_pending_ack(&mut self, id: u32) {
            self.core.unregister_pending_ack(id);
        }

        fn schedule_retry_after_send_error(&mut self, id: u32) {
            // Map to core methods: mark as transport error and queue retry
            self.core.mark_retry_as_transport_error(id);
            self.core.queue_retry_for_command(id, self.now);
        }

        fn register_sequence(&mut self, id: u32, seq: u32) {
            self.core.register_sequence(id, seq);
        }

        fn free_socket(&mut self, socket: crate::visca_socket::ViscaSocket) {
            self.core.free_socket(socket);
        }
    }
}
