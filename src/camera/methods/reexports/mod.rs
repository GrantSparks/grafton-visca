//! Re-export method traits based on features.

#[cfg(feature = "async")]
mod async_exports;

mod blocking_exports;

// Re-export everything from the appropriate module
#[cfg(feature = "async")]
pub use async_exports::*;

pub use blocking_exports::*;
