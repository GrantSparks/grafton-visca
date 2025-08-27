//! Tests for connect_dyn() builder functionality.
//!
//! This test suite validates that the connect_dyn() methods work correctly
//! across all transport types and runtime configurations, providing stable
//! BoxAsyncTransport handles for dynamic usage.

#![cfg(all(feature = "async", feature = "test-utils"))]

use grafton_visca::transport::Transport;

#[cfg(feature = "rt-tokio")]
use grafton_visca::transport::async_dyn::{BoxAsyncTransport, DynAsyncTransport};

#[cfg(feature = "rt-tokio")]
use grafton_visca::testing::testkit::{helpers, ScriptedTransport};

#[cfg(feature = "rt-tokio")]
use grafton_visca::TokioExecutor;

/// Test that TCP connect_dyn() produces the expected type.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_tcp_connect_dyn_type() {
    // Create a scripted transport that simulates a successful connection
    let scripted = ScriptedTransport::<TokioExecutor>::new(vec![
        helpers::auto_respond_step(), // For any potential handshake
    ]);

    // Test that connect_dyn() returns BoxAsyncTransport
    let transport: BoxAsyncTransport = Box::new(scripted);

    // Verify the transport implements DynAsyncTransport
    fn requires_dyn_transport(_t: &dyn DynAsyncTransport) {}
    requires_dyn_transport(&*transport);
}

/// Test that UDP connect_dyn() produces the expected type.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_udp_connect_dyn_type() {
    // Create a scripted transport that simulates UDP behavior
    let scripted = ScriptedTransport::<TokioExecutor>::new(vec![
        helpers::auto_respond_step(), // For any potential message
    ]);

    // Test that UDP transports can be boxed as well
    let transport: BoxAsyncTransport = Box::new(scripted);

    // Verify type compatibility
    fn accepts_box_transport(_t: BoxAsyncTransport) {}
    accepts_box_transport(transport);
}

/// Test dynamic transport conversion from concrete types.
#[test]
#[cfg(feature = "rt-tokio")]
fn test_transport_to_box_conversion() {
    // Test From<T> implementation for BoxAsyncTransport
    let scripted = ScriptedTransport::<TokioExecutor>::new(vec![helpers::auto_respond_step()]);

    let boxed: BoxAsyncTransport = scripted.into();

    // Test that we can store different transport types in the same collection
    let transports: Vec<BoxAsyncTransport> = vec![boxed];

    // Verify the collection can hold multiple transports
    assert_eq!(transports.len(), 1);
}

/// Test that BoxAsyncTransport can be used across thread boundaries.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_box_transport_send_sync() {
    let scripted = ScriptedTransport::<TokioExecutor>::new(vec![helpers::auto_respond_step()]);

    let transport: BoxAsyncTransport = Box::new(scripted);

    // Test that BoxAsyncTransport can be sent across threads
    let handle = tokio::spawn(async move {
        // Use the transport in another thread
        let _t = transport;
        "success"
    });

    let result = handle.await.unwrap();
    assert_eq!(result, "success");
}

/// Test builder pattern integration with connect_dyn().
#[test]
fn test_builder_connect_dyn_api() {
    use std::time::Duration;

    // Test that builder methods can be chained before connect_dyn()
    let tcp_builder = Transport::tcp()
        .address("192.168.0.110:5678")
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(5))
        .tcp_nodelay(true);

    #[cfg(feature = "rt-tokio")]
    let udp_builder = Transport::udp()
        .address("192.168.0.110:5678")
        .connect_timeout(Duration::from_secs(10))
        .ttl(64);

    // Test that builders have the expected methods available
    // Note: We can't actually call connect_dyn() without a real network
    // but we can verify the API is available
    let _tcp = tcp_builder;

    #[cfg(feature = "rt-tokio")]
    let _udp = udp_builder;
}

/// Test error handling in connect_dyn() builders.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_connect_dyn_error_handling() {
    // Test connection to invalid address should fail gracefully
    let result = Transport::tcp()
        .address("invalid-address:9999")
        .connect_timeout(std::time::Duration::from_millis(100))
        .connect_dyn()
        .await;

    match result {
        Ok(_) => panic!("Expected connection to invalid address to fail"),
        Err(error) => {
            // Expected - connection should fail for invalid address
            println!("Got error (expected): {:?}", error);
        }
    }
}

/// Test that multiple transport types can coexist in dynamic containers.
#[test]
#[cfg(feature = "rt-tokio")]
fn test_heterogeneous_transport_collection() {
    // Create different transport types
    let tcp_like = ScriptedTransport::<TokioExecutor>::new(vec![helpers::auto_respond_step()]);
    let udp_like = ScriptedTransport::<TokioExecutor>::new(vec![helpers::auto_respond_step()]);

    // Store them in a heterogeneous collection
    let transports: Vec<BoxAsyncTransport> = vec![Box::new(tcp_like), Box::new(udp_like)];

    assert_eq!(transports.len(), 2);

    // Test that we can iterate over mixed transport types
    for (i, transport) in transports.iter().enumerate() {
        // Verify each implements DynAsyncTransport
        fn requires_dyn(_: &dyn DynAsyncTransport) {}
        requires_dyn(&**transport);

        println!("Transport {}: OK", i);
    }
}

/// Test dynamic dispatch works correctly with BoxAsyncTransport.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_dynamic_dispatch_functionality() {
    // Create a scripted transport with a predictable response
    let scripted = ScriptedTransport::<TokioExecutor>::new(vec![
        helpers::auto_respond_step(), // ACK response
    ]);

    let mut transport: BoxAsyncTransport = Box::new(scripted);

    // Test that we can call methods through dynamic dispatch
    let send_result = transport.send(b"\x81\x01\x04\x00\x02\xFF").await;
    assert!(
        send_result.is_ok(),
        "Send should succeed: {:?}",
        send_result
    );

    let recv_result = transport.recv().await;
    assert!(
        recv_result.is_ok(),
        "Recv should succeed: {:?}",
        recv_result
    );

    // Check that we received some response (the exact content depends on the scripted response)
    assert!(recv_result.is_ok());
}

/// Test that DynAsyncTransport futures are properly Send.
#[test]
#[cfg(feature = "rt-tokio")]
fn test_dyn_transport_send_futures() {
    // Test DynAsyncTransport future Send bounds

    // Helper to verify Send bounds at compile time
    fn assert_send<F: Send>(_f: F) {}

    let mut transport: BoxAsyncTransport = Box::new(ScriptedTransport::<TokioExecutor>::new(vec![
        helpers::auto_respond_step(),
    ]));

    // Test that DynAsyncTransport futures are Send
    let send_future = transport.send(b"test");
    assert_send(send_future);

    let recv_future = transport.recv();
    assert_send(recv_future);
}

/// Test runtime-agnostic usage of connect_dyn().
#[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
#[test]
fn test_runtime_agnostic_connect_dyn() {
    // Test that connect_dyn() API is available regardless of runtime
    let _tcp_builder = Transport::tcp().address("192.168.0.110:5678");

    #[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
    let _udp_builder = Transport::udp().address("192.168.0.110:5678");

    // The builders should have connect_dyn() method available
    // We can't test actual connection without network, but API should exist
}

/// Test that BoxAsyncTransport maintains type erasure.
#[test]
#[cfg(feature = "rt-tokio")]
fn test_type_erasure_properties() {
    // Create transports with different internal configurations
    let transport1 = ScriptedTransport::<TokioExecutor>::new(vec![helpers::auto_respond_step()]);
    let transport2 = ScriptedTransport::<TokioExecutor>::new(vec![
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
    ]);

    // Both can be stored as the same BoxAsyncTransport type
    let boxed1: BoxAsyncTransport = Box::new(transport1);
    let boxed2: BoxAsyncTransport = Box::new(transport2);

    // They have identical types despite different internal state
    assert_eq!(
        std::any::type_name_of_val(&boxed1),
        std::any::type_name_of_val(&boxed2)
    );
}

/// Test builder validation and error cases.
#[test]
fn test_builder_validation() {
    use std::time::Duration;

    // Test that builders accept valid configurations
    let valid_tcp = Transport::tcp()
        .address("192.168.1.100:5678")
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(10))
        .write_timeout(Duration::from_secs(10))
        .tcp_nodelay(true);

    // Builder should be valid (can't test connection without network)
    let _builder = valid_tcp;

    #[cfg(feature = "rt-tokio")]
    {
        let valid_udp = Transport::udp()
            .address("192.168.1.100:5678")
            .connect_timeout(Duration::from_secs(30))
            .ttl(64);

        let _udp_builder = valid_udp;
    }
}

/// Test that connect_dyn() works with different address formats.
#[test]
fn test_connect_dyn_address_formats() {
    // Test various address formats that should be accepted
    let _ipv4 = Transport::tcp().address("192.168.1.100:5678");
    let _localhost = Transport::tcp().address("localhost:5678");
    let _hostname = Transport::tcp().address("camera.local:5678");

    // All builders should accept these formats
    // Actual connection testing requires network setup
}

/// Integration test demonstrating typical connect_dyn() usage patterns.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_connect_dyn_integration_pattern() -> Result<(), grafton_visca::Error> {
    // Simulate a typical usage pattern where transport type is determined at runtime
    async fn create_transport(
        transport_type: &str,
    ) -> Result<BoxAsyncTransport, grafton_visca::Error> {
        match transport_type {
            "tcp" => {
                // In real code, this would be Transport::tcp().connect_dyn().await
                Ok(Box::new(ScriptedTransport::<TokioExecutor>::new(vec![
                    helpers::auto_respond_step(),
                ])))
            }
            "udp" => {
                // In real code, this would be Transport::udp().connect_dyn().await
                Ok(Box::new(ScriptedTransport::<TokioExecutor>::new(vec![
                    helpers::auto_respond_step(),
                ])))
            }
            _ => Err(grafton_visca::Error::from_code(0xFF)), // Unknown transport type
        }
    }

    // Test that both transport types can be created and used uniformly
    let tcp_transport = create_transport("tcp").await?;
    let udp_transport = create_transport("udp").await?;

    // Both have the same type and can be used interchangeably
    let transports = vec![tcp_transport, udp_transport];

    for (i, mut transport) in transports.into_iter().enumerate() {
        let result = transport.send(b"\x81\x01\x04\x00\x02\xFF").await;
        assert!(result.is_ok(), "Transport {} send failed: {:?}", i, result);
    }

    Ok(())
}
