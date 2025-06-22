//! Demonstrates the simplified async transport approach.
//!
//! This example shows how to use the new simplified AsyncTransport trait
//! which avoids the complexity of the full runtime abstraction layer.

use grafton_visca::{
    camera::Camera,
    command::preset::{PresetAction, PresetCommand},
    profiles::PTZOpticsG2,
    transport::create,
    Error,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    // Create a TCP transport using the simplified API
    let visca_transport = create::tcp("192.168.1.100:1259").await?;

    // Create camera using the transport
    let camera = Camera::<PTZOpticsG2, _>::new(visca_transport);

    // Use the camera normally
    println!("Recalling preset 1...");
    let preset_cmd = PresetCommand {
        action: PresetAction::Recall,
        preset_number: grafton_visca::command::preset::PresetNumber::new(1)?,
    };
    camera.send_command(&preset_cmd).await?;

    println!("Done!");
    Ok(())
}

#[cfg(not(all(feature = "async", feature = "tokio")))]
fn main() {
    eprintln!("This example requires both 'async' and 'tokio' features");
}
