//! Focused preset operations.
//!
//! This example performs exactly one preset operation. It does not move the
//! camera to staged positions or overwrite multiple presets.
//!
//! Run with:
//! ```sh
//! cargo run --example preset_demo -- 192.168.0.110 set 1
//! cargo run --example preset_demo -- 192.168.0.110 recall 1
//! cargo run --example preset_demo -- 192.168.0.110 clear 1
//! ```

#[cfg(not(feature = "mode-async"))]
mod support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, AwaitConfig, Connect},
        capabilities::Capabilities,
        Error,
    };

    use super::support::finish_session;

    #[derive(Debug, Clone, Copy)]
    enum Operation {
        Set(u8),
        Recall(u8),
        Clear(u8),
    }

    #[derive(Debug)]
    struct Args {
        address: String,
        operation: Operation,
    }

    impl Args {
        fn parse() -> Result<Self, io::Error> {
            let mut values = env::args().skip(1).collect::<Vec<_>>();

            if values.len() == 2 {
                values.insert(
                    0,
                    env::var("VISCA_CAMERA_ADDR").unwrap_or_else(|_| "192.168.0.110".to_string()),
                );
            }

            let [address, command, preset] = values.as_slice() else {
                return Err(invalid_input(usage()));
            };
            if address.starts_with('-') {
                return Err(invalid_input(format!(
                    "invalid address `{address}`\n{}",
                    usage()
                )));
            }

            let preset = preset
                .parse::<u8>()
                .map_err(|_| invalid_input("preset must be an integer from 0 to 255"))?;
            let max_preset = Capabilities::from_profile::<PtzOpticsG2>().max_presets;
            if preset > max_preset {
                return Err(invalid_input(format!(
                    "preset must be in the PtzOpticsG2 profile range 0..={max_preset}"
                )));
            }

            let operation = match command.as_str() {
                "set" => Operation::Set(preset),
                "recall" => Operation::Recall(preset),
                "clear" | "reset" => Operation::Clear(preset),
                _ => return Err(invalid_input(usage())),
            };

            Ok(Self {
                address: address.clone(),
                operation,
            })
        }
    }

    fn usage() -> String {
        "usage: cargo run --example preset_demo -- [address] <set|recall|clear> <preset>"
            .to_string()
    }

    fn invalid_input(message: impl Into<String>) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message.into())
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let _ = tracing_subscriber::fmt::try_init();

        let args = Args::parse()?;

        let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&args.address)?;
        let operation_result = (|| -> Result<(), Error> {
            match args.operation {
                Operation::Set(preset) => {
                    camera.presets().set(preset)?;
                    println!("Saved current position to preset {preset}.");
                }
                Operation::Recall(preset) => {
                    camera.presets().recall(preset)?;
                    camera.await_with_config(&AwaitConfig::for_preset_recall())?;
                    println!("Recalled preset {preset}.");
                }
                Operation::Clear(preset) => {
                    camera.presets().reset(preset)?;
                    println!("Cleared preset {preset}.");
                }
            }

            Ok(())
        })();
        let close_result = camera.close();

        finish_session(operation_result, close_result)?;
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
    println!("  cargo run --example preset_demo -- 192.168.0.110 recall 1");
}
