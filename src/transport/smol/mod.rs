//! smol runtime transport implementations.
//!
//! This module provides transport implementations for the smol runtime.

pub(crate) mod connectors;

crate::declare_net_transport!(
    runtime = "smol",
    tcp_stream = crate::transport::smol::connectors::SmolTcpStream,
    udp_socket = smol::net::UdpSocket,
    tcp_connect = crate::transport::smol::connectors::connect_tcp,
    udp_connect = crate::transport::smol::connectors::connect_udp,
    tcp_split = clone
);

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        io::ErrorKind,
        net::TcpListener,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
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

    #[test]
    #[cfg_attr(miri, ignore = "requires a real TCP listener")]
    fn invalid_tcp_config_returns_before_a_listener_can_accept() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        listener
            .set_nonblocking(true)
            .expect("make listener nonblocking");
        let address = listener.local_addr().expect("listener address");

        let accept_thread = thread::spawn(move || -> Result<bool, std::io::Error> {
            let deadline = Instant::now() + Duration::from_millis(100);
            loop {
                match listener.accept() {
                    Ok(_) => return Ok(true),
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Ok(false);
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => return Err(error),
                }
            }
        });

        let result = smol::block_on(Tcp::connect_with_config(
            &address.to_string(),
            invalid_buffer_config(),
        ));
        let accepted = accept_thread
            .join()
            .expect("join accept observer")
            .expect("listener failed while observing connector");

        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
        assert!(
            !accepted,
            "invalid configuration must be rejected before TCP connect"
        );
    }

    #[test]
    fn invalid_udp_config_returns_before_socket_setup() {
        smol::block_on(async {
            // This is the real production connector seam. The spy replaces
            // resolver, bind, connect, and socket setup without I/O.
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
        });
    }
}
