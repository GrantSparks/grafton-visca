//! Consolidated macros for VISCA command implementation.
//!
//! This module provides the unified macro interface as specified in issue #397.
//! Primary macro: `visca_command!` for commands.

/// Create a VISCA command that expects ACK/Completion responses.
///
/// # Examples
///
/// Simple command without parameters:
/// ```ignore
/// visca_command! {
///     /// Power on the camera
///     pub struct PowerOn;
///     bytes = [0x01, 0x04, 0x00, 0x02];
///     category = CommandCategory::Quick;
/// }
/// ```
///
/// Command with parameters:
/// ```ignore
/// visca_command! {
///     pub struct ImageFlip { mode: Flip };
///     prefix = [0x01, 0x06, 0x61];
///     param = match mode { Flip::On => 0x02, Flip::Off => 0x03 };
///     category = CommandCategory::Quick;
/// }
/// ```
#[macro_export]
macro_rules! visca_command {
    // Simple command without parameters
    (
        $(#[$meta:meta])*
        pub struct $name:ident;
        bytes = [$($byte:expr),* $(,)?];
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name;

        impl $crate::command::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = { [$($byte),*].len() + 2 }; // +2 for camera_id and terminator
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                const BYTES: &[u8] = &[$($byte),*];
                let len = BYTES.len() + 2;

                if buffer.len() < len {
                    return Err($crate::Error::BufferTooSmall {
                        required: len,
                        actual: buffer.len()
                    });
                }

                buffer[0] = camera_id.to_address_byte();
                buffer[1..1+BYTES.len()].copy_from_slice(BYTES);
                buffer[len-1] = $crate::command::bytes::VISCA_TERMINATOR;
                Ok(len)
            }

            fn response_kind(&self) -> Option<$crate::command::InquiryKind> {
                None
            }
        }
    };

    // Command with parameters and explicit max_param_size
    //
    // Note: This arm deliberately does NOT override `encoded_size()`. The default
    // implementation returns `MAX_SIZE`, which is conservative but correct.
    // This avoids:
    // 1. Double evaluation of `$param_expr` (once in encoded_size, once in write_into)
    // 2. Potential mismatches if $param_expr has side effects
    //
    // The encoding pipeline (`EncodedCommand::new`, `to_bytes`) calls `encoded_size()`
    // before `write_into()`, so removing the custom implementation ensures the param
    // expression is only evaluated once during `write_into()`.
    (
        $(#[$meta:meta])*
        pub struct $name:ident { $($field:ident : $ftype:ty),* $(,)? };
        prefix = [$($byte:expr),* $(,)?];
        param = $param_expr:expr;
        max_param_size = $max_param_size:expr;
        category = $category:expr;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(/// The parameter value.
            pub $field: $ftype,)*
        }

        impl $crate::command::ViscaCommand for $name {
            type Response = ();
            const MAX_SIZE: usize = 1 + [$($byte),*].len() + $max_param_size + 1;
            const TIMEOUT_CATEGORY: $crate::timeout::CommandCategory = $category;

            // Use default encoded_size() which returns MAX_SIZE.
            // This avoids double evaluation of $param_expr.

            fn write_into(&self, camera_id: $crate::camera_id::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
                const PREFIX: &[u8] = &[$($byte),*];

                // Destructure self for use in param expression
                let Self { $($field),* } = self;

                // Encode parameters without heap allocation
                let params: $crate::macros::param::ParamBuf<$max_param_size> =
                    $crate::macros::param::IntoParamBuf::<$max_param_size>::encode_param($param_expr);

                let total_len = 1 + PREFIX.len() + params.len() + 1; // camera_id + prefix + params + terminator

                if buffer.len() < total_len {
                    return Err($crate::Error::BufferTooSmall {
                        required: total_len,
                        actual: buffer.len()
                    });
                }

                let mut pos = 0;
                buffer[pos] = camera_id.to_address_byte();
                pos += 1;

                buffer[pos..pos+PREFIX.len()].copy_from_slice(PREFIX);
                pos += PREFIX.len();

                buffer[pos..pos+params.len()].copy_from_slice(params.as_slice());
                pos += params.len();

                buffer[pos] = $crate::command::bytes::VISCA_TERMINATOR;
                Ok(pos + 1)
            }

            fn response_kind(&self) -> Option<$crate::command::InquiryKind> {
                None
            }
        }
    };

    // Command with parameters (without explicit max_param_size) - DEPRECATED
    // This arm is removed to enforce explicit max_param_size for safety.
    // All commands with parameters MUST specify max_param_size.
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::{
        camera_id::CameraId,
        command::encode::{EncodedCommand, ViscaCommand},
        timeout::CommandCategory,
    };
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

    // Define a test command that uses a function with side effects
    visca_command! {
        /// Test command to verify parameter evaluation count
        pub struct EvalCounterCommand { value: u8 };
        prefix = [0x01, 0x04, 0x00];
        param = get_param_with_side_effect(*value);
        max_param_size = 1;
        category = CommandCategory::Quick;
    }

    #[test]
    fn test_param_expr_evaluated_once_in_encoding_pipeline() {
        // Reset counter
        reset_counter();

        let cmd = EvalCounterCommand { value: 0x42 };

        // Create an EncodedCommand which calls encoded_size() then write_into()
        let _encoded =
            EncodedCommand::new(&cmd, CameraId::CAMERA_1).expect("should encode command");

        // The parameter expression should only be evaluated once (during write_into),
        // not twice (once during encoded_size, once during write_into)
        let eval_count = get_counter();
        assert_eq!(
            eval_count, 1,
            "Parameter expression should be evaluated exactly once, but was evaluated {} times",
            eval_count
        );
    }

    #[test]
    fn test_param_expr_evaluated_once_in_to_bytes() {
        // Reset counter
        reset_counter();

        let cmd = EvalCounterCommand { value: 0x42 };

        // Call to_bytes which also calls encoded_size() then write_into()
        let _bytes = cmd.to_bytes(CameraId::CAMERA_1).expect("should encode");

        // The parameter expression should only be evaluated once
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
