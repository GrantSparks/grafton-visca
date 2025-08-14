//! System control methods for cameras using the new GAT architecture.

use crate::{command::system::Socket, Error};

/// System operations (async).
#[cfg(feature = "async")]
pub trait SystemOps: Sized {
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    async fn trigger_address_assignment(&self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    async fn interface_clear(&self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    async fn cancel_command(&self, socket: Socket) -> Result<(), Error>;
}

/// System operations (blocking).
pub trait SystemOpsBlocking: Sized {
    /// Trigger automatic address assignment (broadcast command for serial bus).
    /// Note: This doesn't set a specific address but triggers the auto-addressing process.
    fn trigger_address_assignment(&self) -> Result<(), Error>;

    /// Clear interface (reset communication).
    fn interface_clear(&self) -> Result<(), Error>;

    /// Cancel command on specific socket.
    fn cancel_command(&self, socket: Socket) -> Result<(), Error>;
}

// Async implementation
