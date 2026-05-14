//! Tests for camera profile system and compile-time capabilities.
//!
//! This test suite validates the mode-based Camera API with profile capabilities.

#[cfg(feature = "mode-async")]
use grafton_visca::{camera::AsyncCamera, transport::AsyncTransport, Executor};

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{
    camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    transport::BlockingTransport,
    BlockingCamera,
};

use grafton_visca::capabilities::*;

#[test]
fn test_profile_type_aliases() {
    // Verify that type aliases exist and compile for both modes

    #[cfg(feature = "mode-async")]
    {
        #[cfg(feature = "runtime-tokio")]
        {
            let _: Option<
                AsyncCamera<
                    grafton_visca::camera::profiles::PtzOpticsG2,
                    (),
                    grafton_visca::TokioExecutor,
                >,
            > = None;
            let _: Option<
                AsyncCamera<
                    grafton_visca::camera::profiles::SonyFR7,
                    (),
                    grafton_visca::TokioExecutor,
                >,
            > = None;
            let _: Option<
                AsyncCamera<
                    grafton_visca::camera::profiles::GenericVisca,
                    (),
                    grafton_visca::TokioExecutor,
                >,
            > = None;
        }

        #[cfg(all(not(feature = "runtime-tokio"), feature = "runtime-smol"))]
        {
            let _: Option<
                AsyncCamera<
                    grafton_visca::camera::profiles::PtzOpticsG2,
                    (),
                    grafton_visca::SmolExecutor,
                >,
            > = None;
            let _: Option<
                AsyncCamera<
                    grafton_visca::camera::profiles::SonyFR7,
                    (),
                    grafton_visca::SmolExecutor,
                >,
            > = None;
            let _: Option<
                AsyncCamera<
                    grafton_visca::camera::profiles::GenericVisca,
                    (),
                    grafton_visca::SmolExecutor,
                >,
            > = None;
        }
    }

    #[cfg(not(feature = "mode-async"))]
    {
        fn _accepts_g2_camera_blocking<T>(_camera: BlockingCamera<PtzOpticsG2, T>)
        where
            T: BlockingTransport + Send + Sync + 'static,
        {
            // BlockingCamera<P, T> for blocking mode
        }

        fn _accepts_fr7_camera_blocking<T>(_camera: BlockingCamera<SonyFR7, T>)
        where
            T: BlockingTransport + Send + Sync + 'static,
        {
            // BlockingCamera<P, T> for blocking mode
        }

        fn _accepts_generic_camera_blocking<T>(_camera: BlockingCamera<GenericVisca, T>)
        where
            T: BlockingTransport + Send + Sync + 'static,
        {
            // BlockingCamera<P, T> for blocking mode
        }
    }

    // Test passes if code compiles
}

#[test]
fn test_profile_capabilities_are_compile_time() {
    // Mode-specific functions demonstrating compile-time capability checking

    #[cfg(feature = "mode-async")]
    {
        // This function can only accept async cameras with ND filter support
        fn _requires_nd_filter_async<P, T, E>(_camera: &AsyncCamera<P, T, E>) -> bool
        where
            P: Profile + HasNdFilter,
            T: AsyncTransport + Send + Sync + 'static,
            E: Executor + Send + Sync + 'static,
        {
            // At compile time, we know this camera supports ND filter
            true
        }

        // This function can accept any async camera with basic Profile
        fn _requires_only_basic_async<P, T, E>(_camera: &AsyncCamera<P, T, E>) -> bool
        where
            P: Profile,
            T: AsyncTransport + Send + Sync + 'static,
            E: Executor + Send + Sync + 'static,
        {
            // All cameras have basic capabilities
            true
        }
    }

    #[cfg(not(feature = "mode-async"))]
    {
        // This function can only accept blocking cameras with ND filter support
        fn _requires_nd_filter_blocking<P, T>(_camera: &BlockingCamera<P, T>) -> bool
        where
            P: Profile + HasNdFilter,
            T: BlockingTransport + Send + Sync + 'static,
        {
            // At compile time, we know this camera supports ND filter
            true
        }

        // This function can accept any blocking camera with basic Profile
        fn _requires_only_basic_blocking<P, T>(_camera: &BlockingCamera<P, T>) -> bool
        where
            P: Profile,
            T: BlockingTransport + Send + Sync + 'static,
        {
            // All cameras have basic capabilities
            true
        }
    }

    // These demonstrate compile-time checking:
    // - SonyFR7 has typed ND filter support, so it can use requires_nd_filter
    // - GenericVisca doesn't have typed ND filter support, so it cannot
    // - Both can use requires_only_basic
}

#[test]
fn test_profile_traits_composition() {
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
        fn requires_menu<T: MenuCapability>() {}

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
    verify_profile_requirements::<grafton_visca::camera::profiles::PtzOpticsG2>();
    verify_profile_requirements::<grafton_visca::camera::profiles::SonyFR7>();
    verify_profile_requirements::<grafton_visca::camera::profiles::GenericVisca>();
}

#[test]
fn test_optional_capabilities() {
    // Test which profiles have optional capabilities
    fn has_nd_filter<T: HasNdFilter>() {}
    fn has_variable_speed<T: HasVariableSpeed>() {}

    // These compile:
    has_nd_filter::<grafton_visca::camera::profiles::SonyFR7>();
    // Note: SonyFR7 doesn't have typed Motion Sync support.
    has_variable_speed::<grafton_visca::camera::profiles::SonyFR7>();

    // These would NOT compile (commented out to keep test passing):
    // has_nd_filter::<grafton_visca::camera::profiles::PtzOpticsG2>();
    // has_nd_filter::<grafton_visca::camera::profiles::GenericVisca>();
    // has_variable_speed::<grafton_visca::camera::profiles::PtzOpticsG2>();
    // has_variable_speed::<grafton_visca::camera::profiles::GenericVisca>();
    //
    // The API contract compile-fail fixtures cover unsupported Motion Sync
    // markers for built-in PTZOptics profiles, GenericVisca, and SonyFR7.
}

#[test]
fn test_mode_separation() {
    // Demonstrate that async and blocking modes are completely separate types

    #[cfg(feature = "mode-async")]
    {
        // This function only accepts AsyncCamera
        fn _async_mode_only<P, T, E>(_camera: &AsyncCamera<P, T, E>)
        where
            P: Profile,
            T: AsyncTransport,
            E: Executor,
        {
            // AsyncCamera has async methods
        }

        // The following would NOT compile:
        // fn _wrong_mode<P, T>(_camera: &Camera<P, T>)
        // where T: AsyncTransport { }
    }

    #[cfg(not(feature = "mode-async"))]
    {
        // This function only accepts BlockingCamera
        fn _blocking_mode_only<P, T>(_camera: &BlockingCamera<P, T>)
        where
            P: Profile,
            T: BlockingTransport,
        {
            // BlockingCamera has synchronous methods
        }

        // The following would NOT compile:
        // fn _wrong_mode<P, T>(_camera: &Camera<P, T>)
        // where T: BlockingTransport { }
    }
}
