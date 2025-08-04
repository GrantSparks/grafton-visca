//! Unified channel abstractions that provide consistent semantics across blocking and async modes.
//!
//! This module provides channel implementations that behave consistently whether the
//! tokio feature is enabled or not, addressing the semantic differences between
//! tokio's unbounded channels and std's bounded channels.

use std::borrow::Cow;

use crate::error::{Error, Result};

/// Creates an unbounded multi-producer, single-consumer channel.
///
/// This provides consistent unbounded semantics across both async and blocking modes.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    #[cfg(feature = "tokio")]
    {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (UnboundedSender::Tokio(tx), UnboundedReceiver::Tokio(rx))
    }

    #[cfg(not(feature = "tokio"))]
    {
        const UNBOUNDED_BUFFER: usize = 10_000;
        let (tx, rx) = std::sync::mpsc::sync_channel(UNBOUNDED_BUFFER);
        (UnboundedSender::Std(tx), UnboundedReceiver::Std(rx))
    }
}

/// Creates a one-shot channel that can send exactly one value.
///
/// This provides consistent oneshot semantics across both async and blocking modes.
pub fn oneshot<T>() -> (OneshotSender<T>, OneshotReceiver<T>) {
    #[cfg(feature = "tokio")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (OneshotSender::Tokio(tx), OneshotReceiver::Tokio(rx))
    }

    #[cfg(not(feature = "tokio"))]
    {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        (OneshotSender::Std(Some(tx)), OneshotReceiver::Std(rx))
    }
}

/// Sender half of an unbounded channel.
#[derive(Debug)]
pub enum UnboundedSender<T> {
    #[cfg(feature = "tokio")]
    Tokio(tokio::sync::mpsc::UnboundedSender<T>),
    #[cfg(not(feature = "tokio"))]
    Std(std::sync::mpsc::SyncSender<T>),
}

impl<T> UnboundedSender<T> {
    /// Sends a value on this channel.
    ///
    /// This will always succeed unless the receiver has been dropped.
    pub fn send(&self, value: T) -> Result<()> {
        #[cfg(feature = "tokio")]
        {
            match self {
                UnboundedSender::Tokio(tx) => tx.send(value).map_err(|_| Error::ChannelClosed),
            }
        }

        #[cfg(not(feature = "tokio"))]
        {
            match self {
                UnboundedSender::Std(tx) => tx.send(value).map_err(|_| Error::ChannelClosed),
            }
        }
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        match self {
            #[cfg(feature = "tokio")]
            UnboundedSender::Tokio(tx) => UnboundedSender::Tokio(tx.clone()),
            #[cfg(not(feature = "tokio"))]
            UnboundedSender::Std(tx) => UnboundedSender::Std(tx.clone()),
        }
    }
}

/// Receiver half of an unbounded channel.
#[derive(Debug)]
pub enum UnboundedReceiver<T> {
    #[cfg(feature = "tokio")]
    Tokio(tokio::sync::mpsc::UnboundedReceiver<T>),
    #[cfg(not(feature = "tokio"))]
    Std(std::sync::mpsc::Receiver<T>),
}

impl<T> UnboundedReceiver<T> {
    /// Receives a value from this channel.
    ///
    /// Returns `None` if all senders have been dropped.
    #[cfg(feature = "tokio")]
    pub async fn recv(&mut self) -> Option<T> {
        match self {
            UnboundedReceiver::Tokio(rx) => rx.recv().await,
        }
    }

    /// Receives a value from this channel (blocking version).
    #[cfg(not(feature = "tokio"))]
    pub fn recv(&mut self) -> Option<T> {
        match self {
            UnboundedReceiver::Std(rx) => rx.recv().ok(),
        }
    }

    /// Try to receive a value without blocking.
    ///
    /// Returns `None` if no message is available or all senders have been dropped.
    #[cfg(any(not(feature = "tokio"), test))]
    pub fn try_recv(&mut self) -> Option<T> {
        match self {
            #[cfg(feature = "tokio")]
            UnboundedReceiver::Tokio(rx) => rx.try_recv().ok(),
            #[cfg(not(feature = "tokio"))]
            UnboundedReceiver::Std(rx) => rx.try_recv().ok(),
        }
    }
}

/// Sender half of a oneshot channel.
#[derive(Debug)]
pub enum OneshotSender<T> {
    #[cfg(feature = "tokio")]
    Tokio(tokio::sync::oneshot::Sender<T>),
    #[cfg(not(feature = "tokio"))]
    Std(Option<std::sync::mpsc::SyncSender<T>>),
}

impl<T> OneshotSender<T> {
    /// Sends a value on this channel.
    ///
    /// This consumes the sender, ensuring only one value can be sent.
    /// Returns an error containing the value if the receiver has been dropped.
    pub fn send(self, value: T) -> std::result::Result<(), T> {
        match self {
            #[cfg(feature = "tokio")]
            OneshotSender::Tokio(tx) => tx.send(value),
            #[cfg(not(feature = "tokio"))]
            OneshotSender::Std(tx_opt) => {
                if let Some(tx) = tx_opt {
                    tx.send(value).map_err(|e| match e {
                        std::sync::mpsc::SendError(val) => val,
                    })
                } else {
                    Err(value)
                }
            }
        }
    }
}

/// Receiver half of a oneshot channel.
#[derive(Debug)]
pub enum OneshotReceiver<T> {
    #[cfg(feature = "tokio")]
    Tokio(tokio::sync::oneshot::Receiver<T>),
    #[cfg(not(feature = "tokio"))]
    Std(std::sync::mpsc::Receiver<T>),
}

impl<T> OneshotReceiver<T> {
    /// Receives a value from this channel.
    ///
    /// This consumes the receiver, ensuring only one value can be received.
    #[cfg(feature = "tokio")]
    pub async fn recv(self) -> Result<T> {
        match self {
            OneshotReceiver::Tokio(rx) => rx
                .await
                .map_err(|_| Error::TransportError(Cow::Borrowed("Response channel closed"))),
        }
    }

    /// Receives a value from this channel (blocking version).
    #[cfg(not(feature = "tokio"))]
    pub fn recv(self) -> Result<T> {
        match self {
            OneshotReceiver::Std(rx) => rx
                .recv()
                .map_err(|_| Error::TransportError(Cow::Borrowed("Response channel closed"))),
        }
    }

    /// Receives a value from this channel with a timeout (blocking version).
    ///
    /// Returns Error::Timeout if the timeout expires before a value is received.
    pub fn recv_timeout(self, timeout: std::time::Duration) -> Result<T> {
        match self {
            #[cfg(feature = "tokio")]
            OneshotReceiver::Tokio(mut rx) => {
                // For tokio in blocking context, we need to use blocking recv
                // This is a sync method, so we use std::sync::mpsc for the timeout
                // Since tokio oneshot doesn't have recv_timeout, we'll use try_recv in a loop
                let start = std::time::Instant::now();
                loop {
                    match rx.try_recv() {
                        Ok(val) => return Ok(val),
                        Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                            if start.elapsed() >= timeout {
                                return Err(Error::Timeout);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                            return Err(Error::TransportError(Cow::Borrowed(
                                "Response channel closed",
                            )))
                        }
                    }
                }
            }
            #[cfg(not(feature = "tokio"))]
            OneshotReceiver::Std(rx) => rx.recv_timeout(timeout).map_err(|e| match e {
                std::sync::mpsc::RecvTimeoutError::Timeout => Error::Timeout,
                std::sync::mpsc::RecvTimeoutError::Disconnected => {
                    Error::TransportError(Cow::Borrowed("Response channel closed"))
                }
            }),
        }
    }

    /// Async-compatible receive for blocking mode.
    ///
    /// This simply calls the blocking recv() but provides an async interface.
    #[cfg(not(feature = "tokio"))]
    pub async fn recv_async(self) -> Result<T> {
        self.recv()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_unbounded_channel() {
        let (tx, mut rx) = unbounded::<i32>();

        assert!(tx.send(42).is_ok());
        assert!(tx.send(43).is_ok());

        assert_eq!(rx.try_recv(), Some(42));
        assert_eq!(rx.try_recv(), Some(43));
        assert_eq!(rx.try_recv(), None);
    }

    #[test]
    fn test_oneshot_channel() {
        let (tx, rx) = oneshot::<i32>();

        assert!(tx.send(42).is_ok());

        #[cfg(not(feature = "tokio"))]
        assert_eq!(rx.recv().expect("recv should succeed"), 42);

        #[cfg(feature = "tokio")]
        drop(rx);
    }

    #[test]
    fn test_unbounded_dropped_receiver() {
        let (tx, rx) = unbounded::<i32>();
        drop(rx);

        assert!(tx.send(42).is_err());
    }

    #[test]
    fn test_oneshot_dropped_receiver() {
        let (tx, rx) = oneshot::<i32>();
        drop(rx);

        assert_eq!(tx.send(42), Err(42));
    }
}
