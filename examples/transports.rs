//! Compare TCP and UDP connection setup for a PTZOptics G2-compatible camera.
//!
//! This example keeps the camera state unchanged. It opens TCP and/or UDP
//! connections and runs a power inquiry so users can confirm which transport
//! works for that profile and network. When checking both transports, omit an
//! explicit port so each transport uses its profile-defined default.
//!
//! Run with:
//! ```sh
//! cargo run --example transports -- 192.168.0.110
//! cargo run --example transports -- 192.168.0.110 --tcp-only
//! cargo run --example transports -- 192.168.0.110 --udp-only
//! ```

#[cfg(not(feature = "mode-async"))]
mod support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io, net::SocketAddr};

    use grafton_visca::{
        camera::Connect, capabilities::Capabilities, profiles::PtzOpticsG2, Error,
    };

    use super::support::finish_session;

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
        fn parse() -> Result<Self, io::Error> {
            let mut address = None;
            let mut selection = None;

            for arg in env::args().skip(1) {
                match arg.as_str() {
                    "--tcp-only" => select(&mut selection, Selection::TcpOnly)?,
                    "--udp-only" => select(&mut selection, Selection::UdpOnly)?,
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

            let selection = selection.unwrap_or(Selection::Both);
            let address = address
                .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
                .unwrap_or_else(|| "192.168.0.110".to_string());

            if matches!(selection, Selection::Both) && has_explicit_port(&address) {
                return Err(invalid_input(
                    "omit the port when checking both transports so TCP and UDP use their distinct profile defaults",
                ));
            }

            Ok(Self { address, selection })
        }

        fn include_tcp(&self) -> bool {
            matches!(self.selection, Selection::TcpOnly | Selection::Both)
        }

        fn include_udp(&self) -> bool {
            matches!(self.selection, Selection::UdpOnly | Selection::Both)
        }
    }

    fn select(selected: &mut Option<Selection>, requested: Selection) -> Result<(), io::Error> {
        if selected.is_some() {
            return Err(invalid_input(format!(
                "select at most one of --tcp-only or --udp-only\n{}",
                usage()
            )));
        }
        *selected = Some(requested);
        Ok(())
    }

    fn has_explicit_port(address: &str) -> bool {
        address.parse::<SocketAddr>().is_ok()
            || (address.matches(':').count() == 1
                && address
                    .rsplit_once(':')
                    .is_some_and(|(_, port)| port.parse::<u16>().is_ok()))
    }

    fn invalid_input(message: impl Into<String>) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message.into())
    }

    fn usage() -> &'static str {
        "usage: cargo run --example transports -- [address] [--tcp-only|--udp-only]"
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
        let query_result = camera.power().state();
        let close_result = camera.close();

        let power = finish_session(query_result, close_result)?;
        println!("TCP: connected, power is {}", power_label(power));
        Ok(())
    }

    fn check_udp(address: &str) -> Result<(), Error> {
        let camera = Connect::open_udp_blocking::<PtzOpticsG2>(address)?;
        let query_result = camera.power().state();
        let close_result = camera.close();

        let power = finish_session(query_result, close_result)?;
        println!("UDP: connected, power is {}", power_label(power));
        Ok(())
    }

    fn report(label: &str, result: &Result<(), Error>) {
        if let Err(error) = result {
            println!("{label}: failed: {error}");
        }
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let _ = tracing_subscriber::fmt::try_init();

        let args = Args::parse()?;
        let capabilities = Capabilities::from_profile::<PtzOpticsG2>();
        println!("Transport check");
        println!("Profile: {}", capabilities.model_name);
        println!("Address: {}", args.address);

        let tcp_result = args.include_tcp().then(|| check_tcp(&args.address));
        let udp_result = args.include_udp().then(|| check_udp(&args.address));

        if let Some(result) = &tcp_result {
            report("TCP", result);
        }
        if let Some(result) = &udp_result {
            report("UDP", result);
        }

        if let Some(result) = tcp_result {
            result?;
        }
        if let Some(result) = udp_result {
            result?;
        }

        Ok(())
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    println!("This example requires blocking mode. Run without async features:");
    println!("  cargo run --example transports -- 192.168.0.110");
}
