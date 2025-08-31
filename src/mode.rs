//! Mode trait system for unified async/blocking API.
//!
//! This module provides the Mode trait that enables a single API surface
//! to work in both blocking and async modes through type-state parameters.

use core::{
    future::{ready, Ready},
    pin::Pin,
};

use std::future::Future;

/// Mode trait that abstracts over async and blocking execution modes.
///
/// This trait uses GATs (Generic Associated Types) to allow the same API
/// to work in both async and blocking contexts through type parameters.
pub trait Mode {
    /// The return type for operations in this mode.
    ///
    /// For async mode, this will be a boxed Future for flexibility.
    /// For blocking mode, this will be a `Ready<T>` (immediate result).
    type Ret<T>: Future<Output = T> + Send
    where
        T: Send;

    /// Create a return value from a result.
    fn ret<T>(result: T) -> Self::Ret<T>
    where
        T: Send + 'static;

    /// Create a return value from a future.
    fn ret_fut<F, T>(future: F) -> Self::Ret<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static;

    /// Send a command and return the raw ViscaResponse.
    fn send_command<C, P, Tr, Exec>(
        camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        command: &C,
    ) -> Self::Ret<Result<crate::command::response::ViscaResponse, crate::Error>>
    where
        C: crate::command::ViscaEncode + Send + Sync + Clone + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync,
        Self: Sized;

    /// Send a typed command and return the parsed response.
    fn send_command_typed<C, P, Tr, Exec>(
        camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        command: &C,
    ) -> Self::Ret<Result<C::Response, crate::Error>>
    where
        C: crate::command::typed::ViscaCommand
            + crate::command::ViscaEncode
            + Send
            + Sync
            + Clone
            + 'static,
        C::Response: Send + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync,
        Self: Sized;
}

/// Zero-sized type representing async execution mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Async;

/// Zero-sized type representing blocking execution mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Blocking;

impl Mode for Async {
    type Ret<T>
        = Pin<Box<dyn Future<Output = T> + Send>>
    where
        T: Send;

    fn ret<T>(result: T) -> Self::Ret<T>
    where
        T: Send + 'static,
    {
        Box::pin(ready(result))
    }

    fn ret_fut<F, T>(future: F) -> Self::Ret<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        Box::pin(future)
    }

    #[cfg(feature = "async")]
    fn send_command<C, P, Tr, Exec>(
        camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        command: &C,
    ) -> Self::Ret<Result<crate::command::response::ViscaResponse, crate::Error>>
    where
        C: crate::command::ViscaEncode + Send + Sync + Clone + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync,
        Self: Sized,
    {
        use crate::command::bytes::VISCA_TERMINATOR;

        // Get the runtime handle for async command execution
        match camera.runtime_handle() {
            Some(runtime_handle) => {
                let runtime_handle = runtime_handle.clone();
                let camera_id = camera.camera_id();
                let command = command.clone(); // Clone to avoid lifetime issues
                                               // Encode the command into a buffer first
                let mut buffer = [0u8; 64];
                match command.encode_into(camera_id, &mut buffer) {
                    Ok(size) => {
                        let cmd_bytes = &buffer[..size];

                        // Add VISCA terminator if not present
                        let mut cmd_vec = cmd_bytes.to_vec();
                        if !cmd_vec.ends_with(&[VISCA_TERMINATOR]) {
                            cmd_vec.push(VISCA_TERMINATOR);
                        }

                        // Determine if this is an inquiry based on the typed response
                        let is_inquiry = command.response_type().is_some();

                        // Apply envelope and get pre-framed bytes
                        let envelope = camera.envelope();
                        let envelope_buffer_manager = camera.envelope_buffer_manager();
                        let framed_bytes =
                            envelope.frame_command(&cmd_vec, is_inquiry, envelope_buffer_manager);

                        // Create an async block that returns the raw response
                        Box::pin(async move {
                            // Send pre-framed bytes to runtime with proper categorization
                            if is_inquiry {
                                runtime_handle
                                    .send_inquiry_framed(
                                        &framed_bytes,
                                        camera_id,
                                        command.response_type(),
                                    )
                                    .await
                            } else {
                                let category = C::TIMEOUT_CATEGORY;
                                runtime_handle
                                    .send_command_framed(&framed_bytes, camera_id, None, category)
                                    .await
                            }
                        })
                    }
                    Err(e) => Box::pin(ready(Err(e))),
                }
            }
            None => {
                // No runtime handle available - return error
                Box::pin(ready(Err(crate::Error::InvalidState(
                    "No runtime handle available for async command execution".into(),
                ))))
            }
        }
    }

    #[cfg(not(feature = "async"))]
    fn send_command<C, P, Tr, Exec>(
        _camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        _command: &C,
    ) -> Self::Ret<Result<crate::command::response::ViscaResponse, crate::Error>>
    where
        C: crate::command::ViscaEncode + Send + Sync + Clone + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync,
        Self: Sized,
    {
        // Async mode is not available without the async feature
        Box::pin(ready(Err(crate::Error::FeatureNotSupported {
            feature: "async command execution requires the 'async' feature",
        })))
    }

    #[cfg(feature = "async")]
    fn send_command_typed<C, P, Tr, Exec>(
        camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        command: &C,
    ) -> Self::Ret<Result<C::Response, crate::Error>>
    where
        C: crate::command::typed::ViscaCommand
            + crate::command::ViscaEncode
            + Send
            + Sync
            + Clone
            + 'static,
        C::Response: Send + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync,
        Self: Sized,
    {
        // Delegate to send_command and parse response
        let response_future = Self::send_command(camera, command);
        Box::pin(async move {
            let response = response_future.await?;
            C::from_response(response)
        })
    }

    #[cfg(not(feature = "async"))]
    fn send_command_typed<C, P, Tr, Exec>(
        _camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        _command: &C,
    ) -> Self::Ret<Result<C::Response, crate::Error>>
    where
        C: crate::command::typed::ViscaCommand
            + crate::command::ViscaEncode
            + Send
            + Sync
            + Clone
            + 'static,
        C::Response: Send + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync,
        Self: Sized,
    {
        // Async mode is not available without the async feature
        Box::pin(ready(Err(crate::Error::FeatureNotSupported {
            feature: "async typed command execution requires the 'async' feature",
        })))
    }
}

impl Mode for Blocking {
    type Ret<T>
        = Ready<T>
    where
        T: Send;

    fn ret<T>(result: T) -> Self::Ret<T>
    where
        T: Send + 'static,
    {
        ready(result)
    }

    fn ret_fut<F, T>(_future: F) -> Self::Ret<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        // For blocking mode, futures should be avoided.
        unreachable!("Blocking mode should not use futures directly - use Mode::ret() instead")
    }

    #[cfg(not(feature = "async"))]
    fn send_command<C, P, Tr, Exec>(
        camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        command: &C,
    ) -> Self::Ret<Result<crate::command::response::ViscaResponse, crate::Error>>
    where
        C: crate::command::ViscaEncode + Send + Sync + Clone + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync, // Keep Sync requirement for blocking mode 
        Self: Sized,
    {
        use crate::command::bytes::VISCA_TERMINATOR;
        use crate::command::response::ViscaResponse;

        // Get access to the transport
        match camera.transport() {
            Some(transport_cell) => {
                // Encode command bytes using ViscaEncode
                let mut buffer = [0u8; 64];
                match command.encode_into(camera.camera_id(), &mut buffer) {
                    Ok(size) => {
                        let cmd_bytes = &buffer[..size];

                        // Add VISCA terminator if not present
                        let mut cmd_vec = cmd_bytes.to_vec();
                        if !cmd_vec.ends_with(&[VISCA_TERMINATOR]) {
                            cmd_vec.push(VISCA_TERMINATOR);
                        }

                        // Determine if this is an inquiry based on the typed response
                        let is_inquiry = command.response_type().is_some();

                        // Apply envelope and send command
                        let envelope = camera.envelope();
                        let envelope_buffer_manager = camera.envelope_buffer_manager();
                        let request =
                            envelope.frame_command(&cmd_vec, is_inquiry, envelope_buffer_manager);

                        // Send command through transport
                        let mut transport = transport_cell.borrow_mut();
                        if let Err(e) = transport.send(&request) {
                            return ready(Err(e));
                        }

                        // Handle response based on command type
                        let timeout_config = camera.timeout_config();

                        if !is_inquiry {
                            // For non-inquiry commands, handle ACK/Completion sequence

                            // Read first response (should be ACK or error) with ACK timeout
                            match transport.recv_with_timeout(timeout_config.ack_timeout) {
                                Ok(first_response_bytes) => {
                                    match envelope.extract_response(&first_response_bytes[..]) {
                                        Ok(first_visca) => {
                                            match ViscaResponse::parse(&first_visca[..]) {
                                                Ok(ViscaResponse::Error(e)) => ready(Err(e)),
                                                Ok(ViscaResponse::CmdAck) => {
                                                    // Got ACK, now wait for completion
                                                    let completion_timeout = timeout_config
                                                        .get_timeout(C::TIMEOUT_CATEGORY);
                                                    match transport
                                                        .recv_with_timeout(completion_timeout)
                                                    {
                                                        Ok(second_response_bytes) => match envelope
                                                            .extract_response(
                                                                &second_response_bytes[..],
                                                            ) {
                                                            Ok(second_visca) => {
                                                                match ViscaResponse::parse(
                                                                    &second_visca[..],
                                                                ) {
                                                                    Ok(response) => {
                                                                        ready(Ok(response))
                                                                    }
                                                                    Err(e) => ready(Err(e)),
                                                                }
                                                            }
                                                            Err(e) => ready(Err(e)),
                                                        },
                                                        Err(e) => ready(Err(e)),
                                                    }
                                                }
                                                Ok(response) => ready(Ok(response)), // Some cameras skip ACK
                                                Err(e) => ready(Err(e)),
                                            }
                                        }
                                        Err(e) => ready(Err(e)),
                                    }
                                }
                                Err(e) => ready(Err(e)),
                            }
                        } else {
                            // For inquiry commands, just read one response with quick timeout
                            let quick_timeout = timeout_config.get_timeout(C::TIMEOUT_CATEGORY);
                            match transport.recv_with_timeout(quick_timeout) {
                                Ok(response_bytes) => {
                                    match envelope.extract_response(&response_bytes[..]) {
                                        Ok(visca) => match ViscaResponse::parse(&visca[..]) {
                                            Ok(response) => ready(Ok(response)),
                                            Err(e) => ready(Err(e)),
                                        },
                                        Err(e) => ready(Err(e)),
                                    }
                                }
                                Err(e) => ready(Err(e)),
                            }
                        }
                    }
                    Err(e) => ready(Err(e)),
                }
            }
            None => {
                // No transport available - return error
                ready(Err(crate::Error::InvalidState(
                    "No transport available for blocking command execution".into(),
                )))
            }
        }
    }

    #[cfg(feature = "async")]
    fn send_command<C, P, Tr, Exec>(
        _camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        _command: &C,
    ) -> Self::Ret<Result<crate::command::response::ViscaResponse, crate::Error>>
    where
        C: crate::command::ViscaEncode + Send + Sync + Clone + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync,
        Self: Sized,
    {
        // When async feature is enabled, blocking mode still works but currently uses validation only
        // This would need SyncTransport integration when blocking operations are needed in async mode
        // For now, return a feature not supported error
        ready(Err(crate::Error::FeatureNotSupported {
            feature: "blocking command execution in async mode not yet implemented",
        }))
    }

    #[cfg(not(feature = "async"))]
    fn send_command_typed<C, P, Tr, Exec>(
        camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        command: &C,
    ) -> Self::Ret<Result<C::Response, crate::Error>>
    where
        C: crate::command::typed::ViscaCommand
            + crate::command::ViscaEncode
            + Send
            + Sync
            + Clone
            + 'static,
        C::Response: Send + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync, // Keep Sync requirement for blocking mode
        Self: Sized,
    {
        // Delegate to send_command and parse response
        let response = Self::send_command(camera, command);
        match response.into_inner() {
            Ok(visca_response) => ready(C::from_response(visca_response)),
            Err(e) => ready(Err(e)),
        }
    }

    #[cfg(feature = "async")]
    fn send_command_typed<C, P, Tr, Exec>(
        _camera: &crate::camera::unified::Camera<Self, P, Tr, Exec>,
        _command: &C,
    ) -> Self::Ret<Result<C::Response, crate::Error>>
    where
        C: crate::command::typed::ViscaCommand
            + crate::command::ViscaEncode
            + Send
            + Sync
            + Clone
            + 'static,
        C::Response: Send + 'static,
        P: crate::capabilities::Profile + Default,
        Tr: Send + Sync,
        Self: Sized,
    {
        // When async feature is enabled, blocking mode still works but currently uses validation only
        // This would need SyncTransport integration when blocking operations are needed in async mode
        // For now, return a feature not supported error
        ready(Err(crate::Error::FeatureNotSupported {
            feature: "blocking typed command execution in async mode not yet implemented",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_markers_are_zero_sized() {
        use std::mem::size_of;
        assert_eq!(size_of::<Async>(), 0);
        assert_eq!(size_of::<Blocking>(), 0);
    }

    #[tokio::test]
    async fn test_async_mode() {
        let result = Async::ret(42).await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_blocking_mode() {
        let result = Blocking::ret("test").await;
        assert_eq!(result, "test");
    }
}
