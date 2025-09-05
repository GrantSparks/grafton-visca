//! VISCA runtime implementation using flume channels.
//!
//! This module provides the core runtime for VISCA communication,
//! managing command scheduling, socket allocation, and protocol timing.

pub mod core;
pub mod scheduler;

#[cfg(not(feature = "async"))]
pub mod blocking_runner;

#[cfg(feature = "async")]
mod handle;
#[cfg(feature = "async")]
mod loop_task;
#[cfg(feature = "async")]
mod queue;
#[cfg(feature = "async")]
mod rx;
#[cfg(feature = "async")]
mod tx;

pub use core::Priority;
pub use scheduler::MetricsSummary;

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
