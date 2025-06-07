//! Tests for Phase A transport implementation.

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "blocking-client"))]
    use grafton_visca::transport::Transport;
    #[cfg(feature = "async-client")]
    use grafton_visca::transport::{AsyncTcpTransport, AsyncUdpTransport};
    #[cfg(feature = "blocking-client")]
    use grafton_visca::transport::{
        BlockingAdapter, BlockingTransport, TcpTransport, Transport, UdpTransport,
    };
    use grafton_visca::{ViscaCommand, ViscaError};

    // Mock command for testing
    #[allow(dead_code)]
    struct TestCommand;

    impl ViscaCommand for TestCommand {
        fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
            Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
        }

        fn response_type(&self) -> Option<grafton_visca::ViscaResponseType> {
            None
        }

        fn command_category(&self) -> grafton_visca::timeout::CommandCategory {
            grafton_visca::timeout::CommandCategory::Quick
        }
    }

    #[test]
    fn test_transport_trait_exists() {
        // This test verifies that the Transport trait exists and is usable
        #[allow(dead_code)]
        fn accepts_transport<T: Transport>(_t: &T) {}

        // The test passes if this compiles
    }

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_blocking_transport_exists() {
        // This test verifies that BlockingTransport can be used
        struct MockBlockingTransport;

        impl BlockingTransport for MockBlockingTransport {
            fn send_command_blocking(
                &mut self,
                _command: &dyn ViscaCommand,
            ) -> Result<(), ViscaError> {
                Ok(())
            }

            fn receive_response_blocking(&mut self) -> Result<Vec<Vec<u8>>, ViscaError> {
                Ok(vec![vec![0x90, 0x50, 0xFF]])
            }
        }

        fn accepts_transport<T: Transport>(_t: &T) {}

        let transport = MockBlockingTransport;
        let adapter = BlockingAdapter(transport);

        // Verify it implements Transport
        accepts_transport(&adapter);
    }

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_udp_transport_creation() {
        // Test that UdpTransport can be created
        let result = UdpTransport::new("127.0.0.1:1234");
        assert!(result.is_ok(), "Failed to create UDP transport");
    }

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_tcp_transport_creation() {
        // Test that TcpTransport can be created (will fail to connect but that's OK)
        let result = TcpTransport::new("127.0.0.1:1234");
        // We expect this to fail since there's no server
        assert!(result.is_err(), "Expected connection to fail");
    }

    #[cfg(feature = "async-client")]
    #[tokio::test]
    async fn test_async_udp_transport_creation() {
        // Test that AsyncUdpTransport can be created
        let result = AsyncUdpTransport::new("127.0.0.1:1234").await;
        assert!(result.is_ok(), "Failed to create async UDP transport");
    }

    #[cfg(feature = "async-client")]
    #[tokio::test]
    async fn test_async_tcp_transport_creation() {
        // Test that AsyncTcpTransport can be created (will fail to connect but that's OK)
        let result = AsyncTcpTransport::new("127.0.0.1:1234").await;
        // We expect this to fail since there's no server
        assert!(result.is_err(), "Expected async TCP connection to fail");
    }
}
