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
/// }
/// ```
///
/// Command with parameters:
/// ```ignore
/// visca_command! {
///     pub struct ImageFlip { mode: Flip };
///     prefix = [0x01, 0x06, 0x61];
///     param = match mode { Flip::On => 0x02, Flip::Off => 0x03 };
/// }
/// ```
#[macro_export]
macro_rules! visca_command {
    // Simple command without parameters
    (
        $(#[$meta:meta])*
        pub struct $name:ident;
        bytes = [$($byte:expr),* $(,)?];
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name;

        impl $crate::command::encode::WireEncode for $name {
            const MAX_SIZE: usize = { [$($byte),*].len() + 2 }; // +2 for camera_id and terminator

            fn write_into(&self, camera_id: $crate::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
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

        }
    };

    // Command with parameters and explicit max_param_size.
    (
        $(#[$meta:meta])*
        pub struct $name:ident { $($field:ident : $ftype:ty),* $(,)? };
        prefix = [$($byte:expr),* $(,)?];
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
            const MAX_SIZE: usize = 1 + [$($byte),*].len() + $max_param_size + 1;

            fn write_into(&self, camera_id: $crate::CameraId, buffer: &mut [u8]) -> Result<usize, $crate::Error> {
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

        }
    };

    // Command parameters must declare max_param_size so encoding capacity is explicit.
    // All commands with parameters MUST specify max_param_size.
}

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

        let mut buffer = [0u8; EvalCounterCommand::MAX_SIZE];
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
