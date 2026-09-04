//! Blocking transport implementations for VISCA communication.
//!
//! This module provides synchronous I/O for VISCA camera control,
//! designed as the primary API for most use cases.

use std::{io, time::Duration};

/// Restores a socket timeout when the scoped operation ends.
///
/// The operation's result is authoritative once bytes have been written to or
/// read from the socket.  A later restore failure can only describe cleanup of
/// local socket configuration; it must never rewrite that completed I/O into a
/// failure.  Keeping the cleanup in `Drop` also covers every early return from
/// the bounded operation.
pub(crate) struct TimeoutRestoreGuard<'a, Socket> {
    socket: &'a mut Socket,
    original_timeout: Option<Duration>,
    set_timeout: fn(&mut Socket, Option<Duration>) -> io::Result<()>,
    operation: &'static str,
}

impl<'a, Socket> TimeoutRestoreGuard<'a, Socket> {
    pub(crate) fn new(
        socket: &'a mut Socket,
        original_timeout: Option<Duration>,
        set_timeout: fn(&mut Socket, Option<Duration>) -> io::Result<()>,
        operation: &'static str,
    ) -> Self {
        Self {
            socket,
            original_timeout,
            set_timeout,
            operation,
        }
    }

    pub(crate) fn set_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        (self.set_timeout)(self.socket, timeout)
    }

    pub(crate) fn socket_mut(&mut self) -> &mut Socket {
        self.socket
    }
}

impl<Socket> Drop for TimeoutRestoreGuard<'_, Socket> {
    fn drop(&mut self) {
        if let Err(error) = (self.set_timeout)(self.socket, self.original_timeout) {
            tracing::warn!(
                operation = self.operation,
                %error,
                "failed to restore scoped socket timeout"
            );
        }
    }
}

pub mod tcp;
pub mod udp;

// Re-exports
pub use tcp::Tcp;
pub use udp::Udp;

#[cfg(test)]
mod tests;
