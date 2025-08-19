//! Timeout management utilities for transport implementations.
//!
//! This module provides a unified way to handle timeouts across different
//! transport types, eliminating code duplication.

use crate::Error;
use std::io;
use std::net::{TcpStream, UdpSocket};
use std::sync::MutexGuard;
use std::time::Duration;

/// A trait for managing timeouts on socket operations.
///
/// This trait provides a consistent interface for saving, setting, and
/// restoring timeout values across different socket types.
pub trait TimeoutManager {
    /// Execute a function with a temporary timeout.
    ///
    /// This method saves the current timeout, sets a new timeout for the
    /// duration of the function execution, then restores the original timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration to use
    /// * `f` - The function to execute with the timeout
    ///
    /// # Returns
    ///
    /// The result of the function execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Getting or setting timeouts fails
    /// - The provided function returns an error
    fn with_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>;

    /// Get the current read timeout.
    fn get_read_timeout(&self) -> io::Result<Option<Duration>>;

    /// Set the read timeout.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()>;

    /// Get the current write timeout.
    fn get_write_timeout(&self) -> io::Result<Option<Duration>>;

    /// Set the write timeout.
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()>;

    /// Execute a function with a temporary read timeout.
    ///
    /// This is a convenience method that specifically manages read timeouts.
    fn with_read_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>,
    {
        // Save the current timeout
        let original_timeout = self.get_read_timeout()?;

        // Set the new timeout
        self.set_read_timeout(Some(timeout))?;

        // Execute the function
        let result = f(self);

        // Restore the original timeout
        self.set_read_timeout(original_timeout)?;

        result
    }

    /// Execute a function with a temporary write timeout.
    ///
    /// This is a convenience method that specifically manages write timeouts.
    fn with_write_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>,
    {
        // Save the current timeout
        let original_timeout = self.get_write_timeout()?;

        // Set the new timeout
        self.set_write_timeout(Some(timeout))?;

        // Execute the function
        let result = f(self);

        // Restore the original timeout
        self.set_write_timeout(original_timeout)?;

        result
    }
}

/// Implement TimeoutManager for TcpStream.
impl TimeoutManager for TcpStream {
    fn with_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>,
    {
        self.with_read_timeout(timeout, f)
    }

    fn get_read_timeout(&self) -> io::Result<Option<Duration>> {
        self.read_timeout()
    }

    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, timeout)
    }

    fn get_write_timeout(&self) -> io::Result<Option<Duration>> {
        self.write_timeout()
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        TcpStream::set_write_timeout(self, timeout)
    }
}

/// Implement TimeoutManager for UdpSocket.
impl TimeoutManager for UdpSocket {
    fn with_timeout<F, R>(&mut self, timeout: Duration, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Self) -> Result<R, Error>,
    {
        self.with_read_timeout(timeout, f)
    }

    fn get_read_timeout(&self) -> io::Result<Option<Duration>> {
        self.read_timeout()
    }

    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        UdpSocket::set_read_timeout(self, timeout)
    }

    fn get_write_timeout(&self) -> io::Result<Option<Duration>> {
        self.write_timeout()
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        UdpSocket::set_write_timeout(self, timeout)
    }
}

/// A helper struct to manage timeouts on a socket within a closure.
///
/// This struct ensures that timeouts are properly restored even if the
/// operation fails or panics.
pub struct TimeoutGuard<'a, T: TimeoutManager> {
    socket: &'a mut T,
    original_read_timeout: Option<Duration>,
    original_write_timeout: Option<Duration>,
    restored: bool,
}

impl<T: TimeoutManager> std::fmt::Debug for TimeoutGuard<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeoutGuard")
            .field("original_read_timeout", &self.original_read_timeout)
            .field("original_write_timeout", &self.original_write_timeout)
            .field("restored", &self.restored)
            .finish()
    }
}

impl<'a, T: TimeoutManager> TimeoutGuard<'a, T> {
    /// Create a new timeout guard with specified timeouts.
    pub fn new(
        socket: &'a mut T,
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
    ) -> Result<Self, Error> {
        let original_read_timeout = socket.get_read_timeout()?;
        let original_write_timeout = socket.get_write_timeout()?;

        if let Some(timeout) = read_timeout {
            socket.set_read_timeout(Some(timeout))?;
        }

        if let Some(timeout) = write_timeout {
            socket.set_write_timeout(Some(timeout))?;
        }

        Ok(Self {
            socket,
            original_read_timeout,
            original_write_timeout,
            restored: false,
        })
    }

    /// Restore the original timeouts.
    pub fn restore(&mut self) -> Result<(), Error> {
        if !self.restored {
            self.socket.set_read_timeout(self.original_read_timeout)?;
            self.socket.set_write_timeout(self.original_write_timeout)?;
            self.restored = true;
        }
        Ok(())
    }
}

impl<T: TimeoutManager> Drop for TimeoutGuard<'_, T> {
    fn drop(&mut self) {
        // Best effort to restore timeouts
        let _ = self.restore();
    }
}

/// Execute a function with a temporary timeout on a MutexGuard-wrapped socket.
///
/// This is a utility function for working with sockets that are protected
/// by a Mutex, which is common in the transport implementations.
pub fn with_timeout_on_guard<T, F, R>(
    guard: &mut MutexGuard<'_, T>,
    timeout: Duration,
    f: F,
) -> Result<R, Error>
where
    T: TimeoutManager,
    F: FnOnce(&mut T) -> Result<R, Error>,
{
    guard.with_read_timeout(timeout, f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn test_tcp_timeout_manager() {
        // Start a TCP server
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn a thread to accept connections
        thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            // Keep the connection open
            thread::sleep(Duration::from_secs(10));
        });

        // Connect to the server
        let mut stream = TcpStream::connect(addr).unwrap();

        // Test setting and getting timeouts
        assert_eq!(stream.get_read_timeout().unwrap(), None);

        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        assert_eq!(
            stream.get_read_timeout().unwrap(),
            Some(Duration::from_secs(5))
        );

        // Test with_read_timeout
        let result = stream.with_read_timeout(Duration::from_secs(1), |s| {
            // Verify timeout is set
            assert_eq!(s.get_read_timeout().unwrap(), Some(Duration::from_secs(1)));
            Ok(42)
        });

        assert_eq!(result.unwrap(), 42);
        // Verify timeout is restored
        assert_eq!(
            stream.get_read_timeout().unwrap(),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn test_udp_timeout_manager() {
        let mut socket = UdpSocket::bind("127.0.0.1:0").unwrap();

        // Test setting and getting timeouts
        assert_eq!(socket.get_read_timeout().unwrap(), None);

        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        assert_eq!(
            socket.get_read_timeout().unwrap(),
            Some(Duration::from_secs(3))
        );

        // Test with_read_timeout
        let result = socket.with_read_timeout(Duration::from_secs(2), |s| {
            // Verify timeout is set
            assert_eq!(s.get_read_timeout().unwrap(), Some(Duration::from_secs(2)));
            Ok("success")
        });

        assert_eq!(result.unwrap(), "success");
        // Verify timeout is restored
        assert_eq!(
            socket.get_read_timeout().unwrap(),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn test_timeout_guard() {
        let mut socket = UdpSocket::bind("127.0.0.1:0").unwrap();

        // Set initial timeouts
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();

        // Verify initial timeouts
        assert_eq!(
            socket.get_read_timeout().unwrap(),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            socket.get_write_timeout().unwrap(),
            Some(Duration::from_secs(10))
        );

        {
            let mut _guard = TimeoutGuard::new(
                &mut socket,
                Some(Duration::from_secs(1)),
                Some(Duration::from_secs(2)),
            )
            .unwrap();

            // Guard holds the mutable reference, so we can't access socket here
            // The guard will automatically restore on drop
        }

        // Verify original timeouts are restored after guard is dropped
        assert_eq!(
            socket.get_read_timeout().unwrap(),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            socket.get_write_timeout().unwrap(),
            Some(Duration::from_secs(10))
        );
    }
}
