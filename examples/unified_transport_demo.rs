//! Demonstrates the unified transport API that works for both async and blocking contexts.

use grafton_visca::command::{power::Power, PowerCommand};
use grafton_visca::transport::Transport;
use grafton_visca::Error;

#[cfg(feature = "blocking-client")]
fn blocking_example() -> Result<(), Error> {
    use grafton_visca::transport::{BlockingAdapter, TcpTransport};

    println!("=== Blocking Transport Example ===");

    // Create a blocking TCP transport
    let tcp = TcpTransport::new("192.168.1.100:5678").map_err(Error::Io)?;

    // Wrap it in the adapter to use the Transport trait
    let mut transport = BlockingAdapter(tcp);

    // Use the transport interface
    let power_cmd = PowerCommand { power: Power::On };

    // This blocks immediately in blocking context
    // Since we're using the Transport interface which returns futures,
    // we need to use a minimal executor to poll them
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake};

    struct NoopWaker;
    impl Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
    }

    let waker = Arc::new(NoopWaker).into();
    let mut cx = Context::from_waker(&waker);

    let mut send_fut = transport.send_command(&power_cmd);
    match Pin::new(&mut send_fut).poll(&mut cx) {
        Poll::Ready(result) => {
            let response = result?;
            println!("Command response: {:?}", response);
        }
        Poll::Pending => return Err(Error::Timeout),
    }

    println!("Power on command sent successfully!");

    Ok(())
}

#[cfg(feature = "async-client")]
async fn async_example() -> Result<(), Error> {
    use grafton_visca::transport::AsyncTcpTransport;

    println!("=== Async Transport Example ===");

    // Create an async TCP transport
    let mut transport = AsyncTcpTransport::new("192.168.1.100:5678")
        .await
        .map_err(Error::Io)?;

    // Use the transport interface - same API!
    let power_cmd = PowerCommand { power: Power::On };

    // This returns a future that we await
    let response = transport.send_command(&power_cmd).await?;
    println!("Command response: {:?}", response);

    println!("Power on command sent successfully!");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Unified Transport Demo");
    println!("This example shows how to use transports in both async and blocking contexts.\n");

    // Run blocking example if feature is enabled
    #[cfg(feature = "blocking-client")]
    {
        if let Err(e) = blocking_example() {
            eprintln!("Blocking example error: {}", e);
        }
        println!();
    }

    // Run async example if feature is enabled
    #[cfg(feature = "async-client")]
    {
        if let Err(e) = async_example().await {
            eprintln!("Async example error: {}", e);
        }
    }

    #[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
    {
        println!(
            "Please enable either 'blocking-client' or 'async-client' feature to run this example."
        );
    }

    Ok(())
}

// Example showing how to write transport-agnostic code
#[allow(dead_code)]
fn transport_agnostic_function<T: Transport>(
    transport: &mut T,
    command: &dyn grafton_visca::Command,
) {
    // This function works with any transport implementation
    let _future = transport.send_command(command);
    // In real code, you would await or poll this future
}
