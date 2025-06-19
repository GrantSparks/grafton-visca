#![allow(missing_docs)]
//! Tests for Phase A transport implementation.

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "blocking-client"))]
    use grafton_visca::transport::Transport;
    #[cfg(feature = "blocking-client")]
    use grafton_visca::transport::{BlockingAdapter, BlockingTransport, Transport};
    use grafton_visca::{Command, Error};

    // Mock command for testing
    #[allow(dead_code)]
    struct TestCommand;

    impl Command for TestCommand {
        fn to_bytes(&self) -> Result<Vec<u8>, Error> {
            Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
        }

        fn response_type(&self) -> Option<grafton_visca::command::ResponseType> {
            None
        }

        fn command_category(&self) -> grafton_visca::timeout::CommandCategory {
            grafton_visca::timeout::CommandCategory::Quick
        }
    }

    #[test]
    const fn test_transport_trait_exists() {
        // This test verifies that the Transport trait exists and is usable
        #[allow(dead_code)]
        const fn accepts_transport<T: Transport>(_t: &T) {}

        // The test passes if this compiles
    }

    #[cfg(feature = "blocking-client")]
    #[test]
    #[allow(clippy::missing_const_for_fn)] // Test functions cannot be const
    fn test_blocking_transport_exists() {
        // This test verifies that BlockingTransport can be used
        struct MockBlockingTransport;

        impl BlockingTransport for MockBlockingTransport {
            fn send_command_blocking(
                &mut self,
                _command: &dyn Command,
            ) -> Result<grafton_visca::Response, Error> {
                Ok(grafton_visca::Response::Completion)
            }
        }

        const fn accepts_transport<T: Transport>(_t: &T) {}

        let transport = MockBlockingTransport;
        let adapter = BlockingAdapter(transport);

        // Verify it implements Transport
        accepts_transport(&adapter);
    }

    // Note: TCP and UDP transport tests have been removed since concrete
    // transport implementations are now provided as examples rather than
    // being part of the core library. Users should implement their own
    // transports based on the Transport trait.
}
