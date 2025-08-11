//! Unified channel abstractions that provide consistent semantics across blocking and async modes.
//!
//! This module provides channel implementations that behave consistently whether the
//! async feature is enabled or not, using runtime-agnostic async-channel when async
//! is enabled and std channels for blocking mode.

use crate::error::{Error, Result};

/// Creates an unbounded multi-producer, single-consumer channel.
///
/// This provides consistent unbounded semantics across both async and blocking modes.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    #[cfg(feature = "async")]
    {
        let (tx, rx) = async_channel::unbounded();
        (UnboundedSender::Async(tx), UnboundedReceiver::Async(rx))
    }

    #[cfg(not(feature = "async"))]
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
    #[cfg(feature = "async")]
    {
        let (tx, rx) = futures::channel::oneshot::channel();
        (OneshotSender::Async(tx), OneshotReceiver::Async(rx))
    }

    #[cfg(not(feature = "async"))]
    {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        (OneshotSender::Std(Some(tx)), OneshotReceiver::Std(rx))
    }
}

/// Sender half of an unbounded channel.
#[derive(Debug)]
pub enum UnboundedSender<T> {
    #[cfg(feature = "async")]
    Async(async_channel::Sender<T>),
    #[cfg(not(feature = "async"))]
    Std(std::sync::mpsc::SyncSender<T>),
}

impl<T> UnboundedSender<T> {
    /// Sends a value on this channel.
    ///
    /// This will always succeed unless the receiver has been dropped.
    pub fn send(&self, value: T) -> Result<()> {
        match self {
            #[cfg(feature = "async")]
            UnboundedSender::Async(tx) => {
                // async_channel::Sender::send is a blocking operation but
                // since the channel is unbounded, it should complete immediately
                tx.send_blocking(value).map_err(|_| Error::ChannelClosed)
            }
            #[cfg(not(feature = "async"))]
            UnboundedSender::Std(tx) => tx.send(value).map_err(|_| Error::ChannelClosed),
        }
    }
}

impl<T> Clone for UnboundedSender<T> {
    fn clone(&self) -> Self {
        match self {
            #[cfg(feature = "async")]
            UnboundedSender::Async(tx) => UnboundedSender::Async(tx.clone()),
            #[cfg(not(feature = "async"))]
            UnboundedSender::Std(tx) => UnboundedSender::Std(tx.clone()),
        }
    }
}

/// Receiver half of an unbounded channel.
#[derive(Debug)]
pub enum UnboundedReceiver<T> {
    #[cfg(feature = "async")]
    Async(async_channel::Receiver<T>),
    #[cfg(not(feature = "async"))]
    Std(std::sync::mpsc::Receiver<T>),
}

impl<T> UnboundedReceiver<T> {
    /// Receives a value from this channel.
    ///
    /// Returns `None` if all senders have been dropped.
    #[cfg(feature = "async")]
    pub async fn recv(&mut self) -> Option<T> {
        match self {
            UnboundedReceiver::Async(rx) => rx.recv().await.ok(),
        }
    }

    /// Receives a value from this channel (blocking version).
    #[cfg(not(feature = "async"))]
    pub fn recv(&mut self) -> Option<T> {
        match self {
            UnboundedReceiver::Std(rx) => rx.recv().ok(),
        }
    }

    /// Try to receive a value without blocking.
    ///
    /// Returns `None` if no message is available or all senders have been dropped.
    #[allow(dead_code)]
    pub fn try_recv(&mut self) -> Option<T> {
        match self {
            #[cfg(feature = "async")]
            UnboundedReceiver::Async(rx) => rx.try_recv().ok(),
            #[cfg(not(feature = "async"))]
            UnboundedReceiver::Std(rx) => rx.try_recv().ok(),
        }
    }
}

/// Sender half of a oneshot channel.
#[derive(Debug)]
pub enum OneshotSender<T> {
    #[cfg(feature = "async")]
    Async(futures::channel::oneshot::Sender<T>),
    #[cfg(not(feature = "async"))]
    Std(Option<std::sync::mpsc::SyncSender<T>>),
}

impl<T> OneshotSender<T> {
    /// Sends a value on this channel.
    ///
    /// This consumes the sender, ensuring only one value can be sent.
    /// Returns an error containing the value if the receiver has been dropped.
    pub fn send(self, value: T) -> std::result::Result<(), T> {
        match self {
            #[cfg(feature = "async")]
            OneshotSender::Async(tx) => tx.send(value),
            #[cfg(not(feature = "async"))]
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
    #[cfg(feature = "async")]
    Async(futures::channel::oneshot::Receiver<T>),
    #[cfg(not(feature = "async"))]
    Std(std::sync::mpsc::Receiver<T>),
}

impl<T> OneshotReceiver<T> {
    /// Receives a value from this channel.
    ///
    /// This consumes the receiver, ensuring only one value can be received.
    #[cfg(feature = "async")]
    pub async fn recv(self) -> Result<T> {
        match self {
            OneshotReceiver::Async(rx) => rx.await.map_err(|_| Error::ResponseChannelClosed),
        }
    }

    /// Receives a value from this channel (blocking version).
    #[cfg(not(feature = "async"))]
    pub fn recv(self) -> Result<T> {
        match self {
            OneshotReceiver::Std(rx) => rx.recv().map_err(|_| Error::ResponseChannelClosed),
        }
    }

    /// Receives a value from this channel with a timeout (blocking version).
    ///
    /// For async mode with runtime support, use timeout_with_runtime instead.
    /// Returns Error::Timeout if the timeout expires before a value is received.
    #[cfg(not(feature = "async"))]
    pub fn recv_timeout(self, timeout: std::time::Duration) -> Result<T> {
        match self {
            OneshotReceiver::Std(rx) => rx.recv_timeout(timeout).map_err(|e| match e {
                std::sync::mpsc::RecvTimeoutError::Timeout => Error::Timeout,
                std::sync::mpsc::RecvTimeoutError::Disconnected => Error::ResponseChannelClosed,
            }),
        }
    }

    /// Receives a value from this channel with a timeout using runtime.
    ///
    /// This method requires a runtime to be configured for timeout support.
    #[cfg(feature = "async")]
    #[allow(dead_code)]
    pub async fn recv_with_timeout(
        self,
        runtime: &dyn crate::runtime::Runtime,
        timeout: std::time::Duration,
    ) -> Result<T> {
        match self {
            OneshotReceiver::Async(rx) => {
                crate::runtime::timeout_with_runtime(runtime, timeout, rx)
                    .await?
                    .map_err(|_| Error::ResponseChannelClosed)
            }
        }
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

        #[cfg(not(feature = "async"))]
        assert_eq!(rx.recv().expect("recv should succeed"), 42);

        #[cfg(feature = "async")]
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

    #[cfg(all(test, feature = "async"))]
    mod async_tests {
        use super::*;

        #[tokio::test]
        async fn test_async_unbounded_channel() {
            let (tx, mut rx) = unbounded::<i32>();

            assert!(tx.send(42).is_ok());
            assert!(tx.send(43).is_ok());

            assert_eq!(rx.recv().await, Some(42));
            assert_eq!(rx.recv().await, Some(43));
        }

        #[tokio::test]
        async fn test_async_oneshot_channel() {
            let (tx, rx) = oneshot::<i32>();

            assert!(tx.send(42).is_ok());
            assert_eq!(rx.recv().await.expect("recv should succeed"), 42);
        }

        #[tokio::test]
        async fn test_async_oneshot_with_timeout() {
            use crate::runtime::TokioRuntime;
            use std::time::Duration;

            let runtime = TokioRuntime;
            let (_tx, rx) = oneshot::<i32>();

            // Should timeout
            let result = rx
                .recv_with_timeout(&runtime, Duration::from_millis(100))
                .await;
            assert!(matches!(result, Err(Error::Timeout)));
        }
    }
}
