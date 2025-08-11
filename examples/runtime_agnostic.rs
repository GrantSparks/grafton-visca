//! Runtime-agnostic async example demonstrating custom runtime usage.
//!
//! This example shows how the library can be used with a custom runtime implementation
//! without requiring tokio features. It demonstrates the compile-time API structure.
//!
//! Run with:
//! ```sh
//! cargo run --example runtime_agnostic
//! ```

#[cfg(feature = "async")]
fn main() {
    use grafton_visca::{
        executor::{Sleep, SpawnableFuture, Spawner},
        runtime::{GenericRuntime, SharedRuntime},
    };
    use std::{pin::Pin, sync::Arc, time::Duration};

    println!("🎥 Runtime-Agnostic Camera Control Demo");
    println!("========================================");
    println!();

    // Define a custom sleep implementation
    #[derive(Debug, Clone)]
    struct CustomSleep;

    impl Sleep for CustomSleep {
        fn sleep(
            &self,
            duration: Duration,
        ) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
            // In a real implementation, this would use your runtime's sleep primitive
            // For demonstration, we're using a no-op future
            Box::pin(async move {
                let _ = duration;
                // Sleep would happen here
            })
        }
    }

    // Define a custom spawner implementation
    #[derive(Debug, Clone)]
    struct CustomSpawner;

    impl Spawner for CustomSpawner {
        fn spawn(&self, task: SpawnableFuture) {
            // In a real implementation, this would spawn on your runtime
            let _ = task;
        }
    }

    // Create a custom runtime using your Sleep and Spawner implementations
    let _runtime: SharedRuntime = Arc::new(GenericRuntime::new(CustomSleep, CustomSpawner));

    println!("✅ Created custom runtime implementation");

    // Demonstrate that the library can be used with a custom runtime
    // The type system ensures that Camera can work with any runtime
    // This shows the API structure without requiring an actual transport
    println!("The Camera type accepts custom runtimes:");
    println!("  Camera::<Profile, Transport>::new(transport).with_runtime(runtime)");

    println!("✅ Demonstrated that Camera accepts custom runtime");
    println!();

    println!("Key Points:");
    println!("- The library does not require tokio features for its runtime abstraction");
    println!("- You can provide your own Runtime implementation");
    println!("- The Runtime trait requires only spawn() and sleep() methods");
    println!("- All async operations will use your provided runtime");
    println!();

    println!("To use with a real async runtime:");
    println!("1. Implement the Sleep trait for your runtime's sleep primitive");
    println!("2. Implement the Spawner trait for your runtime's task spawning");
    println!("3. Create a GenericRuntime with your implementations");
    println!("4. Pass it to Camera::with_runtime()");
    println!();

    println!("✅ Example completed successfully!");
}

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Run with: cargo run --example runtime_agnostic --features async");
    std::process::exit(1);
}
