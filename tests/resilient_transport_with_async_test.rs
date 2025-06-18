//! Tests for ResilientTransport with async transports (issue #58)

#[cfg(all(feature = "tokio", feature = "async-client"))]
mod tests {
    use grafton_visca::{
        command::zoom::ZoomCommand,
        transport::{
            resilient::{ResilienceConfig, ResilientTransport},
            AsyncTcpTransport, AsyncUdpTransport, Transport,
        },
        types::SocketId,
    };
    use std::sync::Arc;

    #[tokio::test]
    async fn test_resilient_transport_with_async_tcp() {
        // This test verifies that ResilientTransport can be created with AsyncTcpTransport
        // and that Clone works correctly

        // Create factory for AsyncTcpTransport
        let factory = || async {
            AsyncTcpTransport::new("192.168.1.100:52381")
                .await
                .map_err(|e| grafton_visca::Error::Io(e))
        };

        // Create initial transport
        let tcp = match AsyncTcpTransport::new("192.168.1.100:52381").await {
            Ok(transport) => transport,
            Err(_) => {
                // Skip test if we can't connect (no camera available)
                println!("Skipping test - no camera available at 192.168.1.100:52381");
                return;
            }
        };

        // Create resilient transport - this should compile now that AsyncTcpTransport implements Clone
        let resilient = ResilientTransport::new(
            tcp,
            move || {
                let fut = factory();
                futures::executor::block_on(fut)
            },
            ResilienceConfig::default(),
        );

        // Test that we can clone the resilient transport
        let _cloned = resilient.clone();

        // Verify both implement Transport trait
        fn assert_transport<T: Transport>(_t: &T) {}
        assert_transport(&resilient);
        assert_transport(&_cloned);
    }

    #[tokio::test]
    async fn test_resilient_transport_with_async_udp() {
        // This test verifies that ResilientTransport can be created with AsyncUdpTransport
        // and that Clone works correctly

        // Create factory for AsyncUdpTransport
        let factory = || async {
            AsyncUdpTransport::new("192.168.1.100:52381")
                .await
                .map_err(|e| grafton_visca::Error::Io(e))
        };

        // Create initial transport
        let udp = match AsyncUdpTransport::new("192.168.1.100:52381").await {
            Ok(transport) => transport,
            Err(_) => {
                // Skip test if we can't create UDP socket
                println!("Skipping test - cannot create UDP socket");
                return;
            }
        };

        // Create resilient transport - this should compile now that AsyncUdpTransport implements Clone
        let resilient = ResilientTransport::new(
            udp,
            move || {
                let fut = factory();
                futures::executor::block_on(fut)
            },
            ResilienceConfig::default(),
        );

        // Test that we can clone the resilient transport
        let _cloned = resilient.clone();

        // Verify both implement Transport trait
        fn assert_transport<T: Transport>(_t: &T) {}
        assert_transport(&resilient);
        assert_transport(&_cloned);
    }

    #[tokio::test]
    async fn test_resilient_transport_concurrent_usage() {
        // This test verifies that cloned transports can be used concurrently

        // Create initial transport
        let tcp = match AsyncTcpTransport::new("192.168.1.100:52381").await {
            Ok(transport) => transport,
            Err(_) => {
                println!("Skipping test - no camera available");
                return;
            }
        };

        // Create resilient transport
        let resilient = Arc::new(ResilientTransport::new(
            tcp,
            || {
                futures::executor::block_on(async {
                    AsyncTcpTransport::new("192.168.1.100:52381")
                        .await
                        .map_err(|e| grafton_visca::Error::Io(e))
                })
            },
            ResilienceConfig::default(),
        ));

        // Clone for concurrent usage
        let resilient1 = resilient.clone();
        let resilient2 = resilient.clone();

        // Spawn concurrent tasks
        let handle1 = tokio::spawn(async move {
            let mut transport = (*resilient1).clone();
            let command = ZoomCommand::ZoomInStandard;
            let _ = transport.send_command(&command, SocketId::SOCKET_0).await;
        });

        let handle2 = tokio::spawn(async move {
            let mut transport = (*resilient2).clone();
            let command = ZoomCommand::ZoomOutStandard;
            let _ = transport.send_command(&command, SocketId::SOCKET_0).await;
        });

        // Wait for both tasks to complete
        let _ = tokio::join!(handle1, handle2);
    }
}

#[cfg(not(all(feature = "tokio", feature = "async-client")))]
fn main() {
    println!("Tests require tokio and async-client features");
}
