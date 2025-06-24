// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Unified camera_commands! declarative macro for reducing code duplication.
//!
//! This macro consolidates the various visca_method* proc macros into a single
//! declarative macro that generates both async and blocking implementations
//! from concise command definitions.

/// Generate camera command methods for both async and blocking implementations.
///
/// This macro reduces code duplication by generating both async and blocking
/// versions of camera methods from a single definition.
///
/// # Syntax
///
/// ```ignore
/// camera_commands! {
///     // Simple commands
///     method_name => CommandType { field: value };
///     
///     // Commands with parameters
///     method_name(param: Type) => CommandType { field: param };
///     
///     // Commands with generic parameters
///     method_name<G>(param: G) where G: Trait => CommandType { field: param.into() };
///     
///     // Fallible commands (note the ? in the definition)
///     method_name => {
///         CommandType {
///             field: expression?,
///         }
///     };
///     
///     // Commands with blocks for complex logic
///     method_name(param: Type) => {
///         let processed = transform(param)?;
///         CommandType { field: processed }
///     };
///     
///     // Commands with documentation
///     #[doc = "Method documentation"]
///     method_name => CommandType { field: value };
/// }
/// ```
macro_rules! camera_commands {
    // Main entry point - process all commands
    (
        $(
            $(#[$attr:meta])*
            $vis:vis fn $fn_name:ident $( < $($gen:ident),* $(,)? > )? $( ( $($param:ident : $param_ty:ty),* $(,)? ) )?
            $( where { $($where_clause:tt)* } )?
            => $body:expr
        );* $(;)?
    ) => {
        // Generate blocking implementation
        #[cfg(not(feature = "async"))]
        impl<P, T> Camera<P, T>
        where
            P: CameraProfile,
            T: crate::transport::blocking::BlockingTransport,
        {
            $(
                $(#[$attr])*
                $vis fn $fn_name $( < $($gen),* > )? (&mut self $(, $($param: $param_ty),*)?) -> Result<(), crate::Error>
                $( where $($where_clause)* )?
                {
                    let cmd = camera_commands!(@build_cmd $body);
                    self.send_and_wait(&cmd)
                }
            )*
        }

        // Generate async implementation
        #[cfg(feature = "async")]
        impl<P, T> Camera<P, T>
        where
            P: CameraProfile,
            T: crate::transport::AsyncTransport,
        {
            $(
                $(#[$attr])*
                $vis async fn $fn_name $( < $($gen),* > )? (&self $(, $($param: $param_ty),*)?) -> Result<(), crate::Error>
                $( where $($where_clause)* )?
                {
                    let cmd = camera_commands!(@build_cmd $body);
                    self.send_and_wait(&cmd).await
                }
            )*
        }
    };

    // Build command - handle block expressions (with braces)
    (@build_cmd { $($body:tt)* }) => {
        (|| -> Result<_, crate::Error> {
            Ok({ $($body)* })
        })()?
    };

    // Build command - handle simple expressions
    (@build_cmd $body:expr) => {
        $body
    };
}
