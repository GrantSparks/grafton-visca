//! Unified camera module with Send-safe, profile-centric VISCA API.
//!
//! This module provides a single, unified camera implementation that works in both
//! async and blocking modes through the Mode trait system. All mode-specific behavior
//! is resolved at compile time for zero runtime overhead.
//!
//! The unified design ensures Send-safe futures and eliminates the complexity of
//! separate AsyncCamera/BlockingCamera types.
//!
//! Use the type aliases for cleaner syntax:
//! - `AsyncCamera<P, Tr, Exec>` for async cameras with runtime-specific executors
//! - `BlockingCamera<P, Tr>` for blocking cameras

pub mod builder;
pub mod capabilities;
pub mod controls;
pub mod movement_detection;
pub mod movement_probe;
pub mod profiles;
pub mod unified;

// Blocking-specific wrapper module
#[cfg(not(feature = "async"))]
pub mod blocking_api;

// Re-export the unified camera types with convenient aliases
pub use unified::Camera;

// Type aliases for easier usage
/// Async camera type alias for easier usage.
///
/// This type represents a camera operating in async mode with Send-safe futures.
/// It requires an async transport and executor for operation.
#[cfg(feature = "async")]
pub type AsyncCamera<P, Tr, Exec> = Camera<crate::mode::Async, P, Tr, Exec>;

/// Blocking camera type alias for unified API usage.
///
/// This type represents a camera operating in blocking mode with synchronous operations.
/// It requires a sync transport for operation. For ergonomic blocking API with direct
/// Result returns, see `blocking_api::BlockingCamera`.
#[cfg(not(feature = "async"))]
pub type UnifiedBlockingCamera<P, Tr> = Camera<crate::mode::Blocking, P, Tr, ()>;

// Re-export builder types
pub use builder::CameraBuilder;

// Re-export camera type aliases for convenience
#[cfg(all(feature = "async", feature = "rt-tokio"))]
pub use builder::async_cameras::TokioCamera;

#[cfg(all(feature = "async", feature = "rt-async-std"))]
pub use builder::async_std_cameras::AsyncStdCamera;

#[cfg(all(feature = "async", feature = "rt-smol"))]
pub use builder::smol_cameras::SmolCamera;

// Re-export movement detection types
pub use movement_probe::{MovementConfig, PanTiltPosition};

/// Internal trait that provides mode-agnostic sending capabilities.
///
/// This trait abstracts over the differences between async and blocking modes,
/// allowing control traits to have a single implementation that works for both.
pub trait CameraSend<M>
where
    M: crate::mode::Mode,
{
    /// Send a command and expect completion.
    fn send_and_complete<C>(&self, command: C) -> M::Ret<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaEncode + Send + Sync + Clone + 'static;

    /// Send a typed command and parse the response.
    fn send_and_parse<C>(&self, command: C) -> M::Ret<'_, Result<C::Response, crate::Error>>
    where
        C: crate::command::typed::ViscaCommand
            + crate::command::ViscaEncode
            + Send
            + Sync
            + Clone
            + 'static,
        C::Response: Send + 'static;

    /// Return an error immediately.
    fn error<T>(&self, error: crate::Error) -> M::Ret<'_, Result<T, crate::Error>>
    where
        T: Send + 'static;
}

// Async implementation of CameraSend
#[cfg(feature = "async")]
impl<P, Tr, Exec> CameraSend<crate::mode::Async> for Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor + Send + Sync + Clone + 'static,
{
    fn send_and_complete<C>(
        &self,
        command: C,
    ) -> <crate::mode::Async as crate::mode::Mode>::Ret<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaEncode + Send + Sync + Clone + 'static,
    {
        use crate::mode::Mode;
        let future = self.send_command(&command);
        crate::mode::Async::ret_fut(async move {
            use crate::command::response::ViscaResponse;
            match future.await? {
                ViscaResponse::Completion { .. } => Ok(()),
                ViscaResponse::Error(e) => Err(e),
                _ => Ok(()),
            }
        })
    }

    fn send_and_parse<C>(
        &self,
        command: C,
    ) -> <crate::mode::Async as crate::mode::Mode>::Ret<'_, Result<C::Response, crate::Error>>
    where
        C: crate::command::typed::ViscaCommand
            + crate::command::ViscaEncode
            + Send
            + Sync
            + Clone
            + 'static,
        C::Response: Send + 'static,
    {
        self.send_command_typed(&command)
    }

    fn error<T>(
        &self,
        error: crate::Error,
    ) -> <crate::mode::Async as crate::mode::Mode>::Ret<'_, Result<T, crate::Error>>
    where
        T: Send + 'static,
    {
        use crate::mode::Mode;
        crate::mode::Async::ret(Err(error))
    }
}

// Blocking implementation of CameraSend
#[cfg(not(feature = "async"))]
impl<P, Tr> CameraSend<crate::mode::Blocking> for Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn send_and_complete<C>(
        &self,
        command: C,
    ) -> <crate::mode::Blocking as crate::mode::Mode>::Ret<'_, Result<(), crate::Error>>
    where
        C: crate::command::ViscaEncode + Send + Sync + Clone + 'static,
    {
        #[cfg(test)]
        eprintln!("send_and_complete: entering");

        // In blocking mode, send_command already returns a Ready<Result<...>>
        // so we need to extract the value from it using pollster
        use crate::mode::BlockingFutureExt;
        #[cfg(test)]
        eprintln!("send_and_complete: calling send_command");
        let future = self.send_command(&command);
        #[cfg(test)]
        eprintln!("send_and_complete: got future, calling .block()");
        let result = future.block();
        #[cfg(test)]
        eprintln!("send_and_complete: got result from .block()");
        use crate::command::response::ViscaResponse;
        std::future::ready(match result {
            Ok(ViscaResponse::Completion { .. }) => Ok(()),
            Ok(ViscaResponse::Error(e)) => Err(e),
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        })
    }

    fn send_and_parse<C>(
        &self,
        command: C,
    ) -> <crate::mode::Blocking as crate::mode::Mode>::Ret<'_, Result<C::Response, crate::Error>>
    where
        C: crate::command::typed::ViscaCommand
            + crate::command::ViscaEncode
            + Send
            + Sync
            + Clone
            + 'static,
        C::Response: Send + 'static,
    {
        // send_command_typed already returns a Ready future in blocking mode
        // so we need to extract the value from it using .block()
        use crate::mode::BlockingFutureExt;
        std::future::ready(self.send_command_typed(&command).block())
    }

    fn error<T>(
        &self,
        error: crate::Error,
    ) -> <crate::mode::Blocking as crate::mode::Mode>::Ret<'_, Result<T, crate::Error>>
    where
        T: Send + 'static,
    {
        use crate::mode::Mode;
        crate::mode::Blocking::ret(Err(error))
    }
}
