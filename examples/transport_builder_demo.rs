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
mod support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io, time::Duration};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, CameraConfig},
        transport::{BackoffStrategy, RetryConfig, TcpKeepaliveConfig, TransportConfig},
    };

    use super::support::finish_session;

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
        fn parse() -> Result<Self, io::Error> {
            let mut address = None;
            let mut transport = None;

            for arg in env::args().skip(1) {
                match arg.as_str() {
                    "--udp" => select_transport(&mut transport, TransportKind::Udp)?,
                    "--tcp" => select_transport(&mut transport, TransportKind::Tcp)?,
                    "-h" | "--help" => return Err(io::Error::other(usage())),
                    _ if arg.starts_with('-') => {
                        return Err(invalid_input(format!(
                            "unknown option `{arg}`\n{}",
                            usage()
                        )));
                    }
                    _ if address.is_none() => address = Some(arg),
                    _ => {
                        return Err(invalid_input(format!(
                            "unexpected extra argument `{arg}`\n{}",
                            usage()
                        )));
                    }
                }
            }

            Ok(Self {
                address: address
                    .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
                    .unwrap_or_else(|| "192.168.0.110".to_string()),
                transport: transport.unwrap_or(TransportKind::Tcp),
            })
        }
    }

    fn select_transport(
        selected: &mut Option<TransportKind>,
        requested: TransportKind,
    ) -> Result<(), io::Error> {
        if selected.is_some() {
            return Err(invalid_input(format!(
                "select exactly one of --tcp or --udp\n{}",
                usage()
            )));
        }
        *selected = Some(requested);
        Ok(())
    }

    fn invalid_input(message: impl Into<String>) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message.into())
    }

    fn usage() -> &'static str {
        "usage: cargo run --example transport_builder_demo -- [address] [--tcp|--udp]"
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

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let _ = tracing_subscriber::fmt::try_init();

        let args = Args::parse()?;
        println!("Configured connection example");
        println!("Address: {}", args.address);
        println!("Transport: {}", args.transport.label());

        let camera = camera_config(&args).open_blocking()?;
        let power_result = camera.power().state();
        let close_result = camera.close();
        let power = finish_session(power_result, close_result)?;

        println!("Connected. Power is {}.", power_label(power));
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
    eprintln!("Run with: cargo run --example transport_builder_demo -- 192.168.0.110");
}
