//! Attach a caller-owned blocking transport with `Session::open`.
//!
//! Use `Connect` for the shortest standard path. `Session::open` is the
//! canonical escape hatch when application code constructs and owns a
//! configured transport itself.

use std::{env, time::Duration};

use grafton_visca::{
    blocking::Session,
    camera::profiles::PtzOpticsG2,
    transport::{BackoffStrategy, RetryConfig, Transport},
    SessionConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110:1259".to_owned());

    let transport = Transport::udp()
        .address(address)
        .retry_config(RetryConfig {
            max_retries: 4,
            base_retry_delay: Duration::from_millis(75),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Exponential,
        })
        .build_blocking()?;
    let config = SessionConfig::from_compile_time::<PtzOpticsG2>()?;
    let session = Session::open(transport, config)?;
    let camera = session.camera::<PtzOpticsG2>()?;
    println!(
        "Power: {}",
        if camera.power().state()? { "on" } else { "off" }
    );
    session.close()?;
    Ok(())
}
