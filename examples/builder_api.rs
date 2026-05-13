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
mod blocking {
    use std::{env, time::Duration};

    use grafton_visca::{
        camera::profiles::PtzOpticsG2,
        transport::{BackoffStrategy, RetryConfig, Transport, TransportConfig},
        CameraBuilder,
    };

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

    pub fn main() -> grafton_visca::Result<()> {
        let _ = tracing_subscriber::fmt::try_init();

        let udp_endpoint = env::args()
            .nth(1)
            .or_else(|| env::var("VISCA_CAMERA_UDP_ADDR").ok())
            .unwrap_or_else(|| "192.168.0.110:1259".to_string());

        println!("CameraBuilder BYO transport example");
        println!("UDP endpoint: {udp_endpoint}");

        let transport = Transport::udp()
            .address(&udp_endpoint)
            .retry_config(transport_config().retry_config)
            .build_blocking()?;

        let camera = CameraBuilder::from_transport_handle(transport)
            .profile::<PtzOpticsG2>()
            .open()?;

        let power = camera.power().state()?;
        println!(
            "Connected through caller-owned transport. Power is {}.",
            power_label(power)
        );
        camera.close()?;

        Ok(())
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() -> grafton_visca::Result<()> {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    eprintln!("This blocking example requires no async runtime feature.");
    eprintln!("Run with: cargo run --example builder_api -- 192.168.0.110:1259");
}
