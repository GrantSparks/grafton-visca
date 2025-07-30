//! Tests for camera profile system and compile-time capabilities.

use grafton_visca::{capabilities::*, transport::UnifiedTransport, Camera};

// Import type aliases from the blocking prelude
use grafton_visca::prelude::blocking::{GenericViscaCam, PTZOpticsG2Cam, SonyFR7Cam};

// Tests demonstrating the compile-time profile system

#[test]
fn test_profile_type_aliases() {
    // Verify that type aliases exist and compile
    fn _accepts_g2_camera<T>(_camera: PTZOpticsG2Cam<T>)
    where
        T: UnifiedTransport,
    {
        // PTZOpticsG2Cam is a type alias for Camera<PTZOpticsG2, T>
    }

    fn _accepts_fr7_camera<T>(_camera: SonyFR7Cam<T>)
    where
        T: UnifiedTransport,
    {
        // SonyFR7Cam is a type alias for Camera<SonyFR7, T>
    }

    fn _accepts_generic_camera<T>(_camera: GenericViscaCam<T>)
    where
        T: UnifiedTransport,
    {
        // GenericViscaCam is a type alias for Camera<GenericVisca, T>
    }

    // Test passes if code compiles
}

#[test]
fn test_profile_capabilities_are_compile_time() {
    // This function can only accept cameras with ND filter support
    fn _requires_nd_filter<P, T>(_camera: &Camera<P, T>) -> bool
    where
        P: Profile + NDFilter,
        T: UnifiedTransport,
    {
        // At compile time, we know this camera supports ND filter
        true
    }

    // This function can accept any camera with basic Profile
    fn _requires_only_basic<P, T>(_camera: &Camera<P, T>) -> bool
    where
        P: Profile,
        T: UnifiedTransport,
    {
        // All cameras have basic capabilities
        true
    }

    // These demonstrate compile-time checking:
    // - SonyFR7 has NDFilter, so it can use requires_nd_filter
    // - GenericVisca doesn't have NDFilter, so it cannot
    // - Both can use requires_only_basic
}

#[test]
fn test_profile_traits_composition() {
    use grafton_visca::capabilities::*;

    // Verify that Profile trait requires all basic capabilities
    fn verify_profile_requirements<P>()
    where
        P: Profile,
    {
        // A type implementing Profile must also implement:
        fn requires_metadata<T: ProfileMetadata>() {}
        fn requires_pan_tilt<T: PanTilt>() {}
        fn requires_zoom<T: Zoom>() {}
        fn requires_focus<T: Focus>() {}
        fn requires_exposure<T: Exposure>() {}
        fn requires_white_balance<T: WhiteBalance>() {}
        fn requires_image_processing<T: ImageProcessing>() {}
        fn requires_presets<T: Presets>() {}
        fn requires_power<T: Power>() {}
        fn requires_menu<T: MenuControl>() {}

        // This would only compile if P implements all these traits
        requires_metadata::<P>();
        requires_pan_tilt::<P>();
        requires_zoom::<P>();
        requires_focus::<P>();
        requires_exposure::<P>();
        requires_white_balance::<P>();
        requires_image_processing::<P>();
        requires_presets::<P>();
        requires_power::<P>();
        requires_menu::<P>();
    }

    // Test that known profiles implement Profile correctly
    verify_profile_requirements::<grafton_visca::camera::profiles::PTZOpticsG2>();
    verify_profile_requirements::<grafton_visca::camera::profiles::SonyFR7>();
    verify_profile_requirements::<grafton_visca::camera::profiles::GenericVisca>();
}

#[test]
fn test_optional_capabilities() {
    use grafton_visca::capabilities::*;

    // Test which profiles have optional capabilities
    fn has_nd_filter<T: NDFilter>() {}
    fn has_motion_sync<T: MotionSync>() {}
    fn has_variable_speed<T: VariableSpeed>() {}

    // These compile:
    has_nd_filter::<grafton_visca::camera::profiles::SonyFR7>();
    has_motion_sync::<grafton_visca::camera::profiles::PTZOpticsG2>();
    // Note: SonyFR7 doesn't have MotionSync in the current implementation
    has_variable_speed::<grafton_visca::camera::profiles::SonyFR7>();

    // These would NOT compile (commented out to keep test passing):
    // has_nd_filter::<grafton_visca::camera::profiles::PTZOpticsG2>();
    // has_nd_filter::<grafton_visca::camera::profiles::GenericVisca>();
    // has_motion_sync::<grafton_visca::camera::profiles::GenericVisca>();
    // has_variable_speed::<grafton_visca::camera::profiles::PTZOpticsG2>();
    // has_variable_speed::<grafton_visca::camera::profiles::GenericVisca>();
}
