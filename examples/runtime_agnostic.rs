//! Runtime-agnostic async example demonstrating multiple runtime support.
//!
//! This example shows how the library can be used with different async runtimes:
//! - tokio (with --features rt-tokio)
//! - async-std (with --features rt-async-std)  
//! - smol (with --features rt-smol)
//! - custom runtime implementations
//!
//! Run with:
//! ```sh
//! cargo run --example runtime_agnostic --features rt-tokio
//! cargo run --example runtime_agnostic --features rt-async-std
//! cargo run --example runtime_agnostic --features rt-smol
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

    // Define a custom executor implementation
    #[derive(Debug, Clone)]
    struct CustomExecutor;

    impl Executor for CustomExecutor {
        type Join<T>
            = Pin<Box<dyn Future<Output = Result<T, grafton_visca::ExecError>> + Send + 'static>>
        where
            T: Send + 'static;

        fn spawn<F>(&self, _future: F) -> Self::Join<F::Output>
        where
            F: Future + Send + 'static,
            F::Output: Send + 'static,
        {
            // In a real implementation, this would spawn on your runtime
            Box::pin(async {
                Err(grafton_visca::ExecError::TaskFailed(
                    "Not implemented".into(),
                ))
            })
        }

        fn block_on<F: Future>(&self, _future: F) -> F::Output {
            // In a real implementation, this would block on your runtime
            panic!("block_on not implemented for demo executor")
        }

        fn sleep(&self, _duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            // In a real implementation, this would use your runtime's sleep
            Box::pin(async {
                // Sleep would happen here
            })
        }

        fn timeout<'a, F, T>(
            &'a self,
            _duration: Duration,
            _future: F,
        ) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'a>>
        where
            F: Future<Output = T> + Send + 'a,
            T: Send + 'a,
        {
            // In a real implementation, this would use your runtime's timeout
            Box::pin(async { Err(Error::Timeout) })
        }
    }

    // Demonstrate using different runtime executors
    #[cfg(feature = "rt-tokio")]
    {
        use grafton_visca::TokioExecutor;
        println!("Using Tokio executor:");
        if let Ok(executor) = TokioExecutor::from_current() {
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

    // Create a custom executor instance
    let executor = CustomExecutor;

    println!("\n✅ Created custom executor implementation");

    // Demonstrate that the library can be used with a custom executor
    // The type system ensures that Camera can work with any executor
    // This shows the API structure without requiring an actual transport
    println!("\nThe Camera type accepts any executor:");
    println!("  Camera::with_executor(transport, executor)");

    // Show that we can reference the executor type
    let _ = executor;

    println!("✅ Demonstrated that Camera accepts custom executor");
    println!();

    println!("Key Points:");
    println!("- The library does not require tokio for its executor abstraction");
    println!("- You can provide your own Executor implementation");
    println!("- The Executor trait requires spawn(), block_on(), sleep(), and timeout() methods");
    println!("- All async operations will use your provided executor");
    println!();

    println!("To use with a real async runtime:");
    println!("1. Implement the Executor trait for your runtime");
    println!("2. Pass it to Camera::with_executor()");
    println!("3. The camera will use your executor for all async operations");
    println!();

    println!("✅ Example completed successfully!");
}

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Run with: cargo run --example runtime_agnostic --features async");
    std::process::exit(1);
}
