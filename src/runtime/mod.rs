//! VISCA runtime implementation using flume channels.
//!
//! This module provides the core runtime for VISCA communication,
//! managing command scheduling, socket allocation, and protocol timing.
//!
//! ## Wire-Error Semantics
//!
//! The runtime handles transport errors with specific policies designed to
//! maximize resilience while preserving error observability:
//!
//! ### Send Failures
//! When a command fails to send (transport returns an error):
//! - **Immediate failure**: The command fails immediately with `TransportError`
//! - **Rollback**: Any reserved sockets or pending ACK registrations are rolled back
//! - **No wire side-effects**: Since the command never reached the wire, no protocol
//!   state is affected
//! - **No retry**: Send failures are considered fatal for that specific attempt
//!
//! ### Receive Failures
//! When the transport fails to receive a response:
//! - **Transient treatment**: Treated as a temporary network condition
//! - **Automatic retry**: Commands are retried according to the configured retry policy
//! - **Eventual timeout**: After exhausting retries, commands fail with `Error::Timeout`
//! - **Preserved cause**: The original transport error is preserved internally for
//!   debugging, though the public API returns `Timeout` for consistency
//!
//! ### Rationale
//! This dual approach balances several concerns:
//! 1. **Send failures** are unrecoverable for that attempt since we cannot know if
//!    the command reached the camera
//! 2. **Receive failures** may be transient network issues, so retry is appropriate
//! 3. **Timeout** as the final error maintains API compatibility and indicates the
//!    operation didn't complete within the expected time
//!
//! ### Breaking Changes (v0.x → v1.0)
//! - Send failures now fail immediately instead of retrying
//! - Receive failures result in `Timeout` rather than `TransportError`
//! - This provides better error specificity while maintaining resilience

pub mod core;

#[cfg(not(feature = "async"))]
pub mod blocking_runner;

#[cfg(feature = "async")]
mod async_adapter;
#[cfg(feature = "async")]
mod handle;
#[cfg(feature = "async")]
mod loop_task;

pub use core::Priority;

#[cfg(feature = "async")]
pub use async_adapter::MetricsSummary;

#[cfg(feature = "async")]
pub use handle::RuntimeHandle;

#[cfg(test)]
mod tests {

    #[cfg(feature = "async")]
    #[test]
    fn test_socket_id() {
        use crate::ViscaSocket;
        assert_eq!(ViscaSocket::S1.as_index(), 0);
        assert_eq!(ViscaSocket::S2.as_index(), 1);
    }
}
