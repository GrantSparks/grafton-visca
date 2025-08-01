//! Demonstrates using the external executor injection API with different async runtimes.
//!
//! This example shows how to provide your own spawner implementation to integrate
//! with async runtimes other than Tokio.

#[cfg(feature = "async")]
use grafton_visca::{
    executor::{SpawnableFuture, Spawner},
    prelude::r#async::*,
};

// Example 1: A minimal spawner using std::thread
#[cfg(feature = "async")]
#[derive(Clone)]
struct ThreadSpawner;

#[cfg(feature = "async")]
impl Spawner for ThreadSpawner {
    fn spawn(&self, task: SpawnableFuture) {
        std::thread::spawn(move || {
            // Use grafton_visca's minimal executor
            grafton_visca::executor::block_on(task);
        });
    }
}

// Example 2: A spawner that integrates with a custom executor
#[cfg(feature = "async")]
#[derive(Clone)]
struct CustomExecutorSpawner {
    // Your custom executor would go here
    _executor: std::sync::Arc<()>,
}

#[cfg(feature = "async")]
impl Spawner for CustomExecutorSpawner {
    fn spawn(&self, task: SpawnableFuture) {
        // In a real implementation, you would submit the task to your executor
        println!("Would spawn task on custom executor");
        // For demo purposes, just use a thread
        std::thread::spawn(move || {
            grafton_visca::executor::block_on(task);
        });
    }
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example: Using the tokio runtime (Handle implements Spawner automatically)
    #[cfg(feature = "tokio")]
    {
        use grafton_visca::transport::TcpTransport;

        let transport = TcpTransport::connect("192.168.1.100:52381").await?;
        let handle = tokio::runtime::Handle::current();
        // For tokio feature, use the underlying camera directly
        // let inner_camera = PTZOpticsG2Cam::new_async_with_spawner(transport, handle);
        let _ = (transport, handle);
        // TODO: Update for new architecture

        println!("Created camera with Tokio spawner");
        // camera.power_on().await?;
    }

    // Example: Using a custom thread-based spawner
    {
        // For demo purposes, create a dummy transport
        struct DummyTransport;

        #[async_trait::async_trait]
        impl grafton_visca::transport::Transport for DummyTransport {
            async fn send(&self, _data: &[u8]) -> Result<(), grafton_visca::Error> {
                Ok(())
            }

            async fn recv(&self) -> Result<bytes::Bytes, grafton_visca::Error> {
                Ok(bytes::Bytes::new())
            }
        }

        let transport = DummyTransport;
        let spawner = ThreadSpawner;
        // let inner_camera = PTZOpticsG2Cam::new_async_with_spawner(transport, spawner);
        let _ = (transport, spawner);
        // TODO: Update for new architecture

        println!("Created camera with thread-based spawner");
    }

    // Example: Using a custom executor spawner
    {
        struct DummyTransport;

        #[async_trait::async_trait]
        impl grafton_visca::transport::Transport for DummyTransport {
            async fn send(&self, _data: &[u8]) -> Result<(), grafton_visca::Error> {
                Ok(())
            }

            async fn recv(&self) -> Result<bytes::Bytes, grafton_visca::Error> {
                Ok(bytes::Bytes::new())
            }
        }

        let transport = DummyTransport;
        let custom_spawner = CustomExecutorSpawner {
            _executor: std::sync::Arc::new(()),
        };
        let _inner_camera = Camera::<PTZOpticsG2, _>::new_async_with_spawner(transport, custom_spawner);
        // TODO: Update for new architecture

        println!("Created camera with custom executor spawner");
    }

    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("This example requires the 'async' feature to be enabled");
    println!("Run with: cargo run --example custom_spawner_demo --features async");
}
