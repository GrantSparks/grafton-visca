//! Minimal clean transport API demonstration.
//!
//! Shows the fully cleaned transport API without any legacy compatibility.


#[cfg(feature = "async-client")]
use grafton_visca::{
    command::zoom::ZoomCommand,
    transport::{create, RawTransport, ViscaTransport},
};

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example minimal_transport --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Minimal Clean Transport API ===\n");

    // TCP transport - one line creation
    println!("1. TCP Transport");
    match create::tcp_timeout("192.168.1.100:5678", Duration::from_secs(5)).await {
        Ok(mut transport) => {
            println!("   ✓ Created: {}", transport.description());

            let command = ZoomCommand::Stop;
            match transport.send_command(&command).await {
                Ok(response) => println!("   ✓ Response: {response:?}"),
                Err(e) => println!("   ✗ Error: {e}"),
            }
        }
        Err(e) => println!("   ✗ Failed: {e}"),
    }

    println!();

    // UDP transport
    println!("2. UDP Transport");
    match create::udp("192.168.1.100:52381").await {
        Ok(mut transport) => {
            println!("   ✓ Created: {}", transport.description());

            let command = ZoomCommand::Stop;
            match transport.send_command(&command).await {
                Ok(response) => println!("   ✓ Response: {response:?}"),
                Err(e) => println!("   ✗ Error: {e}"),
            }
        }
        Err(e) => println!("   ✗ Failed: {e}"),
    }

    println!();

    // Serial transport
    println!("3. Serial Transport");
    let mut transport = create::serial(1);
    println!("   ✓ Created: {}", transport.description());

    let command = ZoomCommand::Stop;
    match transport.send_command(&command).await {
        Ok(response) => println!("   ✓ Response: {response:?}"),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    println!();

    // Custom transport
    println!("4. Custom Transport");
    let custom_raw = MockRaw::new();
    let mut transport = ViscaTransport::new(custom_raw);
    println!("   ✓ Created: {}", transport.description());

    let command = ZoomCommand::Stop;
    match transport.send_command(&command).await {
        Ok(response) => println!("   ✓ Response: {response:?}"),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    println!();

    // Thread-safe channel transport
    println!("5. Channel Transport");
    match create::tcp("192.168.1.100:5678").await {
        Ok(tcp_transport) => {
            let channel_transport = create::channel(tcp_transport);

            println!("   ✓ Created channel transport");

            // Clone for sharing
            let mut transport_clone = channel_transport.clone();

            let handle = tokio::spawn(async move {
                let command = ZoomCommand::Stop;
                transport_clone.send_command(&command).await
            });

            match handle.await? {
                Ok(response) => println!("   ✓ Task response: {response:?}"),
                Err(e) => println!("   ✗ Task error: {e}"),
            }
        }
        Err(e) => println!("   ✗ Failed: {e}"),
    }

    println!();
    println!("=== Clean API Benefits ===");
    println!("• One simple creation function per transport type");
    println!("• All transports have identical send_command() interface");
    println!("• No trait objects or boxing required for simple usage");
    println!("• Channel wrapper provides thread-safe sharing");
    println!("• Custom transports only need 4 simple methods");
    println!("• All VISCA protocol complexity hidden in the library");

    Ok(())
}

/// Simple mock transport for demonstration.
#[cfg(feature = "async-client")]
#[derive(Debug)]
struct MockRaw {
    responses: std::sync::Mutex<std::collections::VecDeque<Vec<u8>>>,
}

#[cfg(feature = "async-client")]
impl MockRaw {
    fn new() -> Self {
        let mut responses = std::collections::VecDeque::new();
        responses.push_back(vec![0x90, 0x40, 0xFF]); // ACK
        responses.push_back(vec![0x90, 0x50, 0xFF]); // Completion

        Self {
            responses: std::sync::Mutex::new(responses),
        }
    }
}

#[cfg(feature = "async-client")]
impl RawTransport for MockRaw {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> grafton_visca::transport::TransportFuture<'a, ()> {
        Box::pin(async move {
            println!("   Mock send: {:02X?}", data);
            Ok(())
        })
    }

    fn receive<'a>(&'a mut self) -> grafton_visca::transport::TransportFuture<'a, Vec<u8>> {
        Box::pin(async move {
            if let Ok(mut responses) = self.responses.lock() {
                if let Some(response) = responses.pop_front() {
                    println!("   Mock receive: {:02X?}", response);
                    return Ok(response);
                }
            }

            Err(grafton_visca::Error::CommandTimeout {
                duration: Duration::from_millis(100),
                command: "mock".to_string(),
            })
        })
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Mock transport for testing"
    }
}
