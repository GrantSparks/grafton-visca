//! Demonstrates the unified transport API that works for both async and blocking contexts.

use std::future::Future;
use std::pin::Pin;

use grafton_visca::command::{power::Power, PowerCommand};
use grafton_visca::transport::{UnifiedTcpTransport, UnifiedTransport};
use grafton_visca::Error;

#[cfg(feature = "blocking-client")]
fn blocking_example() -> Result<(), Error> {
    use grafton_visca::transport::unified::BlockingTransportAdapter;

    println!("=== Blocking Transport Example ===");

    // Create a blocking TCP transport
    let tcp = UnifiedTcpTransport::new_blocking("192.168.1.100:5678").map_err(Error::Io)?;

    // Wrap it in the adapter to use the unified interface
    let mut transport = BlockingTransportAdapter::new(tcp);

    // Use the unified interface - same API as async!
    let power_cmd = PowerCommand { power: Power::On };

    // This blocks immediately in blocking context
    // Since we're using the unified interface which returns futures,
    // we need to use a minimal executor to poll them
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
        Poll::Ready(result) => result?,
        Poll::Pending => return Err(Error::Timeout),
    }

    let mut recv_fut = transport.receive_response();
    match Pin::new(&mut recv_fut).poll(&mut cx) {
        Poll::Ready(result) => {
            let _ = result?;
        }
        Poll::Pending => return Err(Error::Timeout),
    }

    println!("✅ Blocking transport works with unified API");

    Ok(())
}

#[cfg(feature = "async-client")]
async fn async_example() -> Result<(), Error> {
    println!("=== Async Transport Example ===");

    // Create an async TCP transport
    let mut transport = UnifiedTcpTransport::new_async("192.168.1.100:5678")
        .await
        .map_err(Error::Io)?;

    // Use the unified interface - same API as blocking!
    let power_cmd = PowerCommand { power: Power::On };

    // This is truly async
    transport.send_command(&power_cmd).await?;
    let responses = transport.receive_response().await?;

    println!("✅ Async transport works with unified API");
    println!("   Received {} responses", responses.len());

    // Can also be done in two steps
    transport.send_command(&power_cmd).await?;
    let responses2 = transport.receive_response().await?;
    println!("✅ Two-step approach works too");
    println!("   Received {} responses", responses2.len());

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 Unified Transport API Demo");
    println!("=============================\n");

    // The beauty of the unified API: same interface for both async and blocking

    #[cfg(feature = "blocking-client")]
    {
        if let Err(e) = blocking_example() {
            println!("❌ Blocking example failed: {}", e);
        }
        println!();
    }

    #[cfg(feature = "async-client")]
    {
        let rt = tokio::runtime::Runtime::new()?;
        if let Err(e) = rt.block_on(async_example()) {
            println!("❌ Async example failed: {}", e);
        }
    }

    #[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
    {
        println!("⚠️  No transport features enabled!");
        println!("   Enable with:");
        println!("   cargo run --example unified_transport_demo --features blocking-client");
        println!("   cargo run --example unified_transport_demo --features async-client");
    }

    println!("\n✨ Key benefits of unified transport API:");
    println!("   1. Single trait for both async and blocking");
    println!("   2. No code duplication between implementations");
    println!("   3. Shared utilities (buffer management, frame parsing)");
    println!("   4. Easier to maintain and extend");

    Ok(())
}
