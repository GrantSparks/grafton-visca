//! Inquiry commands for VISCA cameras.
//!
//! This module provides commands for querying the current state of various camera settings,
//! including power status, position, zoom, focus, and other camera parameters.

macro_rules! re_export_builtin_inquiries {
    (
        queryable {
            $(
                $(#[$meta:meta])*
                $struct:ident => {
                    const $bytes_const:ident = [$($byte:expr),+ $(,)?];
                    kind: $kind:ident $body:tt;
                    decode: |$payload:ident| $decode_body:block;
                    $(profile_decode: $profile_decode:ident;)?
                    $(timeout: $timeout:expr;)?
                    response: $response:ident;
                    query: $query:expr;
                    vendor_specific: $vendor_specific:expr;
                    rationale: $rationale:expr;
                    typed: $typed:tt;
                }
            )*
        }
        decode_only { $($decode_entries:tt)* }
        accessors { $($accessor_groups:tt)* }
    ) => {
        pub use super::inquiry_structs::{
            $(
                $struct,
            )*
        };
    };
}

super::inquiry_structs::builtin_inquiry_table!(re_export_builtin_inquiries);
