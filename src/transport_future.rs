// Standard library imports
use std::future::Future;
use std::pin::Pin;

// Crate imports
use crate::error::Error;

/// Type alias for the future returned by async transport methods
pub type TransportFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>;
