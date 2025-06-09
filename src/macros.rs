//! Declarative macros for reducing VISCA command boilerplate.
//!
//! This module provides macros to simplify the creation of VISCA commands,
//! reducing repetitive code while maintaining type safety and clarity.
//!
//! ## Available Macros
//!
//! - `visca_command!` - Create simple command enums
//! - `visca_up_down_reset!` - Create commands with Up/Down/Reset variants
//! - `visca_param_command!` - Create commands with parameter encoding
//! - `visca_inquiry!` - Create inquiry commands
//! - `execute_command!` - Execute a command with standard error handling
//! - `impl_up_down_reset!` - Implement up/down/reset method triplets
//! - `impl_simple_command!` - Implement simple command methods
//! - `visca_bounded_param!` - Create validated newtype wrappers for numeric parameters

/// Create a simple VISCA command enum with byte sequences.
///
/// This macro generates a complete implementation of the `Command` trait
/// for simple commands that don't require parameters.
///
/// # Example
/// ```
/// use grafton_visca::visca_command;
///
/// visca_command! {
///     category = "Movement",
///     enum PanTiltCommand {
///         Home => [0x81, 0x01, 0x06, 0x04, 0xFF],
///         Reset => [0x81, 0x01, 0x06, 0x05, 0xFF],
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_command {
    (
        $(#[$meta:meta])*
        category = $category:literal,
        enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident $( ($($param:ident : $ptype:ty),*) )? => $body:tt
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant $( ($($ptype),*) )?,
            )+
        }

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                match self {
                    $(
                        Self::$variant $( ($($param),*) )? => $crate::visca_command!(@expand_body $body, $($($param),*)?),
                    )+
                }
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                match $category {
                    "Quick" => $crate::timeout::CommandCategory::Quick,
                    "Movement" => $crate::timeout::CommandCategory::Movement,
                    "Preset" => $crate::timeout::CommandCategory::Preset,
                    "LongRunning" => $crate::timeout::CommandCategory::LongRunning,
                    "Custom" => $crate::timeout::CommandCategory::Custom,
                    _ => $crate::timeout::CommandCategory::Custom,
                }
            }
        }
    };

    // Expand simple byte arrays
    (@expand_body [$($byte:expr),+ $(,)?], $($params:ident)*) => {
        Ok(vec![$($byte),+])
    };

    // Expand code blocks
    (@expand_body $block:block, $($params:ident)*) => {
        $block
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
            $crate::Response::Completion => Ok(()),
            $crate::Response::Error(e) => Err(e),
            _ => Err($crate::Error::UnexpectedResponseType),
        }
    };
}

/// Create a validated newtype wrapper for numeric parameters.
///
/// This macro generates a newtype struct with validation, conversion traits,
/// and common methods for VISCA parameter types that have min/max bounds.
///
/// # Example
/// ```ignore
/// use grafton_visca::visca_bounded_param;
///
/// visca_bounded_param! {
///     /// Variable zoom speed (0-7).
///     ZoomSpeed: u8 {
///         min: 0,
///         max: 7,
///         error_msg: "Zoom speed must be in the range 0..=7"
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_bounded_param {
    (
        $(#[$meta:meta])*
        $name:ident: $type:ty {
            min: $min:expr,
            max: $max:expr,
            error_msg: $error_msg:expr
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name($type);

        impl $name {
            /// Minimum allowed value.
            pub const MIN: $type = $min;
            /// Maximum allowed value.
            pub const MAX: $type = $max;

            /// Creates a new instance with validation.
            ///
            /// # Errors
            /// Returns `ViscaError::InvalidParameter` if value is out of range.
            pub fn new(value: $type) -> Result<Self, $crate::Error> {
                if (Self::MIN..=Self::MAX).contains(&value) {
                    Ok(Self(value))
                } else {
                    Err($crate::Error::InvalidParameter($error_msg.into()))
                }
            }

            /// Get the raw value.
            #[must_use]
            pub const fn value(self) -> $type {
                self.0
            }
        }

        impl std::convert::TryFrom<$type> for $name {
            type Error = $crate::Error;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for $type {
            fn from(val: $name) -> Self {
                val.0
            }
        }
    };
}

/// Create VISCA commands with Up/Down/Reset variants.
///
/// This macro is specifically designed for the common pattern of camera controls
/// that support incremental adjustment and reset operations.
///
/// # Example
/// ```
/// use grafton_visca::{visca_up_down_reset, visca_bounded_param};
///
/// visca_bounded_param! {
///     /// Iris level parameter
///     IrisLevel: u8 {
///         min: 0,
///         max: 0x11,
///         error_msg: "Iris level must be between 0x00 and 0x11"
///     }
/// }
///
/// visca_up_down_reset! {
///     #[category = "Quick"]
///     enum IrisCommand {
///         command_byte: 0x0B,
///         Direct(value: IrisLevel) => |high, low| [0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, high, low, 0xFF]
///     }
/// }
/// ```
#[macro_export]
macro_rules! visca_up_down_reset {
    (
        #[category = $category:literal]
        enum $name:ident {
            command_byte: $cmd:expr,
            $(Direct($param:ident: $type:ty) => |$high:ident, $low:ident| [$($direct_byte:expr),+])?
        }
    ) => {
        #[doc = concat!("Commands for controlling ", stringify!($name), " values.")]
        #[doc = ""]
        #[doc = "Provides standard VISCA control operations:"]
        #[doc = "- Reset to default value"]
        #[doc = "- Increment/decrement by one step"]
        #[doc = "- Set to a specific value (if supported)"]
        #[derive(Debug, Copy, Clone)]
        pub enum $name {
            /// Reset to default value.
            Reset,
            /// Increase value by one step.
            Up,
            /// Decrease value by one step.
            Down,
            $(
                /// Set to specific value.
                Direct($type),
            )?
        }

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                Ok(match self {
                    Self::Reset => vec![0x81, 0x01, 0x04, $cmd, 0x00, 0xFF],
                    Self::Up => vec![0x81, 0x01, 0x04, $cmd, 0x02, 0xFF],
                    Self::Down => vec![0x81, 0x01, 0x04, $cmd, 0x03, 0xFF],
                    $(
                        Self::Direct($param) => {
                            // Get the value and compute nibbles
                            let val = $param.value();
                            // This handles both u8 and u16 types. For u8, the cast is trivial.
                            // For u16, it truncates to the low byte as required by the VISCA protocol.
                            #[allow(clippy::cast_possible_truncation, trivial_numeric_casts)]
                            let byte_val = val as u8;
                            let $high = (byte_val >> 4) & 0x0F;
                            let $low = byte_val & 0x0F;
                            vec![$($direct_byte),+]
                        }
                    )?
                })
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                match $category {
                    "Quick" => $crate::timeout::CommandCategory::Quick,
                    "Movement" => $crate::timeout::CommandCategory::Movement,
                    _ => $crate::timeout::CommandCategory::Custom,
                }
            }
        }
    };
}

/// Create VISCA commands with parameter encoding.
///
/// This macro handles commands that require parameter encoding,
/// particularly for direct value settings.
///
/// # Example
/// ```
/// use grafton_visca::{visca_param_command, visca_bounded_param};
///
/// visca_bounded_param! {
///     /// Gain limit level
///     GainLimit: u8 {
///         min: 0,
///         max: 15,
///         error_msg: "Gain limit must be between 0 and 15"
///     }
/// }
///
/// visca_param_command! {
///     /// Command to set gain limit
///     struct GainLimitCommand {
///         /// The gain limit value
///         limit: GainLimit => direct
///     }
///     bytes = [0x81, 0x01, 0x04, 0x2C, {limit}, 0xFF]
/// }
/// ```
#[macro_export]
macro_rules! visca_param_command {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(#[$field_meta:meta])*
            $field:ident : $type:ty => direct
        }
        bytes = [$($byte:tt)*]
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(#[$field_meta])*
            pub $field: $type,
        }

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                // Pre-allocate with the expected size to avoid vec_init_then_push warning
                let mut bytes = Vec::with_capacity(16); // VISCA commands are typically short
                visca_param_command!(@encode bytes, self, [$($byte)*]);
                Ok(bytes)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::Quick
            }
        }
    };

    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(#[$field_meta:meta])*
            $field:ident : $type:ty => |$v:ident| $encode:expr
        }
        bytes = [$($byte:tt)*]
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(#[$field_meta])*
            pub $field: $type,
        }

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                let $v = self.$field;
                let encoded = $encode?;
                let mut bytes = Vec::with_capacity(16);
                $crate::visca_param_command!(@encode bytes, encoded, [$($byte)*]);
                Ok(bytes)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::Quick
            }
        }
    };

    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(#[$field_meta:meta])*
            $field:ident : $type:ty => nibbles
        }
        bytes = [$($byte:tt)*]
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(#[$field_meta])*
            pub $field: $type,
        }

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                let p = (self.$field >> 12) as u8;
                let q = ((self.$field >> 8) & 0x0F) as u8;
                let r = ((self.$field >> 4) & 0x0F) as u8;
                let s = (self.$field & 0x0F) as u8;
                let mut bytes = Vec::with_capacity(16);
                $crate::visca_param_command!(@encode_nibbles bytes, p, q, r, s, [$($byte)*]);
                Ok(bytes)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::Quick
            }
        }
    };

    // Internal encoding rules
    (@encode $bytes:ident, $self:ident, []) => {};

    (@encode $bytes:ident, $self:ident, [$byte:literal, $($rest:tt)*]) => {
        $bytes.push($byte);
        $crate::visca_param_command!(@encode $bytes, $self, [$($rest)*]);
    };

    (@encode $bytes:ident, $self:ident, [$byte:literal]) => {
        $bytes.push($byte);
    };

    (@encode $bytes:ident, $self:ident, [{$field:ident}, $($rest:tt)*]) => {
        $bytes.push($self.$field.value());
        $crate::visca_param_command!(@encode $bytes, $self, [$($rest)*]);
    };

    (@encode $bytes:ident, $self:ident, [{$field:ident}]) => {
        $bytes.push($self.$field.value());
    };

    // Encoding with single encoded value
    (@encode $bytes:ident, $encoded:expr, []) => {};

    (@encode $bytes:ident, $encoded:expr, [$byte:literal, $($rest:tt)*]) => {
        $bytes.push($byte);
        $crate::visca_param_command!(@encode $bytes, $encoded, [$($rest)*]);
    };

    (@encode $bytes:ident, $encoded:expr, [$byte:literal]) => {
        $bytes.push($byte);
    };

    (@encode $bytes:ident, $encoded:expr, [{$field:ident}, $($rest:tt)*]) => {
        $bytes.push($encoded);
        $crate::visca_param_command!(@encode $bytes, $encoded, [$($rest)*]);
    };

    (@encode $bytes:ident, $encoded:expr, [{$field:ident}]) => {
        $bytes.push($encoded);
    };

    // Encoding with nibbles
    (@encode_nibbles $bytes:ident, $p:expr, $q:expr, $r:expr, $s:expr, []) => {};

    (@encode_nibbles $bytes:ident, $p:expr, $q:expr, $r:expr, $s:expr, [$byte:literal, $($rest:tt)*]) => {
        $bytes.push($byte);
        $crate::visca_param_command!(@encode_nibbles $bytes, $p, $q, $r, $s, [$($rest)*]);
    };

    (@encode_nibbles $bytes:ident, $p:expr, $q:expr, $r:expr, $s:expr, [$byte:literal]) => {
        $bytes.push($byte);
    };

    (@encode_nibbles $bytes:ident, $p:expr, $q:expr, $r:expr, $s:expr, [{$field:ident}, $($rest:tt)*]) => {
        $bytes.push($p);
        $bytes.push($q);
        $bytes.push($r);
        $bytes.push($s);
        $crate::visca_param_command!(@encode_nibbles $bytes, $p, $q, $r, $s, [$($rest)*]);
    };

    (@encode_nibbles $bytes:ident, $p:expr, $q:expr, $r:expr, $s:expr, [{$field:ident}]) => {
        $bytes.push($p);
        $bytes.push($q);
        $bytes.push($r);
        $bytes.push($s);
    };
}

/// Create VISCA inquiry commands.
///
/// This macro generates inquiry commands that query camera state.
///
/// # Example
/// ```ignore
/// use grafton_visca::visca_inquiry;
///
/// visca_inquiry! {
///     #[category = "Quick"]
///     PowerInquiry => 0x00
/// }
/// ```
#[macro_export]
macro_rules! visca_inquiry {
    (
        #[category = $category:literal]
        $name:ident => $inquiry_byte:expr
    ) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $name;

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                Ok(vec![0x81, 0x09, 0x04, $inquiry_byte, 0xFF])
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                Some($crate::command::ResponseType::Inquiry)
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                match $category {
                    "Quick" => $crate::timeout::CommandCategory::Quick,
                    _ => $crate::timeout::CommandCategory::Custom,
                }
            }
        }
    };
}

/// Implement up/down/reset method triplets in extension traits.
///
/// This macro generates the three common methods for camera controls.
///
/// # Example
/// ```ignore
/// use grafton_visca::impl_up_down_reset;
///
/// impl ExposureExt for MyDevice {
///     impl_up_down_reset!(iris, IrisCommand);
///     // Generates: iris_up(), iris_down(), iris_reset()
/// }
/// ```
#[macro_export]
macro_rules! impl_up_down_reset {
    ($prefix:ident, $command_type:ty) => {
        paste::paste! {
            fn [<$prefix _up>](&mut self) -> Result<(), $crate::Error> {
                $crate::execute_command!(self, <$command_type>::Up)
            }

            fn [<$prefix _down>](&mut self) -> Result<(), $crate::Error> {
                $crate::execute_command!(self, <$command_type>::Down)
            }

            fn [<$prefix _reset>](&mut self) -> Result<(), $crate::Error> {
                $crate::execute_command!(self, <$command_type>::Reset)
            }
        }
    };
}

/// Implement simple command methods.
///
/// This macro generates methods that execute a specific command variant.
///
/// # Example
/// ```ignore
/// use grafton_visca::impl_simple_command;
///
/// impl ImageExt for MyDevice {
///     impl_simple_command!(backlight_on, BacklightCommand { status: true });
///     impl_simple_command!(backlight_off, BacklightCommand { status: false });
/// }
/// ```
#[macro_export]
macro_rules! impl_simple_command {
    ($method_name:ident, $command:expr) => {
        fn $method_name(&mut self) -> Result<(), $crate::Error> {
            $crate::execute_command!(self, $command)
        }
    };
}

/// Create VISCA commands with boolean on/off parameters.
///
/// This macro generates commands that have a simple boolean parameter
/// for enabling/disabling features.
///
/// # Example
/// ```
/// use grafton_visca::visca_bool_command;
///
/// visca_bool_command! {
///     /// Enable or disable backlight compensation.
///     struct BacklightCommand {
///         /// Enable (true) or disable (false) backlight compensation.
///         status: bool => |v| if v { 0x02 } else { 0x03 }
///     }
///     bytes = [0x81, 0x01, 0x04, 0x33, {status}, 0xFF]
/// }
/// ```
#[macro_export]
macro_rules! visca_bool_command {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $(#[$field_meta:meta])*
            $field:ident: bool => |$v:ident| $encode:expr
        }
        bytes = [$($byte:tt)*]
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone)]
        pub struct $name {
            $(#[$field_meta])*
            pub $field: bool,
        }

        impl $crate::command::Command for $name {
            fn to_bytes(&self) -> Result<Vec<u8>, $crate::Error> {
                let $v = self.$field;
                let encoded = $encode;
                let mut bytes = Vec::with_capacity(16);
                $crate::visca_bool_command!(@encode bytes, encoded, [$($byte)*]);
                Ok(bytes)
            }

            fn response_type(&self) -> Option<$crate::command::ResponseType> {
                None
            }

            fn command_category(&self) -> $crate::timeout::CommandCategory {
                $crate::timeout::CommandCategory::Quick
            }
        }
    };

    // Internal encoding rules
    (@encode $bytes:ident, $encoded:ident, []) => {};

    (@encode $bytes:ident, $encoded:ident, [$byte:literal, $($rest:tt)*]) => {
        $bytes.push($byte);
        $crate::visca_bool_command!(@encode $bytes, $encoded, [$($rest)*]);
    };

    (@encode $bytes:ident, $encoded:ident, [$byte:literal]) => {
        $bytes.push($byte);
    };

    (@encode $bytes:ident, $encoded:ident, [{$field:ident}, $($rest:tt)*]) => {
        $bytes.push($encoded);
        $crate::visca_bool_command!(@encode $bytes, $encoded, [$($rest)*]);
    };

    (@encode $bytes:ident, $encoded:ident, [{$field:ident}]) => {
        $bytes.push($encoded);
    };
}
