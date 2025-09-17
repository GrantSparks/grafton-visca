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
pub mod driver;
pub mod inquiry_matcher;

#[cfg(not(feature = "mode-async"))]
pub mod blocking_runner;

#[cfg(feature = "mode-async")]
pub mod traits;

#[cfg(feature = "mode-async")]
mod async_adapter;
#[cfg(feature = "mode-async")]
mod handle;
#[cfg(feature = "mode-async")]
mod loop_task;

pub use core::Priority;

#[cfg(feature = "mode-async")]
pub use async_adapter::MetricsSummary;

#[cfg(feature = "mode-async")]
#[doc(hidden)]
pub use handle::RuntimeHandle;

// Re-export runtime traits at the module level for compatibility
#[cfg(feature = "mode-async")]
pub use traits::{Runtime, TransportHandle};

#[cfg(all(feature = "mode-async", feature = "transport-serial-tokio"))]
pub use traits::RuntimeSerial;

#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub use traits::TokioRuntime;

#[cfg(all(feature = "mode-async", feature = "runtime-async-std"))]
pub use traits::AsyncStdRuntime;

#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
pub use traits::SmolRuntime;

#[cfg(test)]
mod tests {

    #[cfg(feature = "mode-async")]
    #[test]
    fn test_socket_id() {
        use crate::ViscaSocket;
        assert_eq!(ViscaSocket::S1.as_index(), 0);
        assert_eq!(ViscaSocket::S2.as_index(), 1);
    }
}
