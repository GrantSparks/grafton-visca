#![allow(missing_docs)]
//! Tests for Phase B unified client implementation.

#[cfg(all(test, any(feature = "blocking-client", feature = "async-client")))]
mod tests {
    use grafton_visca::Client;

    #[cfg(any(
        all(feature = "blocking-client", feature = "async-client"),
        feature = "async-client"
    ))]
    use grafton_visca::command::{Power, PowerCommand};

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_blocking_client_creation() {
        // Test that we can create blocking clients
        let udp_result = Client::connect_udp("127.0.0.1:1234");
        let tcp_result = Client::connect_tcp("127.0.0.1:1234");

        // UDP should succeed (no connection needed)
        assert!(udp_result.is_ok());
        // TCP should fail (no server)
        assert!(tcp_result.is_err());
    }

    #[cfg(feature = "async-client")]
    #[tokio::test]
    async fn test_async_client_creation() {
        // Test that we can create async clients
        let udp_result = Client::connect_udp_async("127.0.0.1:1234").await;
        let tcp_result = Client::connect_tcp_async("127.0.0.1:1234").await;

        // UDP should succeed (no connection needed)
        assert!(udp_result.is_ok());
        // TCP should fail (no server)
        assert!(tcp_result.is_err());
    }

    #[cfg(all(feature = "blocking-client", feature = "async-client"))]
    #[test]
    fn test_blocking_facade_outside_runtime() {
        // Test blocking façade when called outside a Tokio runtime
        let client = Client::connect_udp("127.0.0.1:1234").unwrap();
        let cmd = PowerCommand { power: Power::On };

        // This should create its own runtime internally
        let result = client.send(&cmd);

        // Should fail because no camera is connected
        assert!(result.is_err());
    }

    #[cfg(all(feature = "blocking-client", feature = "async-client"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_blocking_facade_inside_runtime() {
        // Test blocking façade when called inside a Tokio runtime
        let client = Client::connect_udp("127.0.0.1:1234").unwrap();
        let cmd = PowerCommand { power: Power::On };

        // This should use the existing runtime
        let result = client.send(&cmd);

        // Should fail because no camera is connected
        assert!(result.is_err());
    }

    #[cfg(feature = "async-client")]
    #[tokio::test]
    async fn test_async_send() {
        let client = Client::connect_udp_async("127.0.0.1:1234").await.unwrap();
        let cmd = PowerCommand { power: Power::On };

        // Test async send
        let result = client.send_async(&cmd).await;

        // Should fail because no camera is connected
        assert!(result.is_err());
    }

    #[test]
    fn test_client_is_moveable() {
        #[cfg(feature = "blocking-client")]
        {
            let client = Client::connect_udp("127.0.0.1:1234").unwrap();
            let moved = client;
            // This just verifies that client can be moved
            drop(moved);
        }
    }

    #[cfg(feature = "async-client")]
    #[tokio::test]
    async fn test_health_check() {
        let client = Client::connect_udp_async("127.0.0.1:1234").await.unwrap();

        // Health check should fail (no camera)
        let is_healthy = client.is_healthy().await.unwrap();
        assert!(!is_healthy);
    }
}
