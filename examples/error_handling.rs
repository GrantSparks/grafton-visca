//! Focused error-handling patterns with the Tokio API.
//!
//! This example classifies common VISCA errors and then performs one configured
//! connection attempt. It avoids changing camera state.
//!
//! Run with:
//! ```sh
//! cargo run --example error_handling --features runtime-tokio -- 192.168.0.110
//! ```

mod support;

use std::{borrow::Cow, env, io, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraConfig},
    runtime::TokioRuntime,
    transport::TransportConfig,
    Error,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .try_init();

    let address = address()?;

    println!("VISCA error handling example");
    classify_common_errors();
    connect_and_query(&address).await?;

    Ok(())
}

fn address() -> Result<String, io::Error> {
    let mut values = env::args().skip(1);
    let address = match values.next() {
        Some(value) if value.starts_with('-') => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown option `{value}`"),
            ));
        }
        Some(value) => value,
        None => env::var("VISCA_CAMERA_ADDR").unwrap_or_else(|_| "192.168.0.110".to_string()),
    };

    if let Some(extra) = values.next() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unexpected extra argument `{extra}`\nusage: cargo run --example error_handling --features runtime-tokio -- [address]"
            ),
        ));
    }

    Ok(address)
}

fn classify_common_errors() {
    let examples = [
        Error::CameraBusy,
        Error::CommandBufferFull,
        Error::Timeout,
        Error::SyntaxError,
        Error::CommandNotExecutable,
        Error::PresetNotFound { id: 5 },
        Error::FeatureNotSupported {
            feature: "advanced_zoom",
        },
        Error::CommandTimeout {
            duration: Duration::from_secs(5),
            command: Cow::Borrowed("zoom"),
        },
    ];

    println!("\nClassification:");
    for error in examples {
        println!(
            "  {error}: retryable={}, suggested_delay={:?}",
            error.is_retryable(),
            error.suggested_retry_delay()
        );
    }
}

async fn connect_and_query(address: &str) -> Result<(), Error> {
    println!("\nConnection check:");
    println!("  Address: {address}");

    let config = CameraConfig::<PtzOpticsG2>::tcp(address).transport_config(TransportConfig {
        connect_timeout: Duration::from_secs(3),
        read_timeout: Duration::from_secs(2),
        write_timeout: Duration::from_secs(2),
        ..TransportConfig::default()
    });

    let runtime = TokioRuntime::from_current()?;
    let camera = match config.open_async(runtime).await {
        Ok(camera) => camera,
        Err(error) => {
            println!("  Connection failed: {error}");
            println!("  retryable={}", error.is_retryable());
            if let Some(delay) = error.suggested_retry_delay() {
                println!("  suggested retry delay: {delay:?}");
            }
            return Err(error);
        }
    };

    let inquiry_result = camera.power().state().await;
    match &inquiry_result {
        Ok(is_on) => println!(
            "  Connected. Power is {}.",
            if *is_on { "on" } else { "off" }
        ),
        Err(error) => println!("  Connected, but power inquiry failed: {error}"),
    }
    let close_result = camera.close().await;

    finish_session(inquiry_result.map(|_| ()), close_result)
}
