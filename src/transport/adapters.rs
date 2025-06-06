//! Adapters to help migrate from old transport traits to new unified Transport trait

use super::{Transport, TransportFuture};
use crate::{ViscaCommand, ViscaTransport};

/// Adapter that makes old `ViscaTransport` work with new Transport trait
pub struct LegacyTransportAdapter<T: ViscaTransport + Send + Sync> {
    inner: T,
}

impl<T: ViscaTransport + Send + Sync> LegacyTransportAdapter<T> {
    /// Create a new adapter wrapping an old `ViscaTransport`
    pub fn new(transport: T) -> Self {
        Self { inner: transport }
    }

    /// Get a reference to the inner transport
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner transport
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: ViscaTransport + Send + Sync + 'static> Transport for LegacyTransportAdapter<T> {
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) -> TransportFuture<'a, ()> {
        Box::pin(async move { self.inner.send_command(command) })
    }

    fn receive_response(&mut self) -> TransportFuture<'_, Vec<Vec<u8>>> {
        Box::pin(async move { self.inner.receive_response() })
    }
}

/// Extension trait to easily convert old transports to new Transport trait
pub trait IntoTransport {
    /// The type of transport this converts into
    type Transport: Transport;

    /// Convert this old transport into a new Transport
    fn into_transport(self) -> Self::Transport;
}

impl<T: ViscaTransport + Send + Sync + 'static> IntoTransport for T {
    type Transport = LegacyTransportAdapter<T>;

    fn into_transport(self) -> Self::Transport {
        LegacyTransportAdapter::new(self)
    }
}

