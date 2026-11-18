//! Standard transport connectors used by the inherent owner constructors.
//!
//! Plans prove configured profile, policy and endpoint facts before any DNS or
//! device I/O. Opening still verifies the actual transport's descriptor and
//! any serial Address Set report, then builds its owner adapter.

use crate::camera::config::{ConnectionPlan, ConnectionTransport, NetworkConnector};

#[cfg(feature = "blocking")]
pub(crate) fn connect_blocking(plan: ConnectionPlan) -> crate::Result<crate::blocking::Session> {
    use crate::transport::blocking::{Tcp, Udp};
    let ConnectionPlan {
        transport,
        session_config,
    } = plan;
    match transport {
        ConnectionTransport::Network(network) => match network.connector {
            NetworkConnector::Tcp(options) => crate::blocking::Session::open_validated(
                Tcp::connect_endpoint(&network.endpoint, options, network.io_bounds)?,
                session_config,
            ),
            NetworkConnector::Udp(options) => crate::blocking::Session::open_validated(
                Udp::connect_endpoint(&network.endpoint, options, network.io_bounds)?,
                session_config,
            ),
        },
        ConnectionTransport::Serial { config, io_bounds } => {
            #[cfg(feature = "transport-serial")]
            {
                let transport =
                    crate::transport::serial_blocking::SerialTransport::open(config, io_bounds)?;
                crate::blocking::Session::open_validated(transport, session_config)
            }
            #[cfg(not(feature = "transport-serial"))]
            {
                let _ = (config, io_bounds, session_config);
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
            ConnectionTransport::Network(network) => match network.connector {
                NetworkConnector::Tcp(options) => {
                    let transport = runtime
                        .connect_tcp(network.endpoint, options, network.io_bounds)
                        .await?;
                    crate::async_session::Session::open_validated(
                        transport,
                        session_config,
                        runtime,
                    )
                    .await
                }
                NetworkConnector::Udp(options) => {
                    let transport = runtime
                        .connect_udp(network.endpoint, options, network.io_bounds)
                        .await?;
                    crate::async_session::Session::open_validated(
                        transport,
                        session_config,
                        runtime,
                    )
                    .await
                }
            },
            ConnectionTransport::Serial { config, io_bounds } => {
                #[cfg(feature = "transport-serial-tokio")]
                {
                    let transport = runtime.connect_serial(config, io_bounds).await?;
                    crate::async_session::Session::open_validated(
                        transport,
                        session_config,
                        runtime,
                    )
                    .await
                }
                #[cfg(not(feature = "transport-serial-tokio"))]
                {
                    let _ = (config, io_bounds, session_config, runtime);
                    Err(crate::Error::FeatureNotSupported {
                        feature: "async serial transport",
                    })
                }
            }
        }
    }
}
