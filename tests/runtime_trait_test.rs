//! Tests for the Runtime trait implementations.
//!
//! This test file verifies that:
//! 1. Each runtime implementation properly binds executor and transport types
//! 2. Type-safe pairing prevents mismatched runtime combinations
//! 3. The TransportHandle enum correctly dispatches to the underlying transport

#![cfg(feature = "mode-async")]

#[cfg(feature = "runtime-tokio")]
mod tokio_runtime_tests {
    use tokio::time::timeout;

    use std::time::{Duration, Instant};

    use grafton_visca::{
        runtime::{Runtime, TokioRuntime},
        Error, Executor,
    };

    #[tokio::test]
    async fn test_tokio_runtime_creation() {
        // Test creating runtime from current Tokio runtime
        let runtime = TokioRuntime::from_current();
        assert!(runtime.is_ok(), "Should create TokioRuntime from current");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_tokio_runtime_tcp_connection() {
        let runtime = TokioRuntime::from_current().unwrap();

        // This will fail to connect but tests the type system works
        // Wrap in a timeout to prevent hanging
        let result = timeout(
            Duration::from_secs(2),
            runtime.connect_tcp(
                "192.0.2.1:5678", // TEST-NET-1 address that won't connect
                Default::default(),
            ),
        )
        .await;

        // We expect either a timeout or connection error
        match result {
            Ok(inner_result) => {
                assert!(inner_result.is_err());
            }
            Err(_) => {
                // Timed out - this is also acceptable for this test
                // The main point is to verify type system compilation
            }
        }
    }

    #[tokio::test]
    async fn test_tokio_runtime_udp_connection() {
        let runtime = TokioRuntime::from_current().unwrap();

        // UDP might not fail immediately for non-existent addresses
        // This test is more about verifying the type system works
        let _result = runtime
            .connect_udp(
                "192.0.2.1:1259", // TEST-NET-1 address
                Default::default(),
            )
            .await;

        // The important part is that this compiles with the correct types
    }

    #[tokio::test]
    async fn test_transport_handle_type_safety() {
        use grafton_visca::runtime::TransportHandle;
        // This test verifies that TransportHandle is properly parameterized
        let _handle: Result<TransportHandle<TokioRuntime>, Error>;

        // And we can create enum variants (though we can't actually connect)
        // This is a compile-time test more than runtime
    }

    #[tokio::test]
    async fn test_runtime_executor_delegation() {
        let runtime = TokioRuntime::from_current().unwrap();

        // Test that executor methods are properly delegated
        let future = async { 42 };
        let result = runtime.timeout(Duration::from_millis(100), future).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        // Test sleep delegation
        let start = Instant::now();
        runtime.sleep(Duration::from_millis(10)).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
    }
}

#[cfg(feature = "runtime-async-std")]
mod async_std_runtime_tests {
    use std::time::{Duration, Instant};

    use grafton_visca::{
        runtime::{AsyncStdRuntime, Runtime},
        Executor,
    };

    #[async_std::test]
    async fn test_async_std_runtime_creation() {
        let runtime = AsyncStdRuntime::new();
        let _ = runtime;
    }

    #[async_std::test]
    async fn test_async_std_runtime_tcp_connection() {
        let runtime = AsyncStdRuntime::new();

        // This will fail to connect but tests the type system works
        let result = runtime
            .connect_tcp(
                "192.0.2.1:5678", // TEST-NET-1 address that won't connect
                Default::default(),
            )
            .await;

        assert!(result.is_err());
    }

    #[async_std::test]
    async fn test_async_std_runtime_executor_delegation() {
        let runtime = AsyncStdRuntime::new();

        // Test that executor methods are properly delegated
        let future = async { 84 };
        let result = runtime.timeout(Duration::from_millis(100), future).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 84);

        // Test sleep delegation
        let start = Instant::now();
        runtime.sleep(Duration::from_millis(10)).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
    }
}

// NOTE: Smol tests are disabled when Tokio is also enabled to avoid
// runtime conflicts. See issue #394 for details.
#[cfg(all(feature = "runtime-smol", not(feature = "runtime-tokio")))]
mod smol_runtime_tests {
    use grafton_visca::runtime::{Runtime, SmolRuntime};

    fn run_smol<F: std::future::Future>(f: F) -> F::Output {
        smol::block_on(f)
    }

    #[test]
    fn test_smol_runtime_creation() {
        run_smol(async {
            let runtime = SmolRuntime::new();
            let _ = runtime;
        });
    }

    #[test]
    fn test_smol_runtime_tcp_connection() {
        run_smol(async {
            let runtime = SmolRuntime::new();

            // This will fail to connect but tests the type system works
            let result = runtime
                .connect_tcp(
                    "192.0.2.1:5678", // TEST-NET-1 address that won't connect
                    Default::default(),
                )
                .await;

            // We expect connection to fail but the types should compile
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_smol_runtime_executor_delegation() {
        run_smol(async {
            use std::time::Duration;

            use grafton_visca::Executor;

            let runtime = SmolRuntime::new();

            // Test that executor methods are properly delegated
            let future = async { 126 };
            let result = runtime.timeout(Duration::from_millis(100), future).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 126);

            // Test sleep delegation
            let start = std::time::Instant::now();
            runtime.sleep(Duration::from_millis(10)).await;
            let elapsed = start.elapsed();
            assert!(elapsed >= Duration::from_millis(10));
        });
    }
}

// Integration tests for transport connect paths (issue #472)
// These tests verify that UdpTransport::connect_with_config works correctly
// with the new single end-to-end deadline behavior.
#[cfg(feature = "runtime-tokio")]
mod tokio_transport_connect_tests {
    use std::time::Duration;

    use grafton_visca::{
        runtime_adapters::tokio::UdpTransport, transport::builder::TransportConfig,
    };

    /// Test that UDP transport connection completes successfully with a local address.
    ///
    /// This is a smoke test that verifies the happy path for UDP connection
    /// using the single end-to-end deadline pattern.
    #[tokio::test]
    async fn test_udp_transport_connect_with_config_success() {
        // Create a local UDP socket to connect to
        let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();

        let config = TransportConfig {
            connect_timeout: Duration::from_secs(5),
            ..Default::default()
        };

        // This should complete successfully
        let result = UdpTransport::connect_with_config(&addr.to_string(), config).await;
        assert!(
            result.is_ok(),
            "UDP transport should connect successfully to local address"
        );
    }

    /// Test that timeout is respected for UDP connections.
    ///
    /// Uses a short timeout with a non-routable address to verify timeout behavior.
    #[tokio::test]
    async fn test_udp_transport_connect_timeout() {
        use std::time::Instant;

        let config = TransportConfig {
            connect_timeout: Duration::from_millis(100),
            ..Default::default()
        };

        let start = Instant::now();
        // Use a TEST-NET address that won't route
        let result = UdpTransport::connect_with_config("192.0.2.1:9", config).await;
        let elapsed = start.elapsed();

        // The operation should complete (either successfully or with timeout)
        // within a reasonable margin of the configured timeout.
        // Note: UDP connect may succeed even for non-routable addresses on some systems
        // since UDP is connectionless, so we just verify timing is reasonable.
        assert!(
            elapsed < Duration::from_secs(5),
            "Operation should complete within reasonable time"
        );

        // If it failed, it should be a timeout or another expected error
        if result.is_err() {
            // Expected - the address is non-routable
        }
    }
}

#[cfg(feature = "runtime-async-std")]
mod async_std_transport_connect_tests {
    use std::time::Duration;

    use grafton_visca::{
        runtime_adapters::async_std::UdpTransport, transport::builder::TransportConfig,
    };

    /// Test that UDP transport connection completes successfully with a local address.
    #[async_std::test]
    async fn test_udp_transport_connect_with_config_success() {
        // Create a local UDP socket to connect to
        let server = async_std::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .unwrap();
        let addr = server.local_addr().unwrap();

        let config = TransportConfig {
            connect_timeout: Duration::from_secs(5),
            ..Default::default()
        };

        let result = UdpTransport::connect_with_config(&addr.to_string(), config).await;
        assert!(
            result.is_ok(),
            "UDP transport should connect successfully to local address"
        );
    }
}

#[cfg(all(feature = "runtime-smol", not(feature = "runtime-tokio")))]
mod smol_transport_connect_tests {
    use std::time::Duration;

    use grafton_visca::{
        runtime_adapters::smol::UdpTransport, transport::builder::TransportConfig,
    };

    fn run_smol<F: std::future::Future>(f: F) -> F::Output {
        smol::block_on(f)
    }

    /// Test that UDP transport connection completes successfully with a local address.
    #[test]
    fn test_udp_transport_connect_with_config_success() {
        run_smol(async {
            // Create a local UDP socket to connect to
            let server = smol::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let addr = server.local_addr().unwrap();

            let config = TransportConfig {
                connect_timeout: Duration::from_secs(5),
                ..Default::default()
            };

            let result = UdpTransport::connect_with_config(&addr.to_string(), config).await;
            assert!(
                result.is_ok(),
                "UDP transport should connect successfully to local address"
            );
        });
    }
}

// Compile-time tests to ensure type safety
#[cfg(all(feature = "runtime-tokio", feature = "runtime-async-std"))]
mod compile_time_safety_tests {
    use std::any::TypeId;

    use grafton_visca::runtime::{AsyncStdRuntime, TokioRuntime};

    // This function should NOT compile if uncommented, proving type safety:
    // fn mismatched_runtime_transport() {
    //     // This would try to create a TransportHandle<TokioRuntime> with an async-std transport
    //     // which should be impossible
    //     let _bad: TransportHandle<TokioRuntime> = TransportHandle::Tcp(
    //         // Can't put an AsyncStdRuntime::TcpTransport here!
    //         unimplemented!()
    //     );
    // }

    #[test]
    fn test_runtime_types_are_distinct() {
        // Verify that runtime types are distinct at compile time
        fn is_tokio_runtime<R: grafton_visca::runtime::Runtime>() -> bool {
            TypeId::of::<R>() == TypeId::of::<TokioRuntime>()
        }

        assert!(is_tokio_runtime::<TokioRuntime>());
        assert!(!is_tokio_runtime::<AsyncStdRuntime>());
    }
}

// Test that TransportHandle properly implements AsyncTransport
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn test_transport_handle_async_transport_impl() {
    use grafton_visca::{
        runtime::{TokioRuntime, TransportHandle},
        transport::AsyncTransport,
    };

    // We can't actually create a real transport without a connection,
    // but we can verify the trait is implemented
    fn assert_async_transport<T: AsyncTransport>() {}

    // This should compile, proving TransportHandle<R> implements AsyncTransport
    assert_async_transport::<TransportHandle<TokioRuntime>>();
}
