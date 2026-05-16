//! Configured blocking camera connection.
//!
//! Use `Connect` for the shortest path. Use `CameraConfig` when application code
//! needs explicit transport policy such as timeouts, retries, queue depth, or TCP
//! keepalive.
//!
//! Run with:
//! ```sh
//! cargo run --example transport_builder_demo -- 192.168.0.110
//! cargo run --example transport_builder_demo -- 192.168.0.110 --udp
//! ```

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, time::Duration};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, CameraConfig},
        transport::{BackoffStrategy, RetryConfig, TcpKeepaliveConfig, TransportConfig},
    };

    #[derive(Debug, Clone, Copy)]
    enum TransportKind {
        Tcp,
        Udp,
    }

    impl TransportKind {
        fn label(self) -> &'static str {
            match self {
                Self::Tcp => "TCP",
                Self::Udp => "UDP",
            }
        }
    }

    #[derive(Debug)]
    struct Args {
        address: String,
        transport: TransportKind,
    }

    impl Args {
        fn parse() -> Self {
            let mut address = None;
            let mut transport = TransportKind::Tcp;

            for arg in env::args().skip(1) {
                match arg.as_str() {
                    "--udp" => transport = TransportKind::Udp,
                    "--tcp" => transport = TransportKind::Tcp,
                    _ if address.is_none() => address = Some(arg),
                    _ => {}
                }
            }

            Self {
                address: address
                    .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
                    .unwrap_or_else(|| "192.168.0.110".to_string()),
                transport,
            }
        }
    }

    fn transport_config(kind: TransportKind) -> TransportConfig {
        TransportConfig {
            connect_timeout: Duration::from_secs(3),
            read_timeout: Duration::from_secs(2),
            write_timeout: Duration::from_secs(2),
            retry_config: RetryConfig {
                max_retries: 4,
                base_retry_delay: Duration::from_millis(100),
                max_retry_duration: Duration::from_secs(5),
                backoff_strategy: BackoffStrategy::Exponential,
            },
            tcp_keepalive: match kind {
                TransportKind::Tcp => Some(TcpKeepaliveConfig::new(Duration::from_secs(30))),
                TransportKind::Udp => None,
            },
            ..TransportConfig::default()
        }
    }

    fn camera_config(args: &Args) -> CameraConfig<PtzOpticsG2> {
        match args.transport {
            TransportKind::Tcp => CameraConfig::<PtzOpticsG2>::tcp(&args.address),
            TransportKind::Udp => CameraConfig::<PtzOpticsG2>::udp(&args.address),
        }
        .transport_config(transport_config(args.transport))
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

        let args = Args::parse();
        println!("Configured connection example");
        println!("Address: {}", args.address);
        println!("Transport: {}", args.transport.label());

        let camera = camera_config(&args).open_blocking()?;
        let power = camera.power().state()?;

        println!("Connected. Power is {}.", power_label(power));
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
    eprintln!("Run with: cargo run --example transport_builder_demo -- 192.168.0.110");
}
