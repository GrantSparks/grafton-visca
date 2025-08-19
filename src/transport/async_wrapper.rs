//! Generic async wrapper for blocking transports.
//!
//! This module provides a wrapper that allows blocking transports to be used
//! in async contexts by running blocking operations in a thread pool. This
//! enables gradual migration from blocking to async code and allows reuse
//! of existing blocking transport implementations.

use bytes::Bytes;

use std::sync::Arc;

use crate::transport::{AsyncTransport, BlockingTransport};
use crate::Error;

/// Async wrapper for blocking transports.
///
/// This wrapper allows any blocking transport to be used as an async transport
/// by executing blocking operations in a thread pool. This is useful for:
///
/// - Gradual migration from blocking to async code
/// - Using blocking-only transports in async contexts
/// - Testing async code with simpler blocking implementations
///
/// # Performance Considerations
///
/// While this wrapper enables async usage, it does involve thread pool overhead.
/// For optimal performance in async contexts, prefer native async transports
/// when available.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "async")]
/// use grafton_visca::transport::{AsyncWrapper, BlockingTransport, AsyncTransport};
/// use grafton_visca::transport::blocking::Tcp;
///
/// # #[cfg(feature = "async")]
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a blocking transport
/// let blocking_transport = Tcp::connect("192.168.0.110:5678")?;
///
/// // Wrap it for async usage
/// let async_transport = AsyncWrapper::new(blocking_transport);
///
/// // Now it can be used as an AsyncTransport
/// async_transport.send(b"\x81\x01\x04\x00\x02\xFF").await?;
/// let response = async_transport.recv().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct AsyncWrapper<T: BlockingTransport> {
    transport: Arc<T>,
}

// Manual Clone implementation to avoid requiring T: Clone
impl<T: BlockingTransport> Clone for AsyncWrapper<T> {
    fn clone(&self) -> Self {
        Self {
            transport: Arc::clone(&self.transport),
        }
    }
}

impl<T: BlockingTransport + 'static> AsyncWrapper<T> {
    /// Create a new async wrapper for a blocking transport.
    ///
    /// The transport is wrapped in an `Arc` to allow safe sharing across
    /// async tasks. The wrapper will execute blocking operations in a
    /// thread pool to avoid blocking the async runtime.
    pub fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(transport),
        }
    }

    /// Create a wrapper from an already Arc-wrapped transport.
    ///
    /// This is useful when you already have an `Arc<T>` and want to avoid
    /// double-wrapping.
    pub fn from_arc(transport: Arc<T>) -> Self {
        Self { transport }
    }

    /// Get a reference to the inner blocking transport.
    ///
    /// This can be useful for accessing transport-specific methods that
    /// aren't part of the `BlockingTransport` trait.
    pub fn inner(&self) -> &T {
        &self.transport
    }

    /// Extract the inner blocking transport.
    ///
    /// This will only succeed if there are no other references to the
    /// transport (i.e., this is the only `AsyncWrapper` instance).
    pub fn into_inner(self) -> Result<T, Arc<T>> {
        Arc::try_unwrap(self.transport)
    }
}

#[cfg(feature = "async")]
impl<T: BlockingTransport + 'static> AsyncTransport for AsyncWrapper<T> {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        let transport = self.transport.clone();
        let bytes = bytes.to_vec();

        // Execute the blocking operation in a thread pool
        #[cfg(feature = "rt-tokio")]
        {
            tokio::task::spawn_blocking(move || transport.send_blocking(&bytes))
                .await
                .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
        }

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        {
            async_std::task::spawn_blocking(move || transport.send_blocking(&bytes)).await
        }

        #[cfg(all(
            feature = "rt-smol",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std")
        ))]
        {
            smol::unblock(move || transport.send_blocking(&bytes)).await
        }

        // Fallback for test-utils or when no specific runtime is selected
        #[cfg(all(
            feature = "async",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std"),
            not(feature = "rt-smol")
        ))]
        {
            // Use futures-lite's blocking executor
            futures_lite::future::block_on(async move {
                // In this context, we need to spawn the blocking operation
                // This is a simplified fallback for testing
                std::thread::spawn(move || transport.send_blocking(&bytes))
                    .join()
                    .map_err(|_| Error::Io(std::io::Error::other("Thread panicked")))?
            })
        }
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        let transport = self.transport.clone();

        // Execute the blocking operation in a thread pool
        #[cfg(feature = "rt-tokio")]
        {
            tokio::task::spawn_blocking(move || transport.recv_blocking())
                .await
                .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
        }

        #[cfg(all(feature = "rt-async-std", not(feature = "rt-tokio")))]
        {
            async_std::task::spawn_blocking(move || transport.recv_blocking()).await
        }

        #[cfg(all(
            feature = "rt-smol",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std")
        ))]
        {
            smol::unblock(move || transport.recv_blocking()).await
        }

        // Fallback for test-utils or when no specific runtime is selected
        #[cfg(all(
            feature = "async",
            not(feature = "rt-tokio"),
            not(feature = "rt-async-std"),
            not(feature = "rt-smol")
        ))]
        {
            // Use futures-lite's blocking executor
            futures_lite::future::block_on(async move {
                // In this context, we need to spawn the blocking operation
                // This is a simplified fallback for testing
                std::thread::spawn(move || transport.recv_blocking())
                    .join()
                    .map_err(|_| Error::Io(std::io::Error::other("Thread panicked")))?
            })
        }
    }
}

/// Extension trait for convenient wrapper creation.
///
/// This trait provides a convenient method to convert any blocking transport
/// into an async transport using the wrapper.
pub trait AsyncWrapperExt: BlockingTransport + Sized + 'static {
    /// Convert this blocking transport into an async transport.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "async")]
    /// use grafton_visca::transport::{AsyncWrapperExt, BlockingTransport};
    /// use grafton_visca::transport::blocking::Tcp;
    ///
    /// # #[cfg(feature = "async")]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let async_transport = Tcp::connect("192.168.0.110:5678")?.into_async();
    /// # Ok(())
    /// # }
    /// ```
    fn into_async(self) -> AsyncWrapper<Self> {
        AsyncWrapper::new(self)
    }

    /// Create an async wrapper from a reference.
    ///
    /// This is useful when you want to keep ownership of the transport
    /// but still use it in async contexts.
    fn as_async(self: Arc<Self>) -> AsyncWrapper<Self> {
        AsyncWrapper::from_arc(self)
    }
}

// Implement the extension trait for all blocking transports
impl<T: BlockingTransport + 'static> AsyncWrapperExt for T {}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use bytes::Bytes;

    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;

    // Mock blocking transport for testing
    #[derive(Debug, Clone)]
    struct MockBlockingTransport {
        send_data: Arc<Mutex<Vec<Vec<u8>>>>,
        recv_data: Arc<Mutex<Vec<Bytes>>>,
    }

    impl MockBlockingTransport {
        fn new() -> Self {
            Self {
                send_data: Arc::new(Mutex::new(Vec::new())),
                recv_data: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn add_response(&self, data: Vec<u8>) {
            self.recv_data
                .lock()
                .expect("Mock lock poisoned")
                .push(Bytes::from(data));
        }

        #[allow(dead_code)]
        fn get_sent_data(&self) -> Vec<Vec<u8>> {
            self.send_data.lock().expect("Mock lock poisoned").clone()
        }
    }

    impl BlockingTransport for MockBlockingTransport {
        fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
            self.send_data
                .lock()
                .expect("Mock lock poisoned")
                .push(bytes.to_vec());
            Ok(())
        }

        fn recv_blocking(&self) -> Result<Bytes, Error> {
            self.recv_data
                .lock()
                .expect("Mock lock poisoned")
                .pop()
                .ok_or(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "No data available",
                )))
        }

        fn recv_blocking_with_timeout(&self, _timeout: Duration) -> Result<Bytes, Error> {
            self.recv_blocking()
        }
    }

    #[test]
    fn test_wrapper_creation() {
        let transport = MockBlockingTransport::new();
        let wrapper = AsyncWrapper::new(transport);
        assert!(wrapper
            .inner()
            .send_data
            .lock()
            .expect("Mock lock poisoned")
            .is_empty());
    }

    #[test]
    fn test_wrapper_from_arc() {
        let transport = Arc::new(MockBlockingTransport::new());
        let wrapper = AsyncWrapper::from_arc(transport.clone());
        assert!(Arc::ptr_eq(&wrapper.transport, &transport));
    }

    #[test]
    fn test_into_inner_success() {
        let transport = MockBlockingTransport::new();
        let wrapper = AsyncWrapper::new(transport);
        let result = wrapper.into_inner();
        assert!(result.is_ok());
    }

    #[test]
    fn test_into_inner_failure_multiple_refs() {
        let transport = MockBlockingTransport::new();
        let wrapper = AsyncWrapper::new(transport);
        let _wrapper2 = wrapper.clone();
        let result = wrapper.into_inner();
        assert!(result.is_err());
    }

    #[cfg(feature = "async")]
    #[test]
    fn test_async_wrapper_ext() {
        let transport = MockBlockingTransport::new();
        let _wrapper = transport.into_async();
    }

    #[cfg(feature = "async")]
    #[test]
    fn test_async_wrapper_ext_from_arc() {
        let transport = Arc::new(MockBlockingTransport::new());
        let _wrapper = transport.as_async();
    }

    // Async runtime tests would require specific runtime features to be enabled
    #[cfg(all(test, feature = "rt-tokio"))]
    mod tokio_tests {
        use super::*;

        #[tokio::test]
        async fn test_async_send() {
            let transport = MockBlockingTransport::new();
            let wrapper = AsyncWrapper::new(transport);

            let result = wrapper.send(b"test data").await;
            assert!(result.is_ok());

            let sent = wrapper.inner().get_sent_data();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0], b"test data");
        }

        #[tokio::test]
        async fn test_async_recv() {
            let transport = MockBlockingTransport::new();
            transport.add_response(vec![0x90, 0x50, VISCA_TERMINATOR]);
            let wrapper = AsyncWrapper::new(transport);

            let result = wrapper.recv().await;
            assert!(result.is_ok());
            assert_eq!(
                result.expect("Failed to receive in test"),
                Bytes::from(vec![0x90, 0x50, VISCA_TERMINATOR])
            );
        }

        #[tokio::test]
        async fn test_async_send_recv_sequence() {
            let transport = MockBlockingTransport::new();
            transport.add_response(vec![0x90, 0x41, VISCA_TERMINATOR]);
            transport.add_response(vec![0x90, 0x51, VISCA_TERMINATOR]);
            let wrapper = AsyncWrapper::new(transport);

            // Send command
            wrapper
                .send(b"\x81\x01\x04\x00\x02\xFF")
                .await
                .expect("Failed to send in test");

            // Receive ACK
            let ack = wrapper.recv().await.expect("Failed to receive ACK in test");
            assert_eq!(ack, Bytes::from(vec![0x90, 0x51, VISCA_TERMINATOR]));

            // Receive completion
            let completion = wrapper
                .recv()
                .await
                .expect("Failed to receive completion in test");
            assert_eq!(completion, Bytes::from(vec![0x90, 0x41, VISCA_TERMINATOR]));

            // Verify sent data
            let sent = wrapper.inner().get_sent_data();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0], b"\x81\x01\x04\x00\x02\xFF");
        }
    }
}
