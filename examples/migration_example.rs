//! Example showing how to migrate from old transport traits to new unified client

use grafton_visca::{
    command::{power::Power, PowerCommand},
    send_command_and_wait,
    // Old imports
    UdpTransport as OldUdpTransport,
    // New imports
    ViscaClient,
    // Common imports
    ViscaError,
};

fn main() -> Result<(), ViscaError> {
    // Method 1: Using old transport directly (deprecated)
    println!("=== Method 1: Old transport with send_command_and_wait ===");
    let mut old_transport = OldUdpTransport::new("192.168.1.100:5678").map_err(ViscaError::Io)?;

    let power_on = PowerCommand { power: Power::On };
    let response = send_command_and_wait(&mut old_transport, &power_on)?;
    println!("Response: {:?}", response);

    // Method 2: Migrate old transport to unified client (recommended)
    println!("\n=== Method 2: Migrate old transport to unified client ===");
    #[cfg(feature = "async-client")]
    {
        let old_transport = OldUdpTransport::new("192.168.1.100:5678").map_err(ViscaError::Io)?;

        // Convert old transport to work with unified client
        let client = ViscaClient::from_legacy_transport(old_transport);

        let response = client.send(&power_on)?;
        println!("Response: {:?}", response);
    }

    // Method 3: Use new unified client directly (best)
    println!("\n=== Method 3: New unified client (recommended) ===");
    #[cfg(feature = "blocking-client")]
    {
        let client = ViscaClient::connect_udp("192.168.1.100:5678")?;
        let response = client.send(&power_on)?;
        println!("Response: {:?}", response);
    }

    Ok(())
}

