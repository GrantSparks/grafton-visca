//! Test example for Phase A transport implementation.

// TODO: Update this example for v0.5.0 - the Transport trait and feature flags have changed
fn main() {
    println!("This example needs to be updated for v0.5.0");
    println!("The Transport trait interface has changed significantly");
}

/*
#[cfg(feature = "blocking-client")]
use grafton_visca::command::power::Power;
#[cfg(feature = "blocking-client")]
use grafton_visca::command::PowerCommand;
#[cfg(feature = "blocking-client")]
use grafton_visca::transport::{BlockingAdapter, Transport, UdpTransport};
use grafton_visca::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    #[cfg(feature = "blocking-client")]
    {
        // Test blocking UDP transport through adapter
        println!("Testing UDP transport with blocking adapter...");
        let udp = UdpTransport::new("127.0.0.1:1234").map_err(Error::Io)?;
        let mut udp_adapter = BlockingAdapter(udp);

        let cmd = PowerCommand { power: Power::On };

        // This would normally send the command, but will fail since no camera is connected
        match udp_adapter.send_command(&cmd).await {
            Ok(_) => println!("Command sent successfully"),
            Err(e) => println!("Expected error (no camera): {}", e),
        }
    }

    // Test async UDP transport
    #[cfg(feature = "async-client")]
    {
        use grafton_visca::command::power::Power;
        use grafton_visca::command::PowerCommand;
        use grafton_visca::transport::{AsyncUdpTransport, Transport};

        println!("\nTesting async UDP transport...");
        let mut async_udp = AsyncUdpTransport::new("127.0.0.1:1234")
            .await
            .map_err(Error::Io)?;

        let cmd = PowerCommand { power: Power::On };

        match async_udp.send_command(&cmd).await {
            Ok(_) => println!("Command sent successfully"),
            Err(e) => println!("Expected error (no camera): {}", e),
        }
    }

    println!("\nPhase A transport implementation is working!");
    Ok(())
}
*/
