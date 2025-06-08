//! Macros for creating unified extension traits.
//!
//! These macros eliminate code duplication between sync and async implementations
//! while maintaining a clean, consistent API.

/// Creates a unified extension trait with both sync and async methods.
///
/// This macro generates:
/// - A sync trait with blocking methods
/// - An async trait with async methods (when async-client feature is enabled)
/// - Implementations for ViscaClient that delegate to the appropriate send method
///
/// # Example
/// ```ignore
/// unified_ext! {
///     /// Power control extension.
///     trait PowerExt {
///         /// Check if powered on.
///         fn is_powered_on() -> bool {
///             let response = send(&InquiryCommand::Power);
///             match response {
///                 ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
///                 _ => Err(ViscaError::UnexpectedResponseType),
///             }
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! unified_ext {
    (
        $(#[$trait_meta:meta])*
        trait $trait_name:ident {
            $(
                $(#[$method_meta:meta])*
                fn $method_name:ident($($param:ident: $param_ty:ty),*) -> $ret_ty:ty $body:block
            )*
        }
    ) => {
        // Generate the sync trait
        $(#[$trait_meta])*
        pub trait $trait_name {
            $(
                $(#[$method_meta])*
                fn $method_name(&self, $($param: $param_ty),*) -> Result<$ret_ty, ViscaError>;
            )*
        }
        
        // Generate the async trait (only when async-client feature is enabled)
        #[cfg(feature = "async-client")]
        paste::paste! {
            $(#[$trait_meta])*
            #[doc = " (async version)"]
            pub trait [<Async $trait_name>] {
                $(
                    $(#[$method_meta])*
                    async fn [<$method_name _async>](&self, $($param: $param_ty),*) -> Result<$ret_ty, ViscaError>;
                )*
            }
        }
        
        // Implement the sync trait
        impl $trait_name for ViscaClient {
            $(
                fn $method_name(&self, $($param: $param_ty),*) -> Result<$ret_ty, ViscaError> {
                    // In the body, replace `send` with `self.send`
                    let send = |cmd: &dyn ViscaCommand| self.send(cmd);
                    $body
                }
            )*
        }
        
        // Implement the async trait
        #[cfg(feature = "async-client")]
        paste::paste! {
            impl [<Async $trait_name>] for ViscaClient {
                $(
                    async fn [<$method_name _async>](&self, $($param: $param_ty),*) -> Result<$ret_ty, ViscaError> {
                        // In the body, replace `send` with `self.send_async().await`
                        let send = |cmd: &dyn ViscaCommand| async move { self.send_async(cmd).await };
                        
                        // Transform the body to be async
                        async move $body.await
                    }
                )*
            }
        }
    };
}

/// Creates a unified extension method that works in both sync and async contexts.
///
/// This is a simpler macro for individual methods rather than entire traits.
#[macro_export]
macro_rules! unified_method {
    (
        $(#[$meta:meta])*
        fn $name:ident(&self $(, $param:ident: $param_ty:ty)*) -> Result<$ret_ty:ty, ViscaError> {
            $($body:tt)*
        }
    ) => {
        $(#[$meta])*
        #[cfg(feature = "blocking-client")]
        fn $name(&self $(, $param: $param_ty)*) -> Result<$ret_ty, ViscaError> {
            $($body)*
        }
        
        $(#[$meta])*
        #[cfg(feature = "async-client")]
        async fn $name(&self $(, $param: $param_ty)*) -> Result<$ret_ty, ViscaError> {
            $($body)*
        }
    };
}