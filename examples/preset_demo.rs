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
mod blocking {
    use std::{env, time::Duration};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, AwaitConfig, Connect},
        Error,
    };

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
        fn parse() -> Result<Self, String> {
            let mut values = env::args().skip(1).collect::<Vec<_>>();

            if values.len() == 2 {
                values.insert(
                    0,
                    env::var("VISCA_CAMERA_ADDR").unwrap_or_else(|_| "192.168.0.110".to_string()),
                );
            }

            let [address, command, preset] = values.as_slice() else {
                return Err(usage());
            };

            let preset = preset
                .parse::<u8>()
                .map_err(|_| "preset must be an integer from 0 to 127".to_string())?;
            if preset > 127 {
                return Err("preset must be an integer from 0 to 127".to_string());
            }

            let operation = match command.as_str() {
                "set" => Operation::Set(preset),
                "recall" => Operation::Recall(preset),
                "clear" | "reset" => Operation::Clear(preset),
                _ => return Err(usage()),
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

    pub fn main() -> Result<(), Error> {
        let _ = tracing_subscriber::fmt::try_init();

        let args = match Args::parse() {
            Ok(args) => args,
            Err(message) => {
                eprintln!("{message}");
                return Ok(());
            }
        };

        let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&args.address)?;

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

        let _ = camera.await_idle(Duration::from_secs(1));
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
    println!("This example requires blocking mode. Run without async features:");
    println!("  cargo run --example preset_demo -- 192.168.0.110 recall 1");
}
