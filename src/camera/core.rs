//! Core camera implementation that returns futures.
//!
//! This module provides CameraCore which has methods that return futures.
//! The async and blocking facades wrap this to provide their respective APIs.

use crate::{
    capabilities::ProfileMetadata,
    command::Response,
    transport::{core::Transport, visca_protocol::ViscaProtocol},
    Command, Error,
};
use core::future::Future;
use std::marker::PhantomData;

/// Core camera control interface with future-returning methods.
///
/// This struct provides the base implementation for all camera operations.
/// Methods return futures that can be either:
/// - Awaited in async contexts (via CameraAsync facade)
/// - Blocked on in sync contexts (via CameraBlocking facade)
#[derive(Debug)]
pub struct CameraCore<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    profile: PhantomData<P>,
    protocol: ViscaProtocol<T>,
}

impl<P, T> CameraCore<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Create a new camera instance with the given transport.
    pub fn new(transport: T) -> Self {
        Self {
            profile: PhantomData,
            protocol: ViscaProtocol::new(transport),
        }
    }

    /// Get a reference to the underlying transport.
    pub fn transport(&self) -> &T {
        self.protocol.inner()
    }

    /// Send a command and return a future that resolves to the response.
    pub fn send_command<'a>(
        &'a self,
        command: &'a dyn Command,
    ) -> impl Future<Output = Result<Response, Error>> + 'a {
        self.protocol.send_command(command)
    }

    /// Get the camera profile information.
    pub fn profile_info(&self) -> &'static str {
        P::MODEL_NAME
    }
}

// The individual camera methods (zoom, pan/tilt, etc.) will be added via
// extension traits that work with CameraCore, similar to the current design
// but returning futures instead of results.
