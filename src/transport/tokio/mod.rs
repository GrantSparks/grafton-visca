//! Tokio transport implementations for VISCA communication.
//!
//! Tokio contributes only its I/O primitives; the TCP and UDP transports are
//! the generic implementations in `async_tcp` and `async_udp`.

pub(crate) mod connectors;
#[cfg(feature = "transport-serial-tokio")]
pub mod serial;

/// TCP transport over Tokio.
pub mod tcp {
    /// TCP transport for async VISCA communication using Tokio.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use grafton_visca::runtime_adapters::tokio::TcpTransport;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = TcpTransport::connect("192.168.1.100:5678").await?;
    /// let (mut reader, mut writer) = transport.split();
    /// # Ok(())
    /// # }
    /// ```
    pub type Tcp =
        crate::transport::async_tcp::Tcp<crate::transport::tokio::connectors::TokioTcpStream>;
}

/// UDP transport over Tokio.
pub mod udp {
    /// UDP transport for async VISCA communication using Tokio.
    pub type Udp = crate::transport::async_udp::Udp<tokio::net::UdpSocket>;
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    use super::{tcp::Tcp, udp::Udp};
    use crate::{
        transport::{BufferConfig, TransportConfig},
        Error,
    };

    fn invalid_buffer_config() -> TransportConfig {
        TransportConfig {
            buffer_config: BufferConfig {
                recv_buffer_size: 65,
                max_buffer_size: 64,
            },
            ..TransportConfig::default()
        }
    }

    fn assert_invalid_buffer_error<T>(result: Result<T, Error>) {
        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
    }

    #[tokio::test]
    async fn invalid_tcp_config_returns_before_a_listener_can_accept() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener.local_addr().expect("listener address").to_string();

        let result = Tcp::connect_with_config(&address, invalid_buffer_config()).await;
        let accepted = tokio::time::timeout(Duration::from_millis(100), listener.accept()).await;

        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
        assert!(
            accepted.is_err(),
            "invalid configuration must be rejected before TCP connect"
        );
    }

    #[tokio::test]
    async fn invalid_udp_config_returns_before_socket_setup() {
        // This is the real production connector seam. The spy replaces DNS,
        // bind, connect, and socket setup without relying on any of them.
        let setup_calls = Arc::new(AtomicUsize::new(0));
        let spy_calls = Arc::clone(&setup_calls);
        let result =
            Udp::preflight_udp_setup("127.0.0.1:9", invalid_buffer_config(), move |_, _| {
                spy_calls.fetch_add(1, Ordering::SeqCst);
                std::future::ready(Ok(()))
            })
            .await;

        assert_invalid_buffer_error(result);
        assert_eq!(
            setup_calls.load(Ordering::SeqCst),
            0,
            "invalid buffer configuration must not reach the UDP connector"
        );

        // The malformed endpoint makes the buffer error's precedence over
        // address parsing explicit.
        assert_invalid_buffer_error(
            Udp::preflight_udp_setup("[::1", invalid_buffer_config(), |_, _| {
                std::future::ready(Ok(()))
            })
            .await,
        );
    }
}
