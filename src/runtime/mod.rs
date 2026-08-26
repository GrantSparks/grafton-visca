//! VISCA runtime implementation using flume channels.
//!
//! This module provides the core runtime for VISCA communication,
//! managing command scheduling, socket allocation, and protocol timing.
//!
//! ## Runtime Boundary
//!
//! Async handles send command and inquiry traffic over a bounded data-plane
//! submission channel sized by `TransportConfig::max_pending_queue_depth`.
//! A command or inquiry is not considered accepted until the runtime loop admits
//! it into scheduler state and replies on its admission channel. Command IDs are
//! returned to callers only after that admission point, so ID-based cancellation
//! is valid immediately.
//!
//! Cancellation and shutdown use an urgent control-plane channel that is
//! independent of data-plane capacity and selected ahead of normal control
//! traffic. Metrics snapshots and completion subscriptions use a separate normal
//! control-plane channel, preserving wake-driven liveness without allowing
//! observability traffic to block shutdown or cancellation.
//!
//! Explicit shutdown is immediate: the runtime stops accepting new work, fails
//! accepted and queued command/inquiry futures with `Error::RuntimeShutdown`,
//! replies to pending control requests, drops completion subscribers, and then
//! exits the runtime loop.
//!
//! ## Connection Sharing for Multi-Client Applications
//!
//! When building applications where multiple clients may control the same physical
//! camera (e.g., web servers, MCP servers, multi-session applications), **share a
//! single [`Camera`] instance across all clients** rather than creating separate
//! connections.
//!
//! PTZ cameras typically cannot reliably handle multiple concurrent TCP/UDP
//! connections to the same device, which may cause:
//! - Inquiry response timeouts (5+ seconds)
//! - Command delivery failures
//! - Unpredictable response routing between connections
//!
//! The async [`Camera`] (and its underlying runtime) is designed for
//! concurrent access:
//! - **Thread-safe**: Internal synchronization handles concurrent command submission
//! - **Clone-friendly**: the async `Camera` is [`Clone`]; `camera.clone()` creates a
//!   lightweight handle to the same runtime and the same underlying connection,
//!   and the connection is torn down only when the last handle is dropped
//! - **Command sequencing**: The runtime automatically sequences commands per VISCA
//!   protocol requirements
//!
//! The blocking `Camera` owns its transport and is **not** `Clone`; share it by
//! keeping the one value behind your own `Rc<RefCell<_>>` or `Arc<Mutex<_>>`.
//!
//! ### Example: Connection Pooling Pattern
//!
//! ```rust,no_run
//! # #[cfg(feature = "runtime-tokio")]
//! use std::collections::HashMap;
//! # #[cfg(feature = "runtime-tokio")]
//! use grafton_visca::{
//!     camera::{profiles::PtzOpticsG2, Camera, Connect},
//!     mode::Async,
//!     runtime::{TokioRuntime, TransportHandle},
//!     Error,
//! };
//! # #[cfg(feature = "runtime-tokio")]
//! use tokio::sync::RwLock;
//!
//! # #[cfg(feature = "runtime-tokio")]
//! type PooledCamera = Camera<Async, PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime>;
//!
//! // Shared camera pool for multi-client access: one connection per address,
//! // handed out as clones.
//! # #[cfg(feature = "runtime-tokio")]
//! struct CameraPool {
//!     runtime: TokioRuntime,
//!     cameras: RwLock<HashMap<String, PooledCamera>>,
//! }
//!
//! # #[cfg(feature = "runtime-tokio")]
//! impl CameraPool {
//!     /// Get or create a camera connection for the given address.
//!     /// Returns a cloned handle - all clients share the same underlying connection.
//!     async fn get_camera(&self, address: &str) -> Result<PooledCamera, Error> {
//!         // Check if camera already exists
//!         if let Some(camera) = self.cameras.read().await.get(address) {
//!             return Ok(camera.clone()); // Lightweight clone, same connection
//!         }
//!
//!         // Create new connection (only happens once per physical camera)
//!         let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(address, self.runtime.clone())
//!             .await?
//!             .into_inner();
//!
//!         // Another task may have opened the same address in the meantime;
//!         // keep whichever connection reached the map first.
//!         let mut cameras = self.cameras.write().await;
//!         let pooled = cameras.entry(address.to_string()).or_insert(camera);
//!         Ok(pooled.clone())
//!     }
//! }
//! # fn main() {}
//! ```
//!
//! [`Camera`]: crate::Camera
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

pub(crate) mod core;
pub(crate) mod driver;

#[cfg(not(feature = "mode-async"))]
pub(crate) mod blocking_runner;

#[cfg(feature = "mode-async")]
mod traits;

#[cfg(feature = "mode-async")]
mod async_adapter;
#[cfg(feature = "mode-async")]
mod handle;
#[cfg(feature = "mode-async")]
mod loop_task;

#[cfg(feature = "mode-async")]
pub(crate) use handle::RuntimeHandle;

// Command scheduling priority is part of the stable surface in every build
// configuration, including the blocking (non-`mode-async`) default.
pub use core::Priority;

// Re-export the stable runtime contract at the module level.
#[cfg(feature = "mode-async")]
pub use traits::{Runtime, TransportHandle};

#[cfg(all(feature = "mode-async", feature = "transport-serial-tokio"))]
pub use traits::RuntimeSerial;

#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
pub use traits::TokioRuntime;

#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
pub use traits::SmolRuntime;

/// Runtime test harness utilities.
///
/// Enable the `test-utils` feature to use these from integration tests. The
/// production runtime contract is the [`Runtime`] trait plus concrete runtime
/// adapters such as `TokioRuntime` and `SmolRuntime`.
#[cfg(feature = "test-utils")]
pub mod testing {
    #[cfg(feature = "mode-async")]
    pub use super::async_adapter::{CompletionEvent, MetricsSummary};
    #[cfg(not(feature = "mode-async"))]
    pub use super::blocking_runner::BlockingRunner;
    pub use super::core::Priority;
    #[cfg(feature = "mode-async")]
    pub use super::handle::RuntimeHandle;
}

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
