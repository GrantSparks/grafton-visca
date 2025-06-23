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
//! - `define_camera_methods!` - Generate unified camera methods for both blocking and async
//! - `define_generic_camera_methods!` - Generate generic camera methods with type conversions

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
/// fn zoom_in(&mut self) -> Result<(), Error> {
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
            /// Returns `Error::InvalidParameter` if value is out of range.
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

/// Generate both blocking and async implementations for camera methods.
///
/// This macro creates both synchronous and asynchronous versions of camera methods
/// based on the enabled features. When the `async` feature is enabled, it generates
/// async methods. Otherwise, it generates blocking methods.
///
/// The macro automatically handles the conversion between sync and async by:
/// - For blocking: uses `&mut self` and direct method calls
/// - For async: uses `&self` and adds `.await` to async method calls
///
/// # Example
/// ```ignore
/// use grafton_visca::define_camera_methods;
///
/// define_camera_methods! {
///     impl<P: CameraProfile> Camera<P> {
///         /// Power on the camera.
///         pub fn power_on() -> Result<(), Error> {
///             let command = PowerCommand { power: Power::On };
///             self.send_and_wait(&command)
///         }
///         
///         /// Stop all camera movement.
///         pub fn stop() -> Result<(), Error> {
///             let command = PanTiltCommand::Move {
///                 direction: PanTiltDirection::Stop,
///                 pan_speed: PanSpeed::new(0)?,
///                 tilt_speed: TiltSpeed::new(0)?,
///             };
///             self.send_and_wait(&command)
///         }
///     }
/// }
/// ```
#[macro_export]
/// Generate unified camera methods that work for both blocking and async modes.
///
/// This macro takes method definitions and generates two implementations:
/// - Blocking: uses `&mut self` and calls `send_and_wait` without `.await`
/// - Async: uses `&self` and calls `send_and_wait` with `.await`
///
/// The method body should be a single expression calling `self.send_and_wait(...)`.
macro_rules! define_camera_methods {
    (
        $(
            $(#[$doc:meta])*
            pub fn $name:ident(&self $(, $param:ident : $ptype:ty)*) -> Result<$ret:ty, Error> {
                self.send_and_wait($cmd:expr)
            }
        )*
    ) => {
        // Generate blocking implementation
        #[cfg(not(feature = "async"))]
        impl<P: CameraProfile, T> Camera<P, T>
        where
            T: $crate::transport::blocking::BlockingTransport,
        {
            $(
                $(#[$doc])*
                pub fn $name(&mut self $(, $param : $ptype)*) -> Result<$ret, $crate::error::Error> {
                    match self.send_command($cmd)? {
                        $crate::Response::Completion => Ok(()),
                        $crate::Response::Ack => Ok(()),
                        response => Err($crate::error::Error::InvalidResponse {
                            expected: "Completion".to_string(),
                            actual: format!("{:?}", response).into_bytes(),
                        }),
                    }
                }
            )*
        }

        // Generate async implementation
        #[cfg(feature = "async")]
        impl<P: CameraProfile, T> Camera<P, T>
        where
            T: $crate::transport::AsyncTransport,
        {
            $(
                $(#[$doc])*
                pub async fn $name(&self $(, $param : $ptype)*) -> Result<$ret, $crate::error::Error> {
                    match self.send_command($cmd).await? {
                        $crate::Response::Completion => Ok(()),
                        $crate::Response::Ack => Ok(()),
                        response => Err($crate::error::Error::InvalidResponse {
                            expected: "Completion".to_string(),
                            actual: format!("{:?}", response).into_bytes(),
                        }),
                    }
                }
            )*
        }
    };
}

/// Generate camera methods with generic parameter acceptance.
///
/// This macro creates methods that accept multiple parameter types through generic bounds,
/// enabling ergonomic usage with raw values, typed wrappers, and enums.
///
/// The macro generates both sync (blocking) and async versions of the methods automatically.
/// For async methods, it transforms `self.send_and_wait(...)` calls to include `.await`.
///
/// # Example
/// ```ignore
/// use grafton_visca::define_generic_camera_methods;
///
/// define_generic_camera_methods! {
///     /// Set zoom position with flexible parameter types.
///     #[generic_params(Z)]
///     #[where_clause(Z: TryInto<ZoomPosition>, Z::Error: Into<Error>)]
///     pub fn set_zoom(&self, position: Z) -> Result<(), Error> {
///         let position = position.try_into().map_err(Into::into)?;
///         self.send_and_wait(&ZoomCommand::direct::<P>(position.value())?)
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_generic_camera_methods {
    (
        $(
            $(#[$doc:meta])*
            generic_params = [$($gen:ident),+];
            where_clause = [$($where_clause:tt)*];
            pub fn $name:ident(&self $(, $param:ident : $ptype:ty)*) -> Result<$ret:ty, Error> {
                $($body:tt)*
            }
        )*
    ) => {
        // Generate blocking implementation
        #[cfg(not(feature = "async"))]
        impl<P: CameraProfile, T> Camera<P, T>
        where
            T: $crate::transport::blocking::BlockingTransport,
        {
            $(
                $(#[$doc])*
                pub fn $name<$($gen),+>(&mut self $(, $param : $ptype)*) -> Result<$ret, $crate::error::Error>
                where
                    $($where_clause)*
                {
                    $($body)*
                }
            )*
        }

        // Generate async implementation
        #[cfg(feature = "async")]
        impl<P: CameraProfile, T> Camera<P, T>
        where
            T: $crate::transport::AsyncTransport,
        {
            $(
                $(#[$doc])*
                pub async fn $name<$($gen),+>(&self $(, $param : $ptype)*) -> Result<$ret, $crate::error::Error>
                where
                    $($where_clause)*
                {
                    define_generic_camera_methods!(@async_body $($body)*)
                }
            )*
        }
    };

    // Transform bodies for async
    (@async_body { $($body:tt)* }) => {
        {
            define_generic_camera_methods!(@async_transform $($body)*)
        }
    };

    (@async_transform) => {};

    (@async_transform let $var:ident = $init:expr; $($rest:tt)*) => {
        let $var = $init;
        define_generic_camera_methods!(@async_transform $($rest)*)
    };

    (@async_transform self.send_and_wait($expr:expr) $($rest:tt)*) => {
        self.send_and_wait($expr).await
        define_generic_camera_methods!(@async_transform $($rest)*)
    };

    (@async_transform $stmt:stmt) => {
        $stmt
    };

    (@async_transform $stmt:stmt; $($rest:tt)*) => {
        $stmt;
        define_generic_camera_methods!(@async_transform $($rest)*)
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
