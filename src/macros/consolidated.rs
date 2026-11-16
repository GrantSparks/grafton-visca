//! Crate-private fixed-body and single-parameter built-in encoders.
//!
//! Bodies omit the target address and terminator. The shared frame writer
//! supplies both; parameter expressions are evaluated exactly once.

/// Generate a built-in encoder with a fixed body or one bounded parameter.
/// Unit commands also receive `Default` and a `const fn new()`.
macro_rules! visca_command {
    // Simple command without parameters
    (
        $(#[$meta:meta])*
        pub struct $name:ident;
        bytes = $bytes:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, Default)]
        pub struct $name;

        impl $name {
            /// Creates the command.
            #[must_use]
            pub const fn new() -> Self {
                Self
            }
        }

        impl $crate::command::encode::WireEncode for $name {
            fn write_into(
                &self,
                camera_id: $crate::CameraId,
                buffer: &mut [u8],
            ) -> ::core::result::Result<usize, $crate::Error> {
                $crate::command::bytes::write_frame(camera_id, &[&$bytes], buffer)
            }
        }
    };

    // Command with parameters; `max_param_size` makes the encoding capacity
    // explicit.
    (
        $(#[$meta:meta])*
        pub struct $name:ident { $($field:ident : $ftype:ty),* $(,)? };
        prefix = $prefix:expr;
        param = $param_expr:expr;
        max_param_size = $max_param_size:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(/// The parameter value.
            pub $field: $ftype,)*
        }

        impl $crate::command::encode::WireEncode for $name {
            fn write_into(
                &self,
                camera_id: $crate::CameraId,
                buffer: &mut [u8],
            ) -> ::core::result::Result<usize, $crate::Error> {
                let Self { $($field),* } = self;
                let params: $crate::macros::param::ParamBuf<$max_param_size> =
                    $crate::macros::param::IntoParamBuf::<$max_param_size>::encode_param($param_expr);
                $crate::command::bytes::write_frame(
                    camera_id,
                    &[&$prefix, params.as_slice()],
                    buffer,
                )
            }
        }
    };
}

pub(crate) use visca_command;

/// Generates a boolean register's encoder and its shared reply convention.
/// Existing constructors and getters may be declared beside the invocation.
macro_rules! on_off_command {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(#[$field_meta:meta])*
            $field_visibility:vis $field:ident: bool $(,)?
        };
        prefix = $prefix:expr;
        convention = $convention:expr;
    ) => {
        $(#[$meta])*
        pub struct $name {
            $(#[$field_meta])*
            $field_visibility $field: bool,
        }

        impl $name {
            /// The sourced convention used by commands and inquiry replies.
            pub(crate) const CONVENTION: $crate::wire::BoolConvention = $convention;
        }

        impl $crate::command::encode::WireEncode for $name {
            fn write_into(
                &self,
                camera_id: $crate::CameraId,
                buffer: &mut [u8],
            ) -> $crate::Result<usize> {
                $crate::command::bytes::FrameWriter::new(camera_id, buffer)
                    .bytes(&$prefix)
                    .byte(Self::CONVENTION.to_wire(self.$field))
                    .finish()
            }
        }
    };
    ($(#[$doc:meta])* $name:ident, $prefix:expr) => {
        $crate::macros::consolidated::on_off_command! {
            $(#[$doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(deny_unknown_fields))]
            #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
            pub struct $name { enabled: bool };
            prefix = $prefix;
            convention = $crate::wire::BoolConvention::OnIs02;
        }

        impl $name {
            /// Creates an enable or disable command.
            #[must_use]
            pub const fn new(enabled: bool) -> Self {
                Self { enabled }
            }

            /// Returns the requested enablement state.
            #[must_use]
            #[cfg(test)]
            pub const fn enabled(self) -> bool {
                self.enabled
            }
        }
    };
}

pub(crate) use on_off_command;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::{camera_id::CameraId, command::encode::WireEncode};
    use std::cell::Cell;

    // Counter to track parameter evaluation - use thread-local to avoid interference
    thread_local! {
        static EVAL_COUNTER: Cell<usize> = const { Cell::new(0) };
    }

    fn get_param_with_side_effect(value: u8) -> u8 {
        EVAL_COUNTER.with(|c| c.set(c.get() + 1));
        value
    }

    fn reset_counter() {
        EVAL_COUNTER.with(|c| c.set(0));
    }

    fn get_counter() -> usize {
        EVAL_COUNTER.with(|c| c.get())
    }

    const CONSTANT_BODY: [u8; 4] = [0x01, 0x04, 0x00, 0x03];
    visca_command! {
        /// Unit command with a constant array body.
        pub struct FixedBody;
        bytes = CONSTANT_BODY;
    }
    const FIXED: FixedBody = FixedBody::new();

    #[test]
    fn fixed_body_preserves_const_default_and_checked_capacity() {
        fn default_value<T: Default>() -> T {
            T::default()
        }
        let mut bytes = [0; 6];
        assert_eq!(
            FIXED
                .write_into(CameraId::CAMERA_1, &mut bytes)
                .expect("fixed body"),
            6
        );
        assert_eq!(bytes, [0x81, 0x01, 0x04, 0x00, 0x03, 0xff]);
        assert_eq!(
            default_value::<FixedBody>()
                .write_into(CameraId::CAMERA_2, &mut bytes)
                .expect("default body"),
            6
        );
        assert_eq!(bytes[0], 0x82);
        assert!(matches!(
            FIXED.write_into(CameraId::CAMERA_1, &mut []),
            Err(crate::Error::BufferTooSmall {
                required: 6,
                actual: 0,
                ..
            })
        ));
    }

    // Define a test command that uses a function with side effects
    visca_command! {
        /// Test command to verify parameter evaluation count
        pub struct EvalCounterCommand { value: u8 };
        prefix = [0x01, 0x04, 0x00];
        param = get_param_with_side_effect(*value);
        max_param_size = 1;
    }

    #[test]
    fn test_param_expr_evaluated_once_in_wire_write() {
        // Reset counter
        reset_counter();

        let cmd = EvalCounterCommand { value: 0x42 };

        let mut buffer = [0u8; 6];
        cmd.write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("should encode command");

        // The parameter expression is evaluated once during write_into.
        let eval_count = get_counter();
        assert_eq!(
            eval_count, 1,
            "Parameter expression should be evaluated exactly once, but was evaluated {} times",
            eval_count
        );
    }

    #[test]
    fn test_encoded_bytes_correct() {
        use crate::command::bytes::VISCA_TERMINATOR;

        let cmd = EvalCounterCommand { value: 0x42 };
        let mut buffer = [0u8; 16];
        let len = cmd
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("should encode");

        // Verify the encoded bytes are correct
        // Format: camera_id (0x81) + prefix (0x01, 0x04, 0x00) + param (0x42) + terminator
        assert_eq!(len, 6);
        assert_eq!(
            &buffer[..len],
            &[0x81, 0x01, 0x04, 0x00, 0x42, VISCA_TERMINATOR],
            "Encoded bytes should match expected format"
        );
    }
}
