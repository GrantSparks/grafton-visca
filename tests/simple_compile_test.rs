//! Simple test to verify compilation succeeds with generic Camera implementation.
//!
//! This test demonstrates the mode-based Camera API with compile-time mode selection.

#[cfg(feature = "async")]
use grafton_visca::{camera::AsyncMode, transport::async_transport::AsyncTransport};
#[cfg(not(feature = "async"))]
use grafton_visca::{camera::BlockingMode, transport::BlockingTransport};
use grafton_visca::{
    camera::Camera,
    capabilities::{NDFilter, Profile},
};

// Import type aliases based on feature flags
#[cfg(not(feature = "async"))]
use grafton_visca::prelude::blocking::{GenericViscaCam, PTZOpticsG2Cam, SonyFR7Cam};

#[cfg(feature = "async")]
use grafton_visca::prelude::r#async::{GenericViscaCam, PTZOpticsG2Cam, SonyFR7Cam};

#[test]
fn test_compilation_succeeds() {
    // The generic Camera API provides compile-time type safety
    // These types exist and can be used (would need actual transports in real usage):

    // Type aliases exist for common camera models
    type _G2Camera<T> = PTZOpticsG2Cam<T>;
    type _FR7Camera<T> = SonyFR7Cam<T>;
    type _GenericCamera<T> = GenericViscaCam<T>;

    // The test validates that the Camera types compile correctly
    // No assertion needed - the test passes if compilation succeeds
}

// Compile-time capability checking example with mode-based API:
#[test]
fn test_compile_time_safety() {
    // For async cameras with ND filter support
    #[cfg(feature = "async")]
    fn _use_nd_filter_async<P, T>(_camera: &Camera<AsyncMode, P, T>)
    where
        P: Profile + NDFilter,
        T: AsyncTransport + Send + Sync + 'static,
    {
        // This would compile only for async cameras with ND filter support
    }

    // For blocking cameras with ND filter support
    #[cfg(not(feature = "async"))]
    fn _use_nd_filter_blocking<P, T>(_camera: &Camera<BlockingMode, P, T>)
    where
        P: Profile + NDFilter,
        T: BlockingTransport + Send + Sync + 'static,
    {
        // This would compile only for blocking cameras with ND filter support
    }

    // For async cameras with basic features
    #[cfg(feature = "async")]
    fn _use_basic_features_async<P, T>(_camera: &Camera<AsyncMode, P, T>)
    where
        P: Profile,
        T: AsyncTransport + Send + Sync + 'static,
    {
        // Basic features available on all async cameras
    }

    // For blocking cameras with basic features
    #[cfg(not(feature = "async"))]
    fn _use_basic_features_blocking<P, T>(_camera: &Camera<BlockingMode, P, T>)
    where
        P: Profile,
        T: BlockingTransport + Send + Sync + 'static,
    {
        // Basic features available on all blocking cameras
    }

    // Test passes if code compiles
}

#[test]
fn test_mode_specific_apis() {
    // Demonstrate that the mode marker ensures correct API usage

    #[cfg(feature = "async")]
    {
        // This function only accepts async cameras
        fn _async_only<P, T>(_camera: &Camera<AsyncMode, P, T>)
        where
            P: Profile,
            T: AsyncTransport,
        {
            // Would have access to async methods here
        }
    }

    #[cfg(not(feature = "async"))]
    {
        // This function only accepts blocking cameras
        fn _blocking_only<P, T>(_camera: &Camera<BlockingMode, P, T>)
        where
            P: Profile,
            T: BlockingTransport,
        {
            // Would have access to blocking methods here
        }
    }
}
