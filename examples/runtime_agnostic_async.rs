//! Example demonstrating runtime-agnostic async support.
//!
//! This example shows how to use the library with the `async` feature
//! but without the `tokio` feature, allowing you to bring your own runtime.

#[cfg(all(feature = "async", not(feature = "tokio")))]
fn main() {
    println!("Runtime-Agnostic Async Example");
    println!("==============================");
    println!();
    println!("This example demonstrates how to use grafton-visca with any async runtime.");
    println!();
    println!("When using only the 'async' feature (without 'tokio'), you must:");
    println!("1. Implement the Transport trait for your runtime's I/O types");
    println!("2. Handle timeouts using your runtime's timeout facilities");
    println!("3. Manage any background tasks yourself");
    println!();
    println!("Example with async-std:");
    println!();
    println!(
        r#"
use grafton_visca::{{Camera, CameraModel, transport::Transport}};
use async_std::net::TcpStream;
use async_std::io::{{ReadExt, WriteExt}};
use async_std::future::timeout;
use std::time::Duration;
use std::future::Future;
use std::pin::Pin;

struct AsyncStdTcp {{
    stream: TcpStream,
}}

impl Transport for AsyncStdTcp {{
    type Error = std::io::Error;
    type SendFut<'a> = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>>;
    type RecvFut<'a> = Pin<Box<dyn Future<Output = Result<bytes::Bytes, Self::Error>> + Send + 'a>>;

    fn send<'a>(&'a self, bytes: &'a [u8]) -> Self::SendFut<'a> {{
        Box::pin(async move {{
            self.stream.write_all(bytes).await?;
            self.stream.flush().await
        }})
    }}

    fn recv<'a>(&'a self) -> Self::RecvFut<'a> {{
        Box::pin(async move {{
            // Read until VISCA terminator (0xFF)
            let mut buffer = Vec::new();
            let mut byte = [0u8; 1];
            
            loop {{
                self.stream.read_exact(&mut byte).await?;
                buffer.push(byte[0]);
                if byte[0] == 0xFF {{
                    break;
                }}
            }}
            
            Ok(bytes::Bytes::from(buffer))
        }})
    }}
}}

#[async_std::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    // Connect using async-std
    let stream = TcpStream::connect("192.168.1.100:5678").await?;
    let transport = AsyncStdTcp {{ stream }};
    
    // Create camera with the transport
    let camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);
    
    // Use timeout from async-std for operations
    let timeout_duration = Duration::from_secs(5);
    
    // Power on with timeout
    timeout(timeout_duration, camera.power_on()).await??;
    
    // Move to home with timeout
    timeout(timeout_duration, camera.pan_tilt_home()).await??;
    
    Ok(())
}}
"#
    );

    println!();
    println!("Key differences from tokio mode:");
    println!("- No built-in timeout handling (must use runtime's timeout)");
    println!("- Must implement Transport trait yourself");
    println!("- No pre-built TCP/UDP implementations");
    println!("- Complete control over async runtime");
    println!();
    println!("Benefits:");
    println!("- Works with ANY async runtime (async-std, smol, embassy, etc.)");
    println!("- No tokio dependency if you don't need it");
    println!("- Minimal dependencies for embedded or constrained environments");
}

#[cfg(not(all(feature = "async", not(feature = "tokio"))))]
fn main() {
    eprintln!("This example requires the 'async' feature without 'tokio'.");
    eprintln!("Run with: cargo run --example runtime_agnostic_async --features async --no-default-features");
}
