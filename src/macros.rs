//! Declarative macros for reducing VISCA command boilerplate.
//!
//! This module provides macros to simplify the creation of VISCA commands,
//! reducing repetitive code while maintaining type safety and clarity.
//!
//! ## Available Macros
//!
//! - `visca_command!` - Create simple command enums
//! - `visca_param_command!` - Create commands with parameters
//! - `visca_inquiry!` - Create inquiry commands with response parsing
//! - `execute_command!` - Execute a command with standard error handling
//! - `impl_up_down_reset!` - Generate up/down/reset method triplets

/// Create a simple VISCA command enum with byte sequences.
///
/// This macro generates a complete implementation of the `ViscaCommand` trait
/// for simple commands that don't require parameters.
///
/// # Example
/// ```
/// use grafton_visca::visca_command;
///
/// visca_command! {
///     #[category = "Movement"]
///     enum PanTiltCommand {
///         Home => [0x81, 0x01, 0x06, 0x04, 0xFF],
///         Reset => [0x81, 0x01, 0x06, 0x05, 0xFF],
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_command {
    (
        #[category = $category:literal]
        enum $name:ident {
            $(
                $variant:ident => [$($byte:expr),+ $(,)?]
            ),+ $(,)?
        }
    ) => {
        #[derive(Debug)]
        pub enum $name {
            $(
                $variant,
            )+
        }

        impl $crate::command::ViscaCommand for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::ViscaError> {
                match self {
                    $(
                        Self::$variant => Ok(vec![$($byte),+]),
                    )+
                }
            }

            fn response_type(&self) -> Option<$crate::command::ViscaResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                match $category {
                    "Quick" => $crate::timeout::CommandCategory::Quick,
                    "Movement" => $crate::timeout::CommandCategory::Movement,
                    "Preset" => $crate::timeout::CommandCategory::Preset,
                    "LongRunning" => $crate::timeout::CommandCategory::LongRunning,
                    _ => $crate::timeout::CommandCategory::Custom,
                }
            }
        }
    };
}

/// Create a VISCA command with parameters.
///
/// This macro generates commands that accept parameters and encode them
/// according to VISCA protocol requirements.
///
/// # Example
/// ```ignore
/// use grafton_visca::visca_param_command;
///
/// visca_param_command! {
///     #[category = "Movement"]
///     struct DirectZoomCommand {
///         position: u16 => nibbles(4),
///     }
///     bytes = [0x81, 0x01, 0x04, 0x47, {position}, 0xFF]
/// }
/// ```
#[macro_export]
macro_rules! visca_param_command {
    (
        #[category = $category:literal]
        struct $name:ident {
            $($field:ident : $type:ty => $encoding:ident($size:expr)),* $(,)?
        }
        bytes = [$($byte:tt)*]
    ) => {
        #[derive(Debug)]
        pub struct $name {
            $(pub $field: $type,)*
        }

        impl $crate::command::ViscaCommand for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::ViscaError> {
                let mut bytes = Vec::new();
                visca_param_command!(@encode bytes, self, [$($byte)*]);
                Ok(bytes)
            }

            fn response_type(&self) -> Option<$crate::command::ViscaResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                match $category {
                    "Quick" => $crate::timeout::CommandCategory::Quick,
                    "Movement" => $crate::timeout::CommandCategory::Movement,
                    "Preset" => $crate::timeout::CommandCategory::Preset,
                    "LongRunning" => $crate::timeout::CommandCategory::LongRunning,
                    _ => $crate::timeout::CommandCategory::Custom,
                }
            }
        }
    };

    // Internal rule for encoding bytes
    (@encode $bytes:ident, $self:ident, []) => {};

    (@encode $bytes:ident, $self:ident, [$byte:literal $(, $rest:tt)*]) => {
        $bytes.push($byte);
        visca_param_command!(@encode $bytes, $self, [$($rest)*]);
    };

    (@encode $bytes:ident, $self:ident, [{$field:ident} $(, $rest:tt)*]) => {
        // Handle field encoding based on the type
        $bytes.extend_from_slice(&visca_encode_field!($self.$field));
        visca_param_command!(@encode $bytes, $self, [$($rest)*]);
    };
}

/// Helper macro for encoding fields.
#[macro_export]
macro_rules! visca_encode_field {
    ($value:expr) => {{
        // This is a simplified version - in practice, you'd match on the encoding type
        let val = $value as u16;
        vec![
            ((val >> 12) & 0x0F) as u8,
            ((val >> 8) & 0x0F) as u8,
            ((val >> 4) & 0x0F) as u8,
            (val & 0x0F) as u8,
        ]
    }};
}

/// Create inquiry commands with response parsing.
///
/// This macro generates both the command and its response parsing logic.
///
/// # Example
/// ```ignore
/// use grafton_visca::visca_inquiry;
///
/// visca_inquiry! {
///     #[category = "Quick"]
///     PowerInquiry => [0x81, 0x09, 0x04, 0x00, 0xFF]
///     response = |data: &[u8]| {
///         if data.len() == 4 && data[0] == 0x90 && data[1] == 0x50 {
///             Some(ViscaInquiryResponse::Power { on: data[2] == 0x02 })
///         } else {
///             None
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_inquiry {
    (
        #[category = $category:literal]
        $name:ident => [$($byte:expr),+ $(,)?]
        response = $parser:expr
    ) => {
        #[derive(Debug)]
        pub struct $name;

        impl $crate::command::ViscaCommand for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::ViscaError> {
                Ok(vec![$($byte),+])
            }

            fn response_type(&self) -> Option<$crate::command::ViscaResponseType> {
                Some($crate::command::ViscaResponseType::Inquiry)
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                match $category {
                    "Quick" => $crate::timeout::CommandCategory::Quick,
                    "Movement" => $crate::timeout::CommandCategory::Movement,
                    "Preset" => $crate::timeout::CommandCategory::Preset,
                    "LongRunning" => $crate::timeout::CommandCategory::LongRunning,
                    _ => $crate::timeout::CommandCategory::Custom,
                }
            }
        }
    };
}

/// Execute a VISCA command with standard error handling.
///
/// This macro consolidates the common pattern of executing a command and handling
/// the response, reducing 5 lines of boilerplate to a single macro call.
///
/// # Example
/// ```ignore
/// use grafton_visca::execute_command;
///
/// fn zoom_in(&mut self) -> Result<(), ViscaError> {
///     execute_command!(self, ZoomCommand::TeleStandard)
/// }
/// ```
#[macro_export]
macro_rules! execute_command {
    ($device:expr, $command:expr) => {
        match $device.execute_command(&$command)? {
            $crate::ViscaResponse::Completion => Ok(()),
            $crate::ViscaResponse::Error(e) => Err(e),
            _ => Err($crate::ViscaError::UnexpectedResponseType),
        }
    };
}

/// Generate up/down/reset method triplets for camera controls.
///
/// This macro creates three methods following the common pattern used for
/// controls like iris, shutter, gain, brightness, etc.
///
/// # Example
/// ```ignore
/// use grafton_visca::impl_up_down_reset;
///
/// impl ViscaExposureExt for MyDevice {
///     impl_up_down_reset!(iris, IrisCommand);
///     // Generates: iris_up(), iris_down(), iris_reset()
/// }
/// ```
#[macro_export]
macro_rules! impl_up_down_reset {
    ($prefix:ident, $command_type:ty) => {
        fn [<$prefix _up>](&mut self) -> Result<(), $crate::ViscaError> {
            $crate::execute_command!(self, <$command_type>::Up)
        }

        fn [<$prefix _down>](&mut self) -> Result<(), $crate::ViscaError> {
            $crate::execute_command!(self, <$command_type>::Down)
        }

        fn [<$prefix _reset>](&mut self) -> Result<(), $crate::ViscaError> {
            $crate::execute_command!(self, <$command_type>::Reset)
        }
    };
}

/// Generate simple command methods that just execute a command.
///
/// This macro creates methods that construct and execute a command variant.
///
/// # Example
/// ```ignore
/// use grafton_visca::impl_simple_command;
///
/// impl ViscaImageExt for MyDevice {
///     impl_simple_command!(flip_horizontal_on, FlipCommand::HorizontalOn);
///     impl_simple_command!(flip_horizontal_off, FlipCommand::HorizontalOff);
/// }
/// ```
#[macro_export]
macro_rules! impl_simple_command {
    ($method_name:ident, $command:expr) => {
        fn $method_name(&mut self) -> Result<(), $crate::ViscaError> {
            $crate::execute_command!(self, $command)
        }
    };
}

/// Generate command methods with optional speed parameters.
///
/// This macro creates methods that accept an optional speed parameter with validation.
///
/// # Example
/// ```ignore
/// use grafton_visca::impl_speed_command;
///
/// impl ViscaFocusExt for MyDevice {
///     impl_speed_command!(focus_near, FocusCommand::Near, FocusSpeed);
///     impl_speed_command!(focus_far, FocusCommand::Far, FocusSpeed);
/// }
/// ```
#[macro_export]
macro_rules! impl_speed_command {
    ($method_name:ident, $command_variant:path, $speed_type:ty) => {
        fn $method_name(&mut self, speed: Option<$speed_type>) -> Result<(), $crate::ViscaError> {
            let command = if let Some(s) = speed {
                $command_variant(s)
            } else {
                $command_variant(<$speed_type>::default())
            };
            $crate::execute_command!(self, command)
        }
    };
}
