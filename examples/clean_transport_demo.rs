//! Clean transport API demonstration.
//!
//! This example shows the simplified, cleaned-up transport API without
//! any legacy compatibility layers.


#[cfg(feature = "async-client")]
use grafton_visca::{
    command::zoom::ZoomCommand,
    transport::{create, ChannelConfig, RawTransport, ViscaTransport},
};

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature to be enabled.");
    eprintln!("Run with: cargo run --example clean_transport_demo --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Clean Transport API Demo ===\n");

    // Example 1: TCP transport - simple creation
    println!("1. TCP Transport");
    match create::tcp_timeout("192.168.1.100:5678", Duration::from_secs(5)).await {
        Ok(mut transport) => {
            println!("   ✓ Created: {}", transport.description());
            println!("   ✓ Connected: {}", transport.is_connected());

            // Send a command
            let command = ZoomCommand::Stop;
            match transport.send_command(&command).await {
                Ok(response) => println!("   ✓ Response: {response:?}"),
                Err(e) => println!("   ✗ Error: {e}"),
            }
        }
        Err(e) => println!("   ✗ Failed: {e}"),
    }

    println!();

    // Example 2: UDP transport
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

    // Example 3: Serial transport
    println!("3. Serial Transport (mock)");
    let mut transport = create::serial(1);
    println!("   ✓ Created: {}", transport.description());

    let command = ZoomCommand::Stop;
    match transport.send_command(&command).await {
        Ok(response) => println!("   ✓ Response: {response:?}"),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    println!();

    // Example 4: Thread-safe channel transport
    println!("4. Channel Transport (thread-safe)");
    match create::tcp("192.168.1.100:5678").await {
        Ok(tcp_transport) => {
            let channel_transport = create::channel(tcp_transport);

            println!("   ✓ Created channel transport");

            // Clone for sharing
            let mut transport_clone = channel_transport.clone();

            // Use in a separate task
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

    // Example 5: Custom transport
    println!("5. Custom Transport");
    let custom_raw = MockTransport::new();
    let mut transport = ViscaTransport::new(custom_raw);

    println!("   ✓ Created: {}", transport.description());

    let command = ZoomCommand::Stop;
    match transport.send_command(&command).await {
        Ok(response) => println!("   ✓ Response: {response:?}"),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    println!();
    println!("=== API Summary ===");
    println!("• create::tcp(addr) - TCP transport");
    println!("• create::udp(addr) - UDP transport");
    println!("• create::serial(id) - Serial transport");
    println!("• create::channel(transport) - Thread-safe wrapper");
    println!("• ViscaTransport::new(raw) - Custom transport");
    println!();
    println!("All transports have the same send_command() interface!");

    Ok(())
}

/// Example custom transport implementation.
#[cfg(feature = "async-client")]
#[derive(Debug)]
struct MockTransport {
    responses: std::sync::Mutex<std::collections::VecDeque<Vec<u8>>>,
}

#[cfg(feature = "async-client")]
impl MockTransport {
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
impl RawTransport for MockTransport {
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
