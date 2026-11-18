//! Focused error-handling patterns with the Tokio API.
//!
//! This example classifies common VISCA errors and then performs one configured
//! connection attempt. It avoids changing camera state.
//!
//! Run with:
//! ```sh
//! cargo run --example error_handling --features runtime-tokio -- 192.0.2.10
//! ```

mod support;

use std::{env, io, time::Duration};

use grafton_visca::{
    config::CameraConfig,
    error::Recovery,
    profiles::PtzOpticsG2,
    r#async::CameraSession,
    runtime::TokioRuntime,
    transport::{BufferConfig, IoBounds, TcpConnect},
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
        None => env::var("VISCA_CAMERA_ADDR").unwrap_or_else(|_| "192.0.2.10".to_string()),
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
        Error::CommandBufferFull,
        Error::NoSocket,
        Error::io_timeout(),
        Error::SyntaxError,
        Error::CommandNotExecutable,
        Error::runtime_queue_full(64),
        Error::feature_not_supported("advanced_zoom"),
        Error::UnsequencedCommandUnconfirmed,
    ];

    println!("\nClassification:");
    for error in examples {
        let decision = match error.recovery() {
            Recovery::Resubmit { after } => format!("submit again after {after:?}"),
            Recovery::WaitAgain => "wait on the existing operation".into(),
            Recovery::Reconcile => "inspect camera state before deciding".into(),
            Recovery::ReopenSession => "open a new session".into(),
            Recovery::Abandon => "do not retry automatically".into(),
            _ => "inspect this future recovery verdict".into(),
        };
        println!(
            "  {error}: {decision}; context={:?}",
            error.failure_context()
        );
    }
}

async fn connect_and_query(address: &str) -> Result<(), Error> {
    println!("\nConnection check:");
    println!("  Address: {address}");

    let io_timeout = Duration::from_secs(2);
    let config = CameraConfig::<PtzOpticsG2>::tcp_configured(
        address,
        TcpConnect::default().with_connect_timeout(Duration::from_secs(3))?,
        IoBounds::new(io_timeout, io_timeout, BufferConfig::for_tcp())?,
    );

    let runtime = TokioRuntime::from_current()?;
    let session = match CameraSession::connect(&config, runtime).await {
        Ok(session) => session,
        Err(error) => {
            println!("  Connection failed: {error}");
            println!("  recovery={:?}", error.recovery());
            return Err(error);
        }
    };

    let camera = session.camera();
    let inquiry_result = camera.power().status().await;
    match &inquiry_result {
        Ok(status) => println!("  Connected. Power status is {status:?}."),
        Err(error) => println!("  Connected, but power inquiry failed: {error}"),
    }
    let close_result = session.close().await;

    finish_session(inquiry_result.map(|_| ()), close_result)
}
