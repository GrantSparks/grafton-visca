//! Unified channel abstractions that provide consistent semantics across blocking and async modes.
//!
//! This module provides channel implementations that behave consistently whether the
//! tokio feature is enabled or not, addressing the semantic differences between
//! tokio's unbounded channels and std's bounded channels.

// Re-export the appropriate implementation based on features
#[cfg(feature = "tokio")]
mod tokio;
#[cfg(feature = "tokio")]
pub use self::tokio::*;

#[cfg(all(test, feature = "tokio"))]
mod tests {
    use super::*;

    #[test]
    fn test_unbounded_channel() {
        let (tx, mut rx) = unbounded::<i32>();

        // Should be able to send without blocking
        assert!(tx.send(42).is_ok());
        assert!(tx.send(43).is_ok());

        // Should receive in order
        assert_eq!(rx.try_recv(), Some(42));
        assert_eq!(rx.try_recv(), Some(43));
        assert_eq!(rx.try_recv(), None);
    }

    #[test]
    fn test_oneshot_channel() {
        let (tx, rx) = oneshot::<i32>();

        // Send should succeed
        assert!(tx.send(42).is_ok());

        // Blocking receive should work
        #[cfg(not(feature = "tokio"))]
        assert_eq!(rx.recv().unwrap(), 42);

        // For tokio builds, we can't test async recv in a sync test
        #[cfg(feature = "tokio")]
        drop(rx);
    }

    #[test]
    fn test_unbounded_dropped_receiver() {
        let (tx, rx) = unbounded::<i32>();
        drop(rx);

        // Send should fail when receiver is dropped
        assert!(tx.send(42).is_err());
    }

    #[test]
    fn test_oneshot_dropped_receiver() {
        let (tx, rx) = oneshot::<i32>();
        drop(rx);

        // Send should return the value when receiver is dropped
        assert_eq!(tx.send(42), Err(42));
    }
}
