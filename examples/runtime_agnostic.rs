//! Runtime-agnostic async example showing how to bring your own runtime.
//!
//! This example demonstrates:
//! 1. How to implement the Executor trait for a custom runtime
//! 2. How to use the library without depending on any specific runtime
//! 3. How to create your own AsyncTransport implementation
//!
//! The library provides built-in executors for common runtimes:
//! - tokio (with --features rt-tokio)
//! - async-std (with --features rt-async-std)  
//! - smol (with --features rt-smol)
//!
//! But you can use ANY runtime by implementing the Executor trait!
//!
//! Run with:
//! ```sh
//! cargo run --example runtime_agnostic --features async
//! cargo run --example runtime_agnostic --features rt-tokio
//! ```

#[cfg(feature = "async")]
fn main() {
    use std::{future::Future, pin::Pin, time::Duration};

    use grafton_visca::{Error, Executor};

    println!("🎥 Runtime-Agnostic Camera Control Demo");
    println!("========================================");
    println!();

    // Show which runtime features are enabled
    #[cfg(feature = "rt-tokio")]
    println!("✅ Tokio runtime support enabled");

    #[cfg(feature = "rt-async-std")]
    println!("✅ async-std runtime support enabled");

    #[cfg(feature = "rt-smol")]
    println!("✅ smol runtime support enabled");

    println!();

    // Example: Implement a custom executor for your runtime
    // This shows the minimal interface you need to implement
    #[derive(Debug, Clone)]
    struct MyCustomExecutor;

    impl Executor for MyCustomExecutor {
        type Join<T>
            = Pin<Box<dyn Future<Output = Result<T, grafton_visca::ExecError>> + Send + 'static>>
        where
            T: Send + 'static;

        fn spawn<F>(&self, future: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            // Example: If using async-std, you would do:
            // let handle = async_std::task::spawn(future);
            // Box::pin(async move {
            //     Ok(handle.await)
            // })

            // For this demo, we return a stub
            let _ = future;
            Box::pin(async {
                Err(grafton_visca::ExecError::TaskFailed(
                    "Demo executor - implement spawn() for your runtime".into(),
                ))
            })
        }

        fn block_on<F: Future>(&self, future: F) -> F::Output {
            // Example: If using async-std, you would do:
            // async_std::task::block_on(future)

            // For this demo, we panic
            let _ = future;
            panic!("Demo executor - implement block_on() for your runtime")
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            // Example: If using async-std, you would do:
            // Box::pin(async_std::task::sleep(duration))

            // For this demo, we return immediately
            Box::pin(async move {
                println!("  Would sleep for {:?}", duration);
            })
        }

        fn timeout<'a, F, T>(
            &'a self,
            duration: Duration,
            future: F,
        ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            // Example: If using async-std, you would do:
            // Box::pin(async move {
            //     async_std::future::timeout(duration, future)
            //         .await
            //         .map_err(|_| Error::Timeout)
            // })

            // For this demo, we return timeout error
            let _ = (duration, future);
            Box::pin(async { Err(Error::Timeout) })
        }
    }

    // Demonstrate using different runtime executors
    #[cfg(feature = "rt-tokio")]
    {
        use grafton_visca::TokioExecutor;

        println!("Using Tokio executor:");
        if let Ok(_executor) = TokioExecutor::from_current() {
            println!("  ✅ Created TokioExecutor from current runtime");
        } else {
            println!("  Creating TokioExecutor outside of runtime context");
            let rt = tokio::runtime::Runtime::new().unwrap();
            let _guard = rt.enter();
            let executor = TokioExecutor::from_current().unwrap();
            println!("  ✅ Created TokioExecutor after entering runtime");
            let _ = executor;
        }
    }

    #[cfg(feature = "rt-async-std")]
    {
        use grafton_visca::AsyncStdExecutor;

        println!("Using async-std executor:");
        let executor = AsyncStdExecutor::new();
        println!("  ✅ Created AsyncStdExecutor");
        let _ = executor;
    }

    #[cfg(feature = "rt-smol")]
    {
        use grafton_visca::SmolExecutor;

        println!("Using smol executor:");
        let executor = SmolExecutor::new();
        println!("  ✅ Created SmolExecutor");
        let _ = executor;
    }

    // Create an instance of your custom executor
    let my_executor = MyCustomExecutor;
    println!("\n✅ Created custom executor implementation");

    // Example: How to create your own AsyncTransport
    println!("\n📡 Custom AsyncTransport Example:");
    println!("```rust");
    println!("use grafton_visca::transport::AsyncTransport;");
    println!("use bytes::Bytes;");
    println!();
    println!("struct MyCustomTransport {{ /* your fields */ }}");
    println!();
    println!("impl AsyncTransport for MyCustomTransport {{");
    println!("    async fn send(&self, bytes: &[u8]) -> Result<()> {{");
    println!("        // Send bytes using your async I/O");
    println!("        Ok(())");
    println!("    }}");
    println!();
    println!("    async fn recv(&self) -> Result<Bytes> {{");
    println!("        // Receive response using your async I/O");
    println!("        Ok(Bytes::new())");
    println!("    }}");
    println!("}}");
    println!("```");

    // Show how it all comes together
    println!("\n🔧 Putting It All Together:");
    println!("```rust");
    println!("// Create your transport");
    println!("let transport = MyCustomTransport::new();");
    println!();
    println!("// Create your executor");
    println!("let executor = MyCustomExecutor::new();");
    println!();
    println!("// Build the camera with your components");
    println!("let camera = CameraBuilder::with_transport(transport)");
    println!("    .executor(executor)");
    println!("    .profile::<PtzOpticsG2>()");
    println!("    .build()");
    println!("    .await?;");
    println!();
    println!("// Use the camera - all async operations use YOUR runtime!");
    println!("camera.power_on().await?;");
    println!("camera.zoom_in().await?;");
    println!("```");

    println!("\n📚 Key Benefits:");
    println!("✅ No forced runtime dependency");
    println!("✅ Works with ANY async runtime (tokio, async-std, smol, embassy, etc.)");
    println!("✅ Can integrate with embedded async runtimes");
    println!("✅ Full control over async execution");

    println!("\n💡 Tips:");
    println!("- Start with a provided executor (rt-tokio) to test");
    println!("- Look at TokioExecutor source for implementation example");
    println!("- The spawn() method is used for background tasks");
    println!("- The timeout() method is critical for camera operations");

    // Show that we can reference the executor
    let _ = my_executor;

    println!("\n✅ Example completed successfully!");
}

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Run with: cargo run --example runtime_agnostic --features async");
    std::process::exit(1);
}
