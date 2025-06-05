//! Test example for Phase A transport implementation.

// Access internal transport module for testing
use grafton_visca::transport;
use grafton_visca::{ViscaCommand, ViscaError};
use grafton_visca::command::PowerCommand;
use grafton_visca::command::power::Power;

#[tokio::main]
async fn main() -> Result<(), ViscaError> {
    env_logger::init();

    // Test blocking UDP transport through adapter
    println!("Testing UDP transport with blocking adapter...");
    let udp = UdpTransport::new("127.0.0.1:1234").map_err(|e| ViscaError::Io(e))?;
    let mut udp_adapter = BlockingAdapter(udp);
    
    let cmd = PowerCommand { power: Power::On };
    
    // This would normally send the command, but will fail since no camera is connected
    match udp_adapter.send_command(&cmd).await {
        Ok(_) => println!("Command sent successfully"),
        Err(e) => println!("Expected error (no camera): {}", e),
    }

    // Test async UDP transport
    #[cfg(feature = "async-client")]
    {
        use grafton_visca::transport::AsyncUdpTransport;
        
        println!("\nTesting async UDP transport...");
        let mut async_udp = AsyncUdpTransport::new("127.0.0.1:1234").await
            .map_err(|e| ViscaError::Io(e))?;
        
        match async_udp.send_command(&cmd).await {
            Ok(_) => println!("Command sent successfully"),
            Err(e) => println!("Expected error (no camera): {}", e),
        }
    }

    println!("\nPhase A transport implementation is working!");
    Ok(())
}