//! Focused error-handling patterns with the Tokio API.
//!
//! This example classifies common VISCA errors and then performs one configured
//! connection attempt. It avoids changing camera state.
//!
//! Run with:
//! ```sh
//! cargo run --example error_handling --features runtime-tokio -- 192.168.0.110
//! ```

use std::{borrow::Cow, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraConfig},
    runtime::TokioRuntime,
    transport::TransportConfig,
    Error,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .try_init();

    let address = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("VISCA error handling example");
    classify_common_errors();
    connect_and_query(&address).await;

    Ok(())
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

async fn connect_and_query(address: &str) {
    println!("\nConnection check:");
    println!("  Address: {address}");

    let config = CameraConfig::<PtzOpticsG2>::tcp(address).transport_config(TransportConfig {
        connect_timeout: Duration::from_secs(3),
        read_timeout: Duration::from_secs(2),
        write_timeout: Duration::from_secs(2),
        ..TransportConfig::default()
    });

    let runtime = match TokioRuntime::from_current() {
        Ok(runtime) => runtime,
        Err(error) => {
            println!("  Runtime unavailable: {error}");
            return;
        }
    };

    match config.open_async(runtime).await {
        Ok(camera) => {
            match camera.power().state().await {
                Ok(is_on) => println!(
                    "  Connected. Power is {}.",
                    if is_on { "on" } else { "off" }
                ),
                Err(error) => println!("  Connected, but power inquiry failed: {error}"),
            }
            let _ = camera.close().await;
        }
        Err(error) => {
            println!("  Connection failed: {error}");
            println!("  retryable={}", error.is_retryable());
            if let Some(delay) = error.suggested_retry_delay() {
                println!("  suggested retry delay: {delay:?}");
            }
        }
    }
}
