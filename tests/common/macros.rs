#![allow(missing_docs)]
//! Test macros for common assertion patterns.
//!
//! These macros help reduce boilerplate in tests and provide better error messages
//! than simple unwrap() calls.

/// Assert that command bytes match the expected bytes.
///
/// Provides better error output with hex formatting for debugging.
///
/// # Example
/// ```no_run
/// # use grafton_visca::command::EncodeVisca;
/// # let command = unimplemented!();
/// assert_command_bytes!(command, [0x81, 0x01, 0x06, 0x04, 0xFF]);
/// ```
#[macro_export]
macro_rules! assert_command_bytes {
    ($cmd:expr, $expected:expr) => {{
        let cmd = $cmd;
        let bytes = cmd
            .try_into_vec()
            .unwrap_or_else(|e| panic!("Failed to convert command to bytes: {:?}", e));
        let expected = &$expected[..];
        if bytes != expected {
            panic!(
                "Command bytes mismatch\nExpected: {:02X?}\nActual:   {:02X?}",
                expected, bytes
            );
        }
    }};
}

/// Assert that a Result is Ok and matches the expected response.
///
/// Provides context about what response was expected vs actual.
///
/// # Example
/// ```no_run
/// # use grafton_visca::Response;
/// # let result: Result<Response, grafton_visca::Error> = Ok(Response::CmdAck);
/// assert_response_ok!(result, Response::CmdAck);
/// ```
#[macro_export]
macro_rules! assert_response_ok {
    ($result:expr) => {{
        match $result {
            Ok(response) => response,
            Err(e) => panic!("Expected Ok response, got error: {:?}", e),
        }
    }};
    ($result:expr, $expected:expr) => {{
        match $result {
            Ok(response) => {
                assert_eq!(
                    response, $expected,
                    "Response mismatch\nExpected: {:?}\nActual:   {:?}",
                    $expected, response
                );
                response
            }
            Err(e) => panic!("Expected Ok({:?}), got error: {:?}", $expected, e),
        }
    }};
    ($result:expr, $msg:expr) => {{
        match $result {
            Ok(response) => response,
            Err(e) => panic!("{}: {:?}", $msg, e),
        }
    }};
}

/// Assert that a Result is an error and optionally check the error type.
///
/// # Example
/// ```no_run
/// # use grafton_visca::{Response, Error};
/// # let result: Result<Response, Error> = Err(Error::Timeout);
/// assert_response_err!(result);
/// assert_response_err!(result, Error::Timeout);
/// ```
#[macro_export]
macro_rules! assert_response_err {
    ($result:expr) => {{
        match $result {
            Ok(response) => panic!("Expected error, got Ok({:?})", response),
            Err(e) => e,
        }
    }};
    ($result:expr, $expected_err:pat) => {{
        match $result {
            Ok(response) => panic!("Expected error, got Ok({:?})", response),
            Err($expected_err) => {}
            Err(e) => panic!(
                "Expected error {:?}, got {:?}",
                stringify!($expected_err),
                e
            ),
        }
    }};
}

/// Assert that an inquiry response matches the expected variant and extract its data.
///
/// # Example
/// ```no_run
/// # use grafton_visca::{Response, InquiryResponse};
/// # let response = Response::InquiryResponse(InquiryResponse::Power { on: true });
/// let on = assert_inquiry_response!(response, Power { on });
/// assert!(on);
/// ```
#[macro_export]
macro_rules! assert_inquiry_response {
    ($response:expr, $variant:ident { $($field:ident),+ }) => {{
        match $response {
            $crate::Response::InquiryResponse($crate::InquiryResponse::$variant { $($field),+ }) => {
                ($($field),+)
            }
            _ => panic!(
                "Expected InquiryResponse::{} {{ {} }}, got {:?}",
                stringify!($variant),
                stringify!($($field),+),
                $response
            ),
        }
    }};
    ($response:expr, $variant:ident) => {{
        match $response {
            $crate::Response::InquiryResponse($crate::InquiryResponse::$variant) => {}
            _ => panic!(
                "Expected InquiryResponse::{}, got {:?}",
                stringify!($variant),
                $response
            ),
        }
    }};
}

/// Create a test client with better error messages.
///
/// # Example
/// ```no_run
/// # #[cfg(not(feature = "async"))]
/// # {
/// let client = create_test_client!(udp, "127.0.0.1:1234");
/// let client = create_test_client!(tcp, "127.0.0.1:5678");
/// # }
/// ```
#[cfg(not(feature = "async"))]
#[macro_export]
macro_rules! create_test_client {
    (udp, $addr:expr) => {{
        $crate::Client::connect_udp($addr)
            .unwrap_or_else(|e| panic!("Failed to create UDP client at {}: {:?}", $addr, e))
    }};
    (tcp, $addr:expr) => {{
        $crate::Client::connect_tcp($addr)
            .unwrap_or_else(|e| panic!("Failed to create TCP client at {}: {:?}", $addr, e))
    }};
}

/// Assert that sending a command succeeds.
///
/// # Example
/// ```no_run
/// # #[cfg(not(feature = "async"))]
/// # {
/// # use grafton_visca::{Client, command::PowerCommand, command::power::Power};
/// # let client = Client::connect_udp("127.0.0.1:1234").unwrap();
/// assert_send_ok!(client, PowerCommand { power: Power::On });
/// # }
/// ```
#[cfg(not(feature = "async"))]
#[macro_export]
macro_rules! assert_send_ok {
    ($client:expr, $command:expr) => {{
        match $client.send(&$command) {
            Ok(response) => response,
            Err(e) => panic!("Failed to send command {:?}: {:?}", stringify!($command), e),
        }
    }};
    ($client:expr, $command:expr, $expected:expr) => {{
        match $client.send(&$command) {
            Ok(response) => {
                assert_eq!(
                    response,
                    $expected,
                    "Command {:?} returned unexpected response",
                    stringify!($command)
                );
                response
            }
            Err(e) => panic!("Failed to send command {:?}: {:?}", stringify!($command), e),
        }
    }};
}

#[cfg(test)]
mod tests {
    use crate::assert_command_bytes;
    use grafton_visca::command::EncodeVisca;
    use grafton_visca::Error;

    #[test]
    fn test_assert_command_bytes_macro() {
        struct TestCommand;
        impl EncodeVisca for TestCommand {
            type Response = ();
            const MAX_SIZE: usize = 5;

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
                let bytes = [0x81, 0x01, 0x06, 0x04, 0xFF];
                buffer[..5].copy_from_slice(&bytes);
                Ok(5)
            }

            fn response_type(&self) -> Option<grafton_visca::command::ResponseType> {
                None
            }
        }

        let cmd = TestCommand;
        assert_command_bytes!(cmd, [0x81, 0x01, 0x06, 0x04, 0xFF]);
    }

    #[test]
    #[should_panic(expected = "Command bytes mismatch")]
    fn test_assert_command_bytes_failure() {
        struct TestCommand;
        impl EncodeVisca for TestCommand {
            type Response = ();
            const MAX_SIZE: usize = 5;

            fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
                let bytes = [0x81, 0x01, 0x06, 0x05, 0xFF];
                buffer[..5].copy_from_slice(&bytes);
                Ok(5)
            }

            fn response_type(&self) -> Option<grafton_visca::command::ResponseType> {
                None
            }
        }

        let cmd = TestCommand;
        assert_command_bytes!(cmd, [0x81, 0x01, 0x06, 0x04, 0xFF]);
    }
}
