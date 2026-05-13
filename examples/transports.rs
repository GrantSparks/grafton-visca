//! Compare TCP and UDP connection setup.
//!
//! This example keeps the camera state unchanged. It opens TCP and/or UDP
//! connections and runs a power inquiry so users can confirm which transport
//! works for their camera profile and network.
//!
//! Run with:
//! ```sh
//! cargo run --example transports -- 192.168.0.110
//! cargo run --example transports -- 192.168.0.110 --tcp-only
//! cargo run --example transports -- 192.168.0.110 --udp-only
//! ```

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::env;

    use grafton_visca::{camera::Connect, profiles::PtzOpticsG2, Error};

    #[derive(Debug, Clone, Copy)]
    enum Selection {
        TcpOnly,
        UdpOnly,
        Both,
    }

    #[derive(Debug)]
    struct Args {
        address: String,
        selection: Selection,
    }

    impl Args {
        fn parse() -> Self {
            let mut address = None;
            let mut selection = Selection::Both;

            for arg in env::args().skip(1) {
                match arg.as_str() {
                    "--tcp-only" => selection = Selection::TcpOnly,
                    "--udp-only" => selection = Selection::UdpOnly,
                    _ if address.is_none() => address = Some(arg),
                    _ => {}
                }
            }

            Self {
                address: address
                    .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
                    .unwrap_or_else(|| "192.168.0.110".to_string()),
                selection,
            }
        }

        fn include_tcp(&self) -> bool {
            matches!(self.selection, Selection::TcpOnly | Selection::Both)
        }

        fn include_udp(&self) -> bool {
            matches!(self.selection, Selection::UdpOnly | Selection::Both)
        }
    }

    fn power_label(is_on: bool) -> &'static str {
        if is_on {
            "on"
        } else {
            "off"
        }
    }

    fn check_tcp(address: &str) -> Result<(), Error> {
        let camera = Connect::open_tcp_blocking::<PtzOpticsG2>(address)?;
        let power = camera.power().state()?;
        println!("TCP: connected, power is {}", power_label(power));
        camera.close()?;
        Ok(())
    }

    fn check_udp(address: &str) -> Result<(), Error> {
        let camera = Connect::open_udp_blocking::<PtzOpticsG2>(address)?;
        let power = camera.power().state()?;
        println!("UDP: connected, power is {}", power_label(power));
        camera.close()?;
        Ok(())
    }

    pub fn main() {
        let _ = tracing_subscriber::fmt::try_init();

        let args = Args::parse();
        println!("Transport check");
        println!("Address: {}", args.address);

        if args.include_tcp() {
            if let Err(error) = check_tcp(&args.address) {
                println!("TCP: failed: {error}");
            }
        }

        if args.include_udp() {
            if let Err(error) = check_udp(&args.address) {
                println!("UDP: failed: {error}");
            }
        }
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    println!("This example requires blocking mode. Run without async features:");
    println!("  cargo run --example transports -- 192.168.0.110");
}
