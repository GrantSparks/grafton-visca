//! Standard transport connectors used by the inherent owner constructors.
//!
//! Callers resolve and validate profile, policy and endpoint facts before
//! entering these connector functions. Each opened transport is moved directly
//! into one existing owner construction path.

use crate::{
    camera::TransportKind, session_config::ValidatedSessionConfig, transport::TransportConfig,
    Error,
};

#[cfg(feature = "blocking")]
pub(crate) fn connect_blocking(
    kind: TransportKind,
    endpoint: &str,
    transport_config: TransportConfig,
    session_config: ValidatedSessionConfig,
) -> crate::Result<crate::blocking::Session> {
    use crate::transport::blocking::{Tcp, Udp};
    match kind {
        TransportKind::Tcp => crate::blocking::Session::open_validated(
            Tcp::connect_with_config(endpoint, transport_config)?,
            session_config,
        ),
        TransportKind::Udp => crate::blocking::Session::open_validated(
            Udp::connect_with_config(endpoint, transport_config)?,
            session_config,
        ),
        TransportKind::Serial | TransportKind::Custom => Err(Error::InvalidState {
            reason: "network connection plan requires TCP or UDP".into(),
        }),
    }
}

#[cfg(all(feature = "blocking", feature = "transport-serial"))]
pub(crate) fn connect_blocking_serial(
    serial_config: crate::transport::SerialConfig,
    session_config: ValidatedSessionConfig,
) -> crate::Result<crate::blocking::Session> {
    let transport = crate::transport::serial_blocking::SerialTransport::new(serial_config)?;
    crate::blocking::Session::open_validated(transport, session_config)
}

// The explicit Send bound avoids rustc #100013 when a runtime's transport
// future is projected through the public inherent connector. An async fn
// here loses that bound at downstream Send + 'static assertions.
#[cfg(feature = "async")]
#[allow(clippy::manual_async_fn)]
pub(crate) fn connect_async<R: crate::runtime::Runtime>(
    kind: TransportKind,
    endpoint: &str,
    transport_config: TransportConfig,
    session_config: ValidatedSessionConfig,
    runtime: R,
) -> impl std::future::Future<Output = crate::Result<crate::async_session::Session>> + Send + '_ {
    async move {
        match kind {
            TransportKind::Tcp => {
                let transport = runtime.connect_tcp(endpoint, transport_config).await?;
                crate::async_session::Session::open_validated(transport, session_config, runtime)
                    .await
            }
            TransportKind::Udp => {
                let transport = runtime.connect_udp(endpoint, transport_config).await?;
                crate::async_session::Session::open_validated(transport, session_config, runtime)
                    .await
            }
            TransportKind::Serial | TransportKind::Custom => Err(Error::InvalidState {
                reason: "network connection plan requires TCP or UDP".into(),
            }),
        }
    }
}

#[cfg(all(feature = "async", feature = "transport-serial-tokio"))]
#[allow(clippy::manual_async_fn)]
pub(crate) fn connect_async_serial<R: crate::runtime::Runtime>(
    serial_config: crate::transport::SerialConfig,
    session_config: ValidatedSessionConfig,
    runtime: R,
) -> impl std::future::Future<Output = crate::Result<crate::async_session::Session>> + Send {
    async move {
        let transport = runtime.connect_serial(serial_config).await?;
        crate::async_session::Session::open_validated(transport, session_config, runtime).await
    }
}
