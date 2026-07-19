//! Attach an existing blocking transport with `CameraBuilder`.
//!
//! Most applications should use `Connect` or `CameraConfig`. Use
//! `CameraBuilder` when your application already owns the transport.
//!
//! Run with:
//! ```sh
//! cargo run --example builder_api -- 192.168.0.110:1259
//! ```

#[cfg(not(feature = "mode-async"))]
mod support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io, net::SocketAddr, time::Duration};

    use grafton_visca::{
        camera::profiles::PtzOpticsG2,
        capabilities::Capabilities,
        transport::{BackoffStrategy, RetryConfig, Transport, TransportConfig},
        CameraBuilder,
    };

    use super::support::finish_session;

    fn transport_config() -> TransportConfig {
        TransportConfig {
            retry_config: RetryConfig {
                max_retries: 4,
                base_retry_delay: Duration::from_millis(75),
                max_retry_duration: Duration::from_secs(5),
                backoff_strategy: BackoffStrategy::Exponential,
            },
            ..TransportConfig::default()
        }
    }

    fn power_label(is_on: bool) -> &'static str {
        if is_on {
            "on"
        } else {
            "off"
        }
    }

    fn udp_endpoint() -> Result<String, io::Error> {
        let mut values = env::args().skip(1);
        let endpoint = match values.next() {
            Some(value) if value.starts_with('-') => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown option `{value}`"),
                ));
            }
            Some(endpoint) => endpoint,
            None => match env::var("VISCA_CAMERA_UDP_ADDR") {
                Ok(endpoint) => endpoint,
                Err(_) => {
                    let capabilities = Capabilities::from_profile::<PtzOpticsG2>();
                    let port = capabilities.default_udp_port.ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::Unsupported,
                            "PtzOpticsG2 profile does not declare UDP support",
                        )
                    })?;
                    format!("192.168.0.110:{port}")
                }
            },
        };

        if let Some(extra) = values.next() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "unexpected extra argument `{extra}`\nusage: cargo run --example builder_api -- [udp-address:port]"
                ),
            ));
        }
        if !has_explicit_nonzero_port(&endpoint) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "UDP endpoint must include a nonzero port (for example, 192.168.0.110:1259)",
            ));
        }

        Ok(endpoint)
    }

    fn has_explicit_nonzero_port(endpoint: &str) -> bool {
        if let Ok(address) = endpoint.parse::<SocketAddr>() {
            return address.port() != 0;
        }

        endpoint.matches(':').count() == 1
            && endpoint
                .rsplit_once(':')
                .and_then(|(_, port)| port.parse::<u16>().ok())
                .is_some_and(|port| port != 0)
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let _ = tracing_subscriber::fmt::try_init();

        let udp_endpoint = udp_endpoint()?;

        println!("CameraBuilder BYO transport example");
        println!(
            "Profile: {}",
            Capabilities::from_profile::<PtzOpticsG2>().model_name
        );
        println!("UDP endpoint: {udp_endpoint}");

        let transport = Transport::udp()
            .address(&udp_endpoint)
            .retry_config(transport_config().retry_config)
            .build_blocking()?;

        let camera = CameraBuilder::from_transport_handle(transport)
            .profile::<PtzOpticsG2>()
            .open()?;

        let power_result = camera.power().state();
        let close_result = camera.close();
        let power = finish_session(power_result, close_result)?;
        println!(
            "Connected through caller-owned transport. Power is {}.",
            power_label(power)
        );

        Ok(())
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    eprintln!("This blocking example requires no async runtime feature.");
    eprintln!("Run with: cargo run --example builder_api -- 192.168.0.110:1259");
}
