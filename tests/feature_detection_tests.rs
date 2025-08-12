//! Tests demonstrating compile-time feature safety with camera profiles.
//!
//! In the mode-based Camera API, feature support is determined at compile-time
//! through trait bounds and mode markers rather than runtime checks.

use grafton_visca::{
    camera::{AsyncMode, BlockingMode, Camera},
    capabilities::{MotionSync, NDFilter, Profile, VariableSpeed},
    command::resolution::NDFilterPosition,
    transport::{AsyncTransport, BlockingTransport},
};

// These tests demonstrate that the code compiles correctly with proper trait bounds
// and mode separation. The actual runtime tests are in other test files.

#[test]
fn test_nd_filter_compile_time_safety() {
    // Async version - functions that require ND filter support
    #[cfg(feature = "async")]
    fn _set_nd_filter_async<P, T>(_camera: &Camera<AsyncMode, P, T>, _position: NDFilterPosition)
    where
        P: Profile + NDFilter,
        T: AsyncTransport + Send + Sync + 'static,
    {
        // This function can only be called with cameras that have ND filter
        // The trait bound P: NDFilter enforces this at compile time
    }

    // Blocking version - functions that require ND filter support
    #[cfg(not(feature = "async"))]
    fn _set_nd_filter_blocking<P, T>(
        _camera: &Camera<BlockingMode, P, T>,
        _position: NDFilterPosition,
    ) where
        P: Profile + NDFilter,
        T: BlockingTransport + Send + Sync + 'static,
    {
        // This function can only be called with cameras that have ND filter
        // The trait bound P: NDFilter enforces this at compile time
    }

    // Test passes if compilation succeeds
}

#[test]
fn test_motion_sync_compile_time_safety() {
    // Async version - functions that require motion sync support
    #[cfg(feature = "async")]
    fn _enable_motion_sync_async<P, T>(
        _camera: &Camera<AsyncMode, P, T>,
        _mode: grafton_visca::command::MotionSyncMode,
    ) where
        P: Profile + MotionSync,
        T: AsyncTransport + Send + Sync + 'static,
    {
        // Only cameras with MotionSync can compile this function
    }

    // Blocking version - functions that require motion sync support
    #[cfg(not(feature = "async"))]
    fn _enable_motion_sync_blocking<P, T>(
        _camera: &Camera<BlockingMode, P, T>,
        _mode: grafton_visca::command::MotionSyncMode,
    ) where
        P: Profile + MotionSync,
        T: BlockingTransport + Send + Sync + 'static,
    {
        // Only cameras with MotionSync can compile this function
    }

    // Test passes if compilation succeeds
}

#[test]
fn test_variable_speed_compile_time_safety() {
    // Async version - functions that require variable speed support
    #[cfg(feature = "async")]
    fn _set_variable_speed_mode_async<P, T>(
        _camera: &Camera<AsyncMode, P, T>,
        _mode: grafton_visca::command::VariableSpeedMode,
    ) where
        P: Profile + VariableSpeed,
        T: AsyncTransport + Send + Sync + 'static,
    {
        // Only cameras with VariableSpeed can compile this function
    }

    // Blocking version - functions that require variable speed support
    #[cfg(not(feature = "async"))]
    fn _set_variable_speed_mode_blocking<P, T>(
        _camera: &Camera<BlockingMode, P, T>,
        _mode: grafton_visca::command::VariableSpeedMode,
    ) where
        P: Profile + VariableSpeed,
        T: BlockingTransport + Send + Sync + 'static,
    {
        // Only cameras with VariableSpeed can compile this function
    }

    // Test passes if compilation succeeds
}

#[test]
fn test_basic_features_available_to_all() {
    // All cameras have basic features through the Profile trait

    #[cfg(feature = "async")]
    {
        fn _basic_operations_async<P, T>(_camera: &Camera<AsyncMode, P, T>)
        where
            P: Profile,
            T: AsyncTransport + Send + Sync + 'static,
        {
            // All cameras can use basic operations like:
            // - Power on/off
            // - Pan/tilt/zoom
            // - Focus control
            // - Exposure settings
            // - White balance
            // - Presets
            // These are guaranteed by the Profile trait
        }
    }

    #[cfg(not(feature = "async"))]
    {
        fn _basic_operations_blocking<P, T>(_camera: &Camera<BlockingMode, P, T>)
        where
            P: Profile,
            T: BlockingTransport + Send + Sync + 'static,
        {
            // All cameras can use basic operations
        }
    }
}

#[test]
fn test_profile_specific_compile_time_checks() {
    use grafton_visca::camera::profiles::{GenericVisca, PTZOpticsG2, SonyFR7};

    // Functions that demonstrate profile-specific capabilities

    #[cfg(feature = "async")]
    {
        // SonyFR7 has NDFilter and VariableSpeed
        fn _sony_fr7_features_async<T>(_camera: &Camera<AsyncMode, SonyFR7, T>)
        where
            T: AsyncTransport + Send + Sync + 'static,
        {
            // This compiles because SonyFR7 implements NDFilter and VariableSpeed
            fn requires_nd<P: NDFilter>() {}
            fn requires_var_speed<P: VariableSpeed>() {}

            requires_nd::<SonyFR7>();
            requires_var_speed::<SonyFR7>();
        }

        // PTZOpticsG2 has MotionSync
        fn _ptzoptics_g2_features_async<T>(_camera: &Camera<AsyncMode, PTZOpticsG2, T>)
        where
            T: AsyncTransport + Send + Sync + 'static,
        {
            // This compiles because PTZOpticsG2 implements MotionSync
            fn requires_motion_sync<P: MotionSync>() {}

            requires_motion_sync::<PTZOpticsG2>();
        }

        // GenericVisca only has basic features
        fn _generic_visca_features_async<T>(_camera: &Camera<AsyncMode, GenericVisca, T>)
        where
            T: AsyncTransport + Send + Sync + 'static,
        {
            // GenericVisca doesn't have NDFilter, MotionSync, or VariableSpeed
            // So we can only use basic Profile features
            fn requires_profile<P: Profile>() {}

            requires_profile::<GenericVisca>();
        }
    }

    #[cfg(not(feature = "async"))]
    {
        // Same checks for blocking mode
        fn _sony_fr7_features_blocking<T>(_camera: &Camera<BlockingMode, SonyFR7, T>)
        where
            T: BlockingTransport + Send + Sync + 'static,
        {
            fn requires_nd<P: NDFilter>() {}
            fn requires_var_speed<P: VariableSpeed>() {}

            requires_nd::<SonyFR7>();
            requires_var_speed::<SonyFR7>();
        }

        fn _ptzoptics_g2_features_blocking<T>(_camera: &Camera<BlockingMode, PTZOpticsG2, T>)
        where
            T: BlockingTransport + Send + Sync + 'static,
        {
            fn requires_motion_sync<P: MotionSync>() {}

            requires_motion_sync::<PTZOpticsG2>();
        }

        fn _generic_visca_features_blocking<T>(_camera: &Camera<BlockingMode, GenericVisca, T>)
        where
            T: BlockingTransport + Send + Sync + 'static,
        {
            fn requires_profile<P: Profile>() {}

            requires_profile::<GenericVisca>();
        }
    }
}

#[test]
fn test_mode_transport_consistency() {
    // Ensure that async mode requires async transport and blocking mode requires blocking transport

    #[cfg(feature = "async")]
    {
        // This compiles: AsyncMode with AsyncTransport
        fn _correct_async<P, T>(_camera: &Camera<AsyncMode, P, T>)
        where
            P: Profile,
            T: AsyncTransport,
        {
            // Correct pairing
        }

        // This would NOT compile (if uncommented):
        // fn _incorrect_async<P, T>(_camera: &Camera<AsyncMode, P, T>)
        // where
        //     P: Profile,
        //     T: BlockingTransport,  // Wrong transport type for AsyncMode
        // {
        // }
    }

    #[cfg(not(feature = "async"))]
    {
        // This compiles: BlockingMode with BlockingTransport
        fn _correct_blocking<P, T>(_camera: &Camera<BlockingMode, P, T>)
        where
            P: Profile,
            T: BlockingTransport,
        {
            // Correct pairing
        }

        // This would NOT compile (if uncommented):
        // fn _incorrect_blocking<P, T>(_camera: &Camera<BlockingMode, P, T>)
        // where
        //     P: Profile,
        //     T: AsyncTransport,  // Wrong transport type for BlockingMode
        // {
        // }
    }
}
