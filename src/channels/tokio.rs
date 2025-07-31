//! Tokio-based channel implementations.

use crate::error::{Error, Result};
use std::borrow::Cow;

/// Sender half of an unbounded channel.
#[derive(Debug)]
pub struct UnboundedSender<T>(tokio::sync::mpsc::UnboundedSender<T>);

impl<T> UnboundedSender<T> {
    /// Create a new unbounded sender.
    pub(super) fn new(tx: tokio::sync::mpsc::UnboundedSender<T>) -> Self {
        Self(tx)
    }

    /// Sends a value on this channel.
    ///
    /// This will always succeed unless the receiver has been dropped.
    pub fn send(&self, value: T) -> Result<()> {
        self.0.send(value).map_err(|_| Error::ChannelClosed)
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Receiver half of an unbounded channel.
#[derive(Debug)]
pub struct UnboundedReceiver<T>(tokio::sync::mpsc::UnboundedReceiver<T>);

impl<T> UnboundedReceiver<T> {
    /// Create a new unbounded receiver.
    pub(super) fn new(rx: tokio::sync::mpsc::UnboundedReceiver<T>) -> Self {
        Self(rx)
    }

    /// Receives a value from this channel.
    ///
    /// Returns `None` if all senders have been dropped.
    pub async fn recv(&mut self) -> Option<T> {
        self.0.recv().await
    }

    /// Try to receive a value without blocking.
    ///
    /// Returns `None` if no message is available or all senders have been dropped.
    #[allow(dead_code)] // Used in tests
    pub fn try_recv(&mut self) -> Option<T> {
        self.0.try_recv().ok()
    }
}

/// Sender half of a oneshot channel.
#[derive(Debug)]
pub struct OneshotSender<T>(tokio::sync::oneshot::Sender<T>);

impl<T> OneshotSender<T> {
    /// Create a new oneshot sender.
    pub(super) fn new(tx: tokio::sync::oneshot::Sender<T>) -> Self {
        Self(tx)
    }

    /// Sends a value on this channel.
    ///
    /// This consumes the sender, ensuring only one value can be sent.
    /// Returns an error containing the value if the receiver has been dropped.
    pub fn send(self, value: T) -> std::result::Result<(), T> {
        self.0.send(value)
    }
}

/// Receiver half of a oneshot channel.
#[derive(Debug)]
pub struct OneshotReceiver<T>(tokio::sync::oneshot::Receiver<T>);

impl<T> OneshotReceiver<T> {
    /// Create a new oneshot receiver.
    pub(super) fn new(rx: tokio::sync::oneshot::Receiver<T>) -> Self {
        Self(rx)
    }

    /// Receives a value from this channel.
    ///
    /// This consumes the receiver, ensuring only one value can be received.
    pub async fn recv(self) -> Result<T> {
        self.0
            .await
            .map_err(|_| Error::TransportError(Cow::Borrowed("Response channel closed")))
    }
}

/// Creates an unbounded multi-producer, single-consumer channel.
///
/// This provides consistent unbounded semantics across both async and blocking modes.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    (UnboundedSender::new(tx), UnboundedReceiver::new(rx))
}

/// Creates a one-shot channel that can send exactly one value.
///
/// This provides consistent oneshot semantics across both async and blocking modes.
pub fn oneshot<T>() -> (OneshotSender<T>, OneshotReceiver<T>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    (OneshotSender::new(tx), OneshotReceiver::new(rx))
}
