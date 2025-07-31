//! Tests demonstrating compile-time feature safety with camera profiles.
//!
//! In the new generic API, feature support is determined at compile-time
//! through trait bounds rather than runtime checks.

use grafton_visca::{
    camera::generic::Camera,
    capabilities::{MotionSync, NDFilter, Profile, VariableSpeed},
    transport::UnifiedTransport,
};

#[cfg(feature = "tokio")]
use grafton_visca::Error;

// These tests demonstrate that the code compiles correctly with proper trait bounds

#[test]
fn test_ptzoptics_g2_compile_time_features() {
    // This function requires a camera with basic features (all cameras have these)
    fn _use_basic_features<P, T>(_camera: &Camera<P, T>)
    where
        P: Profile,
        T: UnifiedTransport,
    {
        // All cameras implementing Profile support:
        // - PanTilt
        // - Zoom
        // - Focus
        // - Exposure
        // - WhiteBalance
        // - ImageProcessing
        // - Power
        // - Presets
        // - MenuControl
    }

    // PTZOptics G2 implements MotionSync
    fn _use_motion_sync<P, T>(_camera: &Camera<P, T>)
    where
        P: Profile + MotionSync,
        T: UnifiedTransport,
    {
        // Only cameras with MotionSync trait can call this
    }

    // PTZOptics G2 does NOT implement NDFilter
    fn _use_nd_filter<P, T>(_camera: &Camera<P, T>)
    where
        P: Profile + NDFilter,
        T: UnifiedTransport,
    {
        // Only cameras with NDFilter trait can call this
    }

    // This would compile:
    // let g2_camera: PTZOpticsG2Cam<SomeTransport> = ...;
    // use_basic_features(&g2_camera);
    // use_motion_sync(&g2_camera);

    // This would NOT compile:
    // use_nd_filter(&g2_camera); // Error: PTZOpticsG2 doesn't implement NDFilter
}

#[test]
fn test_sony_fr7_compile_time_features() {
    // Sony FR7 has all the features PTZOptics G2 has, plus more

    // Sony FR7 implements NDFilter
    fn _use_nd_filter<P, T>(_camera: &Camera<P, T>)
    where
        P: Profile + NDFilter,
        T: UnifiedTransport,
    {
        // Sony FR7 can use this
    }

    // Sony FR7 implements VariableSpeed
    fn _use_variable_speed<P, T>(_camera: &Camera<P, T>)
    where
        P: Profile + VariableSpeed,
        T: UnifiedTransport,
    {
        // Sony FR7 can use this
    }

    // This would compile:
    // let fr7_camera: SonyFR7Cam<SomeTransport> = ...;
    // use_nd_filter(&fr7_camera);
    // use_variable_speed(&fr7_camera);
}

#[test]
fn test_generic_visca_limited_features() {
    // GenericVisca only has the basic Profile features

    fn _requires_advanced_features<P, T>(_camera: &Camera<P, T>)
    where
        P: Profile + NDFilter + MotionSync + VariableSpeed,
        T: UnifiedTransport,
    {
        // GenericVisca cannot be used here
    }

    // This would NOT compile:
    // let generic_camera: GenericViscaCam<SomeTransport> = ...;
    // requires_advanced_features(&generic_camera); // Error: GenericVisca lacks these traits
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_compile_time_feature_detection() {
    // Note: The library now uses compile-time feature detection exclusively.
    // Camera capabilities are determined by the marker traits implemented by each profile.

    // This approach provides better type safety and zero runtime overhead.

    // Example of compile-time safety in action:
    async fn _control_nd_filter<P, T>(_camera: &Camera<P, T>) -> Result<(), Error>
    where
        P: Profile + NDFilter,
        T: UnifiedTransport,
    {
        // camera.set_nd_filter_mode(NDFilterMode::Clear).await?;
        Ok(())
    }

    // This ensures only cameras with ND filter support can call this function
}
