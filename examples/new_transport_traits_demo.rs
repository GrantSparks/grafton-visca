use grafton_visca::{Error, transport::{traits::*, mock::MockTransportBuilder}};

fn main() -> Result<(), Error> {
    env_logger::init();

    // Create a mock transport with predefined responses
    let mut transport = MockTransportBuilder::new()
        .with_response(vec![0x90, 0x50, 0xFF])  // ACK response
        .with_response(vec![0x90, 0x51, 0xFF])  // Completion response
        .timeout(std::time::Duration::from_secs(2))
        .retries(3)
        .build()?;

    println!("Transport created with config: {:?}", transport.config());

    // Send a power on command
    let power_on_cmd = vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
    println!("Sending command: {:?}", power_on_cmd);
    
    let response = transport.send_command(&power_on_cmd)?;
    println!("Received response: {:?}", response);

    // Send another command
    let zoom_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];
    println!("Sending command: {:?}", zoom_cmd);
    
    let response = transport.send_command(&zoom_cmd)?;
    println!("Received response: {:?}", response);

    // Check statistics
    let stats = transport.stats();
    println!("\nTransport Statistics:");
    println!("  Commands sent: {}", stats.commands_sent);
    println!("  Responses received: {}", stats.responses_received);
    println!("  Errors: {}", stats.errors);
    println!("  Connection status: {}", transport.is_connected());

    // Close the transport
    transport.close()?;
    println!("Transport closed. Connection status: {}", transport.is_connected());

    Ok(())
}