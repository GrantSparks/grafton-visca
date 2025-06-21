//! Demonstrates the simplified async transport approach.
//!
//! This example shows how to use the new simplified AsyncTransport trait
//! which avoids the complexity of the full runtime abstraction layer.

use grafton_visca::{
    camera::{Camera, PTZOpticsG2},
    command::preset::{PresetAction, PresetCommand},
    transport::{AsyncTransportAdapter, SimpleTokioTcpTransport, ViscaTransport},
    Error,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    // Create a simple tokio TCP transport - no Arc<Mutex<>>, no unsafe code!
    let tcp_transport = SimpleTokioTcpTransport::connect("192.168.1.100:1259").await?;

    // Wrap it in the adapter to make it work with ViscaTransport
    let adapter = AsyncTransportAdapter::new(tcp_transport, "Simple TCP transport".to_string());

    // Create the VISCA transport wrapper
    let visca_transport = ViscaTransport::new(adapter);

    // Create camera using the transport
    let camera = Camera::<PTZOpticsG2>::new(visca_transport);

    // Use the camera normally
    println!("Recalling preset 1...");
    let preset_cmd = PresetCommand {
        action: PresetAction::Recall,
        preset_number: grafton_visca::command::preset::PresetNumber::new(1)?,
    };
    camera.send_raw(&preset_cmd).await?;

    println!("Done!");
    Ok(())
}

#[cfg(not(all(feature = "async", feature = "tokio")))]
fn main() {
    eprintln!("This example requires both 'async' and 'tokio' features");
}
