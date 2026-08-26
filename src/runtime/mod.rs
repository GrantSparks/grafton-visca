//! Runtime-neutral owner and deterministic engine integration.
//!
//! The protocol engine is shared by the async and blocking owners. Waiting,
//! transport execution, and observer delivery are implemented by the
//! corresponding owner projection; no mode-generic scheduler or future layer
//! is required.

pub(crate) mod engine;
pub(crate) mod owner;

#[cfg(feature = "async")]
mod traits;

#[cfg(all(feature = "async", feature = "transport-serial-tokio"))]
pub use traits::RuntimeSerial;
#[cfg(all(feature = "async", feature = "runtime-smol"))]
pub use traits::SmolRuntime;
#[cfg(all(feature = "async", feature = "runtime-tokio"))]
pub use traits::TokioRuntime;
#[cfg(feature = "async")]
pub use traits::{Runtime, TransportHandle};

#[cfg(test)]
mod tests {
    #[test]
    fn socket_ids_remain_zero_based() {
        use crate::ViscaSocket;

        assert_eq!(ViscaSocket::S1.as_index(), 0);
        assert_eq!(ViscaSocket::S2.as_index(), 1);
    }
}
