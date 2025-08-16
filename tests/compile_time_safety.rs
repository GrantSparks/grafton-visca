//! Tests to verify compile-time safety of the generic camera API.

use grafton_visca::{
    camera::{
        profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        Camera,
    },
    capabilities::{PanTilt, Profile, ProfileMetadata, Zoom},
};

#[cfg(feature = "async")]
use grafton_visca::camera::AsyncMode;

#[cfg(not(feature = "async"))]
use grafton_visca::camera::BlockingMode;

// Import type aliases from prelude for testing
// Using cfg to conditionally import based on features
#[cfg(feature = "async")]
use grafton_visca::prelude::r#async::{PtzOpticsG2Cam, SonyFR7Cam};

#[cfg(not(feature = "async"))]
use grafton_visca::prelude::blocking::{PtzOpticsG2Cam, SonyFR7Cam};

// This test verifies that the Profile super-trait works correctly
#[test]
fn test_profile_super_trait() {
    // These types should compile because they implement Profile
    fn accepts_profile<P: Profile>() {}

    accepts_profile::<PtzOpticsG2>();
    accepts_profile::<SonyFR7>();
    accepts_profile::<GenericVisca>();
}

// This test verifies that all profile types have the required constants
#[test]
fn test_profile_constants() {
    assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
    assert_eq!(SonyFR7::MODEL_NAME, "Sony FR7");
    assert_eq!(GenericVisca::MODEL_NAME, "Generic VISCA Camera");

    assert_eq!(PtzOpticsG2::ZOOM_SPEED_RANGE, 0..8);
    assert_eq!(SonyFR7::ZOOM_SPEED_RANGE, 0..8);

    assert_eq!(PtzOpticsG2::MAX_PAN_SPEED, 24);
    assert_eq!(SonyFR7::MAX_PAN_SPEED, 24);
}

// This test verifies that generic camera methods work
#[test]
fn test_generic_camera_methods() {
    // We can't actually create a camera without a transport, but we can test the types compile
    #[cfg(feature = "async")]
    type _G2Camera<T, E> = Camera<AsyncMode, PtzOpticsG2, T, E>;
    #[cfg(not(feature = "async"))]
    type _G2Camera<T> = Camera<BlockingMode, PtzOpticsG2, T, ()>;

    #[cfg(feature = "async")]
    type _FR7Camera<T, E> = Camera<AsyncMode, SonyFR7, T, E>;
    #[cfg(not(feature = "async"))]
    type _FR7Camera<T> = Camera<BlockingMode, SonyFR7, T, ()>;

    // Test that type aliases work
    type _G2Alias<T> = PtzOpticsG2Cam<T>;
    type _FR7Alias<T> = SonyFR7Cam<T>;

    // If we had a camera instance, we could call these methods:
    // camera.model_name()
    // camera.zoom_speed_range()
    // camera.max_pan_speed()
    // camera.optical_zoom_max()
}

// The following tests would fail to compile if uncommented, proving compile-time safety:

/*
// This would fail because PtzOpticsG2 doesn't implement NDFilter
#[test]
fn test_nd_filter_compile_error() {
    fn requires_nd_filter<P, T>(camera: &Camera<P, T>)
    where
        P: Profile + grafton_visca::capabilities::NDFilter,
        T: grafton_visca::transport::Transport + Send + Sync + 'static,
        T::Error: Into<grafton_visca::Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        // ND filter methods would be available here
    }

    // This would fail to compile:
    // let g2_camera: Camera<PtzOpticsG2, _> = unimplemented!();
    // requires_nd_filter(&g2_camera); // ERROR: PtzOpticsG2 doesn't implement NDFilter
}
*/

/*
// This would fail because only SonyFR7 implements VariableSpeed
#[test]
fn test_variable_speed_compile_error() {
    fn requires_variable_speed<P, T>(camera: &Camera<P, T>)
    where
        P: Profile + grafton_visca::capabilities::VariableSpeed,
        T: grafton_visca::transport::Transport + Send + Sync + 'static,
        T::Error: Into<grafton_visca::Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        // Variable speed methods would be available here
    }

    // This would fail to compile:
    // let g2_camera: Camera<PtzOpticsG2, _> = unimplemented!();
    // requires_variable_speed(&g2_camera); // ERROR: PtzOpticsG2 doesn't implement VariableSpeed
}
*/
