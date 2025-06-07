//! Synchronization primitives that work in both blocking and async contexts.
//!
//! This module provides unified types for mutex and semaphore that automatically
//! select the appropriate implementation based on enabled features.

// Re-export the appropriate mutex type based on features
#[cfg(feature = "async-client")]
pub use tokio::sync::Mutex;

#[cfg(not(feature = "async-client"))]
pub use parking_lot::Mutex;

// Semaphore abstraction that works for both sync and async
#[cfg(feature = "async-client")]
mod async_semaphore {
    // Standard library imports
    use std::sync::Arc;

    // Third-party imports
    use tokio::sync::{Semaphore as TokioSemaphore, SemaphorePermit};

    pub struct Semaphore {
        inner: Arc<TokioSemaphore>,
    }

    pub struct Permit<'a> {
        _permit: SemaphorePermit<'a>,
    }

    impl Semaphore {
        pub fn new(permits: usize) -> Self {
            Self {
                inner: Arc::new(TokioSemaphore::new(permits)),
            }
        }

        pub async fn acquire(&self) -> Result<Permit<'_>, crate::ViscaError> {
            let permit = self
                .inner
                .acquire()
                .await
                .map_err(|_| crate::ViscaError::InvalidParameter("Semaphore closed".into()))?;
            Ok(Permit { _permit: permit })
        }
    }
}

#[cfg(not(feature = "async-client"))]
mod sync_semaphore {
    // Standard library imports
    use std::sync::Arc;

    // Third-party imports
    use parking_lot::{Condvar, Mutex};

    pub struct Semaphore {
        state: Arc<(Mutex<usize>, Condvar)>,
    }

    pub struct Permit<'a> {
        semaphore: &'a Semaphore,
    }

    impl Semaphore {
        pub fn new(permits: usize) -> Self {
            Self {
                state: Arc::new((Mutex::new(permits), Condvar::new())),
            }
        }

        pub fn acquire(&self) -> Permit<'_> {
            let (lock, cvar) = &*self.state;
            let mut count = lock.lock();

            // Wait until a permit is available
            while *count == 0 {
                cvar.wait(&mut count);
            }

            *count -= 1;
            Permit { semaphore: self }
        }
    }

    impl Drop for Permit<'_> {
        fn drop(&mut self) {
            let (lock, cvar) = &*self.semaphore.state;
            let mut count = lock.lock();
            *count += 1;
            drop(count);
            let _ = cvar.notify_one();
        }
    }
}

#[cfg(feature = "async-client")]
pub use async_semaphore::{Permit, Semaphore};

#[cfg(not(feature = "async-client"))]
pub use sync_semaphore::{Permit, Semaphore};

// Helper trait to unify acquire behavior
pub trait SemaphoreExt {
    type Permit<'a>
    where
        Self: 'a;

    #[cfg(feature = "async-client")]
    async fn acquire_permit(&self) -> Result<Self::Permit<'_>, crate::ViscaError>;

    #[cfg(not(feature = "async-client"))]
    fn acquire_permit(&self) -> Self::Permit<'_>;
}

impl SemaphoreExt for Semaphore {
    type Permit<'a>
        = Permit<'a>
    where
        Self: 'a;

    #[cfg(feature = "async-client")]
    async fn acquire_permit(&self) -> Result<Self::Permit<'_>, crate::ViscaError> {
        self.acquire().await
    }

    #[cfg(not(feature = "async-client"))]
    fn acquire_permit(&self) -> Self::Permit<'_> {
        self.acquire()
    }
}
