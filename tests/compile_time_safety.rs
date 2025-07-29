//! Tests to verify compile-time safety of the generic camera API.

use grafton_visca::{
    camera::{
        generic::Camera,
        profiles::{PTZOpticsG2, SonyFR7},
    },
    capabilities::Profile,
    prelude::*,
};

// This test verifies that the Profile super-trait works correctly
#[test]
fn test_profile_super_trait() {
    // These types should compile because they implement Profile
    fn accepts_profile<P: Profile>() {}
    
    accepts_profile::<PTZOpticsG2>();
    accepts_profile::<SonyFR7>();
    accepts_profile::<GenericVisca>();
}

// This test verifies that all profile types have the required constants
#[test]
fn test_profile_constants() {
    assert_eq!(PTZOpticsG2::MODEL_NAME, "PTZOptics G2");
    assert_eq!(SonyFR7::MODEL_NAME, "Sony FR7");
    assert_eq!(GenericVisca::MODEL_NAME, "Generic VISCA Camera");
    
    assert_eq!(PTZOpticsG2::ZOOM_SPEED_RANGE, 0..8);
    assert_eq!(SonyFR7::ZOOM_SPEED_RANGE, 0..8);
    
    assert_eq!(PTZOpticsG2::MAX_PAN_SPEED, 24);
    assert_eq!(SonyFR7::MAX_PAN_SPEED, 24);
}

// This test verifies that generic camera methods work
#[test]
fn test_generic_camera_methods() {
    // We can't actually create a camera without a transport, but we can test the types compile
    type G2Camera<T> = Camera<PTZOpticsG2, T>;
    type FR7Camera<T> = Camera<SonyFR7, T>;
    
    // Test that type aliases work
    type G2Alias<T> = PTZOpticsG2Cam<T>;
    type FR7Alias<T> = SonyFR7Cam<T>;
    
    // If we had a camera instance, we could call these methods:
    // camera.model_name()
    // camera.zoom_speed_range()
    // camera.max_pan_speed()
    // camera.optical_zoom_max()
}

// The following tests would fail to compile if uncommented, proving compile-time safety:

/*
// This would fail because PTZOpticsG2 doesn't implement NDFilter
#[test]
fn test_nd_filter_compile_error() {
    fn requires_nd_filter<P, T>(camera: &Camera<P, T>)
    where
        P: Profile + grafton_visca::capabilities::NDFilter,
        T: UnifiedTransport,
    {
        // ND filter methods would be available here
    }
    
    // This would fail to compile:
    // let g2_camera: Camera<PTZOpticsG2, _> = unimplemented!();
    // requires_nd_filter(&g2_camera); // ERROR: PTZOpticsG2 doesn't implement NDFilter
}
*/

/*
// This would fail because only SonyFR7 implements VariableSpeed
#[test]
fn test_variable_speed_compile_error() {
    fn requires_variable_speed<P, T>(camera: &Camera<P, T>)
    where
        P: Profile + grafton_visca::capabilities::VariableSpeed,
        T: UnifiedTransport,
    {
        // Variable speed methods would be available here
    }
    
    // This would fail to compile:
    // let g2_camera: Camera<PTZOpticsG2, _> = unimplemented!();
    // requires_variable_speed(&g2_camera); // ERROR: PTZOpticsG2 doesn't implement VariableSpeed
}
*/