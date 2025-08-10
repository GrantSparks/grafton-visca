//! Simple test to verify compilation succeeds with generic Camera implementation.

use grafton_visca::{
    capabilities::{NDFilter, Profile},
    transport::Transport,
    Camera,
};
// Import type aliases from the blocking prelude
use grafton_visca::prelude::blocking::{GenericViscaCam, PTZOpticsG2Cam, SonyFR7Cam};

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

// Compile-time capability checking example:
#[test]
fn test_compile_time_safety() {
    // This function can only be called with cameras that support ND filter
    fn _use_nd_filter<P, T>(_camera: &Camera<P, T>)
    where
        P: Profile + NDFilter,
        T: Transport + Send + Sync + 'static,
        T::Error: Into<grafton_visca::Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        // This would compile only for cameras with ND filter support
    }

    // This function works with any camera
    fn _use_basic_features<P, T>(_camera: &Camera<P, T>)
    where
        P: Profile,
        T: Transport + Send + Sync + 'static,
        T::Error: Into<grafton_visca::Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        // Basic features available on all cameras
    }

    // Test passes if code compiles
}
