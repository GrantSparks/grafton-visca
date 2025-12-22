//! Camera module with Send-safe, profile-centric VISCA API.
//!
//! This module provides a unified Camera<M, P, Tr, Exec> type that works in both
//! async and blocking modes through the Mode trait system. All mode-specific behavior
//! is resolved at compile time for zero runtime overhead.
//!
//! The unified design ensures Send-safe futures and consistent API across both modes.
//!
//! # Type Parameters
//! - `M`: Mode (Async or Blocking)
//! - `P`: Camera profile implementing the Profile trait
//! - `Tr`: Transport type (TCP, UDP, or Serial)
//! - `Exec`: Runtime executor (for async mode)

pub mod accessors;
pub mod builder;
pub mod camera_impl;
pub mod capabilities;
pub mod config;
pub mod controls;
pub mod convenience;
pub mod inflight;
pub mod movement;
pub mod profiles;
pub mod session;

// Blocking-specific wrapper module
#[cfg(not(feature = "mode-async"))]
pub mod blocking_api;

// Re-export the camera type (the actual implementation)
pub use camera_impl::Camera;

// Re-export CommandId for easy access
pub use inflight::CommandId;

// Re-export new API types
pub use config::{CameraConfig, TransportOptions};
pub use session::CameraSession;

// Re-export convenience methods for quick connection
pub use convenience::{Connect, ConnectBuilder};

// Type aliases for easier usage
/// Async camera type alias for easier usage.
///
/// This type represents a camera operating in async mode with Send-safe futures.
/// It requires an async transport and executor for operation.
#[cfg(feature = "mode-async")]
pub type AsyncCamera<P, Tr, Exec> = Camera<crate::mode::Async, P, Tr, Exec>;

/// Blocking camera type alias for unified API usage.
///
/// This type represents a camera operating in blocking mode with synchronous operations.
/// It requires a sync transport for operation. For ergonomic blocking API with direct
/// Result returns, see `blocking_api::BlockingCamera`.
#[cfg(not(feature = "mode-async"))]
pub type BlockingCamera<P, Tr> = Camera<crate::mode::Blocking, P, Tr, ()>;

// Re-export builder types
pub use builder::CameraBuilder;

// Re-export movement detection types
pub use movement::{AwaitConfig, Axes, MovementTolerance, PanTiltPosition};

/// Internal trait that provides mode-agnostic VISCA client capabilities.
///
/// This trait abstracts over the differences between async and blocking modes,
/// allowing control traits to have a single implementation that works for both.
pub trait ViscaClient<M>
where
    M: crate::mode::Mode,
{
    /// Execute a command and expect completion.
    fn execute<C>(&self, command: C) -> M::Fut<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static;

    /// Query with a typed command and parse the response.
    fn query<C>(
        &self,
        command: C,
    ) -> M::Fut<'_, Result<<C as crate::command::typed::ResponseParser>::Response, crate::Error>>
    where
        C: crate::command::typed::ResponseParser
            + crate::command::ViscaCommand
            + Send
            + Sync
            + Clone
            + std::fmt::Debug
            + 'static,
        <C as crate::command::typed::ResponseParser>::Response: Send + 'static;

    /// Return an error immediately.
    fn error<T>(&self, error: crate::Error) -> M::Fut<'_, Result<T, crate::Error>>
    where
        T: Send + 'static;

    /// Get access to the state cache for write-only properties.
    fn cache(&self) -> &crate::cache::StateCache;

    /// Execute a command and update the cache on success.
    ///
    /// This method executes the command and, if successful, calls the provided
    /// function to update the cache. This is used for write-only properties
    /// that need to be tracked in the state cache.
    fn execute_updating_cache<C, F>(
        &self,
        command: C,
        update_fn: F,
    ) -> M::Fut<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        F: FnOnce(&crate::cache::StateCache) + Send + 'static;
}

// Async implementation of ViscaClient
#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> ViscaClient<crate::mode::Async> for Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor + Send + Sync + Clone + 'static,
{
    fn execute<C>(
        &self,
        command: C,
    ) -> <crate::mode::Async as crate::mode::Mode>::Fut<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
    {
        use crate::mode::Mode;
        let future = self.send_command(&command);
        crate::mode::Async::from_future(async move {
            use crate::command::response::Response;
            match future.await? {
                Response::Completion { .. } => Ok(()),
                Response::Error(e) => Err(e),
                _ => Ok(()),
            }
        })
    }

    fn query<C>(
        &self,
        command: C,
    ) -> <crate::mode::Async as crate::mode::Mode>::Fut<
        '_,
        Result<<C as crate::command::typed::ResponseParser>::Response, crate::Error>,
    >
    where
        C: crate::command::typed::ResponseParser
            + crate::command::ViscaCommand
            + Send
            + Sync
            + Clone
            + std::fmt::Debug
            + 'static,
        <C as crate::command::typed::ResponseParser>::Response: Send + 'static,
    {
        self.send_command_typed(&command)
    }

    fn error<T>(
        &self,
        error: crate::Error,
    ) -> <crate::mode::Async as crate::mode::Mode>::Fut<'_, Result<T, crate::Error>>
    where
        T: Send + 'static,
    {
        use crate::mode::Mode;
        crate::mode::Async::ready(Err(error))
    }

    fn cache(&self) -> &crate::cache::StateCache {
        self.state_cache()
    }

    fn execute_updating_cache<C, F>(
        &self,
        command: C,
        update_fn: F,
    ) -> <crate::mode::Async as crate::mode::Mode>::Fut<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        F: FnOnce(&crate::cache::StateCache) + Send + 'static,
    {
        use crate::mode::Mode;
        let future = self.send_command(&command);
        // Clone the cache for the async block (Arc<Mutex<>> is Clone)
        let cache = self.state_cache().clone();
        crate::mode::Async::from_future(async move {
            use crate::command::response::Response;
            match future.await? {
                Response::Completion { .. } => {
                    update_fn(&cache);
                    Ok(())
                }
                Response::Error(e) => Err(e),
                _ => {
                    update_fn(&cache);
                    Ok(())
                }
            }
        })
    }
}

// Blocking implementation of ViscaClient
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> ViscaClient<crate::mode::Blocking> for Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    fn execute<C>(
        &self,
        command: C,
    ) -> <crate::mode::Blocking as crate::mode::Mode>::Fut<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
    {
        // In blocking mode, send_command already returns a Ready<Result<...>>
        // so we need to extract the value from it using pollster
        use crate::mode::BlockingFutureExt;
        let future = self.send_command(&command);
        let result = future.block();
        use crate::command::response::Response;
        std::future::ready(match result {
            Ok(Response::Completion { .. }) => Ok(()),
            Ok(Response::Error(e)) => Err(e),
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        })
    }

    fn query<C>(
        &self,
        command: C,
    ) -> <crate::mode::Blocking as crate::mode::Mode>::Fut<
        '_,
        Result<<C as crate::command::typed::ResponseParser>::Response, crate::Error>,
    >
    where
        C: crate::command::typed::ResponseParser
            + crate::command::ViscaCommand
            + Send
            + Sync
            + Clone
            + std::fmt::Debug
            + 'static,
        <C as crate::command::typed::ResponseParser>::Response: Send + 'static,
    {
        // send_command_typed already returns a Ready future in blocking mode
        // so we need to extract the value from it using .block()
        use crate::mode::BlockingFutureExt;
        std::future::ready(self.send_command_typed(&command).block())
    }

    fn error<T>(
        &self,
        error: crate::Error,
    ) -> <crate::mode::Blocking as crate::mode::Mode>::Fut<'_, Result<T, crate::Error>>
    where
        T: Send + 'static,
    {
        use crate::mode::Mode;
        crate::mode::Blocking::ready(Err(error))
    }

    fn cache(&self) -> &crate::cache::StateCache {
        self.state_cache()
    }

    fn execute_updating_cache<C, F>(
        &self,
        command: C,
        update_fn: F,
    ) -> <crate::mode::Blocking as crate::mode::Mode>::Fut<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        F: FnOnce(&crate::cache::StateCache) + Send + 'static,
    {
        use crate::mode::BlockingFutureExt;
        let future = self.send_command(&command);
        let result = future.block();
        use crate::command::response::Response;
        let cache = self.state_cache();
        std::future::ready(match result {
            Ok(Response::Completion { .. }) => {
                update_fn(cache);
                Ok(())
            }
            Ok(Response::Error(e)) => Err(e),
            Ok(_) => {
                update_fn(cache);
                Ok(())
            }
            Err(e) => Err(e),
        })
    }
}
