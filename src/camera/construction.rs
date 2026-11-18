//! Standard transport connectors used by the inherent owner constructors.
//!
//! Plans prove configured profile, policy and endpoint facts before any DNS or
//! device I/O. Opening still verifies the actual transport's descriptor and
//! any serial Address Set report, then builds its owner adapter.

use crate::camera::config::{ConnectionPlan, ConnectionTransport};
#[cfg(any(feature = "blocking", feature = "async"))]
use crate::transport::NetworkTransport;

#[cfg(feature = "blocking")]
pub(crate) fn connect_blocking(plan: ConnectionPlan) -> crate::Result<crate::blocking::Session> {
    use crate::transport::blocking::{Tcp, Udp};
    let ConnectionPlan {
        transport,
        session_config,
    } = plan;
    match transport {
        ConnectionTransport::Network(network) => match network.kind {
            NetworkTransport::Tcp => crate::blocking::Session::open_validated(
                Tcp::connect_endpoint(&network.endpoint, network.transport_config)?,
                session_config,
            ),
            NetworkTransport::Udp => crate::blocking::Session::open_validated(
                Udp::connect_endpoint(&network.endpoint, network.transport_config)?,
                session_config,
            ),
        },
        ConnectionTransport::Serial(serial_config) => {
            #[cfg(feature = "transport-serial")]
            {
                let transport = crate::transport::serial_blocking::SerialTransport::open_validated(
                    serial_config,
                )?;
                crate::blocking::Session::open_validated(transport, session_config)
            }
            #[cfg(not(feature = "transport-serial"))]
            {
                let _ = (serial_config, session_config);
                Err(crate::Error::FeatureNotSupported {
                    feature: "blocking serial transport",
                })
            }
        }
    }
}

// Keep the explicit Send future bound for downstream spawn contracts (rustc #100013).
#[cfg(feature = "async")]
#[allow(clippy::manual_async_fn)]
pub(crate) fn connect_async<R: crate::runtime::Runtime>(
    plan: ConnectionPlan,
    runtime: R,
) -> impl std::future::Future<Output = crate::Result<crate::async_session::Session>> + Send {
    async move {
        let ConnectionPlan {
            transport,
            session_config,
        } = plan;
        match transport {
            ConnectionTransport::Network(network) => match network.kind {
                NetworkTransport::Tcp => {
                    let transport = runtime
                        .connect_tcp_endpoint(network.endpoint, network.transport_config)
                        .await?;
                    crate::async_session::Session::open_validated(
                        transport,
                        session_config,
                        runtime,
                    )
                    .await
                }
                NetworkTransport::Udp => {
                    let transport = runtime
                        .connect_udp_endpoint(network.endpoint, network.transport_config)
                        .await?;
                    crate::async_session::Session::open_validated(
                        transport,
                        session_config,
                        runtime,
                    )
                    .await
                }
            },
            ConnectionTransport::Serial(serial_config) => {
                #[cfg(feature = "transport-serial-tokio")]
                {
                    let transport = runtime.connect_validated_serial(serial_config).await?;
                    crate::async_session::Session::open_validated(
                        transport,
                        session_config,
                        runtime,
                    )
                    .await
                }
                #[cfg(not(feature = "transport-serial-tokio"))]
                {
                    let _ = (serial_config, session_config, runtime);
                    Err(crate::Error::FeatureNotSupported {
                        feature: "async serial transport",
                    })
                }
            }
        }
    }
}
