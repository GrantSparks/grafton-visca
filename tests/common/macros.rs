#![allow(missing_docs)]
//! Test macros for common assertion patterns.

const VISCA_TERMINATOR: u8 = 0xFF;

#[macro_export]
macro_rules! assert_command_bytes {
    ($cmd:expr, $expected:expr) => {{
        let cmd = $cmd;
        let bytes = cmd
            .try_into_vec()
            .unwrap_or_else(|e| panic!("Failed to convert command to bytes: {e:?}"));
        let expected = &$expected[..];
        if bytes != expected {
            panic!(
                "Command bytes mismatch\nExpected: {:02X?}\nActual:   {:02X?}",
                expected, bytes
            );
        }
    }};
}

#[macro_export]
macro_rules! assert_response_ok {
    ($result:expr) => {{
        match $result {
            Ok(response) => response,
            Err(e) => panic!("Expected Ok response, got error: {e:?}"),
        }
    }};
    ($result:expr, $expected:expr) => {{
        match $result {
            Ok(response) => {
                assert_eq!(
                    response, $expected,
                    "ViscaResponse mismatch\nExpected: {:?}\nActual:   {:?}",
                    $expected, response
                );
                response
            }
            Err(e) => panic!("Expected Ok({:?}), got error: {e:?}", $expected),
        }
    }};
    ($result:expr, $msg:expr) => {{
        match $result {
            Ok(response) => response,
            Err(e) => panic!("{}: {e:?}", $msg),
        }
    }};
}

#[macro_export]
macro_rules! assert_response_err {
    ($result:expr) => {{
        match $result {
            Ok(response) => panic!("Expected error, got Ok({response:?})"),
            Err(e) => e,
        }
    }};
    ($result:expr, $expected_err:pat) => {{
        match $result {
            Ok(response) => panic!("Expected error, got Ok({response:?})"),
            Err($expected_err) => {}
            Err(e) => panic!(
                "Expected error {:?}, got {:?}",
                stringify!($expected_err),
                e
            ),
        }
    }};
}

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

#[cfg(test)]
mod tests {}
