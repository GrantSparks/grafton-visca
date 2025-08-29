//! Tests demonstrating compile-time feature safety with camera profiles.
//!
//! In the mode-based Camera API, feature support is determined at compile-time
//! through trait bounds and mode markers rather than runtime checks.

#[cfg(feature = "async")]
use grafton_visca::{camera::AsyncCamera, transport::AsyncTransport, Executor};

#[cfg(not(feature = "async"))]
use grafton_visca::{camera::BlockingCamera, transport::SyncTransport};

use grafton_visca::{
    capabilities::{MotionSync, NDFilter, Profile, VariableSpeed},
    command::resolution::NDFilterPosition,
};

// These tests demonstrate that the code compiles correctly with proper trait bounds
// and mode separation. The actual runtime tests are in other test files.

#[test]
fn test_nd_filter_compile_time_safety() {
    // Async version - functions that require ND filter support
    #[cfg(feature = "async")]
    fn _set_nd_filter_async<P, T, E>(_camera: &AsyncCamera<P, T, E>, _position: NDFilterPosition)
    where
        P: Profile + NDFilter,
        T: AsyncTransport + Send + Sync + 'static,
        E: Executor,
    {
        // This function can only be called with cameras that have ND filter
        // The trait bound P: NDFilter enforces this at compile time
    }

    // Blocking version - functions that require ND filter support
    #[cfg(not(feature = "async"))]
    fn _set_nd_filter_blocking<P, T>(_camera: &BlockingCamera<P, T>, _position: NDFilterPosition)
    where
        P: Profile + NDFilter,
        T: SyncTransport + Send + Sync + 'static,
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
    fn _enable_motion_sync_async<P, T, E>(
        _camera: &AsyncCamera<P, T, E>,
        _mode: grafton_visca::command::MotionSyncMode,
    ) where
        P: Profile + MotionSync,
        T: AsyncTransport + Send + Sync + 'static,
        E: Executor,
    {
        // Only cameras with MotionSync can compile this function
    }

    // Blocking version - functions that require motion sync support
    #[cfg(not(feature = "async"))]
    fn _enable_motion_sync_blocking<P, T>(
        _camera: &BlockingCamera<P, T>,
        _mode: grafton_visca::command::MotionSyncMode,
    ) where
        P: Profile + MotionSync,
        T: SyncTransport + Send + Sync + 'static,
    {
        // Only cameras with MotionSync can compile this function
    }

    // Test passes if compilation succeeds
}

#[test]
fn test_variable_speed_compile_time_safety() {
    // Async version - functions that require variable speed support
    #[cfg(feature = "async")]
    fn _set_variable_speed_mode_async<P, T, E>(
        _camera: &AsyncCamera<P, T, E>,
        _mode: grafton_visca::command::VariableSpeedMode,
    ) where
        P: Profile + VariableSpeed,
        T: AsyncTransport + Send + Sync + 'static,
        E: Executor,
    {
        // Only cameras with VariableSpeed can compile this function
    }

    // Blocking version - functions that require variable speed support
    #[cfg(not(feature = "async"))]
    fn _set_variable_speed_mode_blocking<P, T>(
        _camera: &BlockingCamera<P, T>,
        _mode: grafton_visca::command::VariableSpeedMode,
    ) where
        P: Profile + VariableSpeed,
        T: SyncTransport + Send + Sync + 'static,
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
        fn _basic_operations_async<P, T, E>(_camera: &AsyncCamera<P, T, E>)
        where
            P: Profile,
            T: AsyncTransport + Send + Sync + 'static,
            E: Executor,
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
        fn _basic_operations_blocking<P, T>(_camera: &BlockingCamera<P, T>)
        where
            P: Profile,
            T: SyncTransport + Send + Sync + 'static,
        {
            // All cameras can use basic operations
        }
    }
}

#[test]
fn test_profile_specific_compile_time_checks() {
    use grafton_visca::camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7};

    // Functions that demonstrate profile-specific capabilities

    #[cfg(feature = "async")]
    {
        // SonyFR7 has NDFilter and VariableSpeed
        fn _sony_fr7_features_async<T, E>(_camera: &AsyncCamera<SonyFR7, T, E>)
        where
            T: AsyncTransport + Send + Sync + 'static,
            E: Executor,
        {
            // This compiles because SonyFR7 implements NDFilter and VariableSpeed
            fn requires_nd<P: NDFilter>() {}
            fn requires_var_speed<P: VariableSpeed>() {}

            requires_nd::<SonyFR7>();
            requires_var_speed::<SonyFR7>();
        }

        // PtzOpticsG2 has MotionSync
        fn _ptzoptics_g2_features_async<T, E>(_camera: &AsyncCamera<PtzOpticsG2, T, E>)
        where
            T: AsyncTransport + Send + Sync + 'static,
            E: Executor,
        {
            // This compiles because PtzOpticsG2 implements MotionSync
            fn requires_motion_sync<P: MotionSync>() {}

            requires_motion_sync::<PtzOpticsG2>();
        }

        // GenericVisca only has basic features
        fn _generic_visca_features_async<T, E>(_camera: &AsyncCamera<GenericVisca, T, E>)
        where
            T: AsyncTransport + Send + Sync + 'static,
            E: Executor,
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
        fn _sony_fr7_features_blocking<T>(_camera: &BlockingCamera<SonyFR7, T>)
        where
            T: SyncTransport + Send + Sync + 'static,
        {
            fn requires_nd<P: NDFilter>() {}
            fn requires_var_speed<P: VariableSpeed>() {}

            requires_nd::<SonyFR7>();
            requires_var_speed::<SonyFR7>();
        }

        fn _ptzoptics_g2_features_blocking<T>(_camera: &BlockingCamera<PtzOpticsG2, T>)
        where
            T: SyncTransport + Send + Sync + 'static,
        {
            fn requires_motion_sync<P: MotionSync>() {}

            requires_motion_sync::<PtzOpticsG2>();
        }

        fn _generic_visca_features_blocking<T>(_camera: &BlockingCamera<GenericVisca, T>)
        where
            T: SyncTransport + Send + Sync + 'static,
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
        fn _correct_async<P, T, E>(_camera: &AsyncCamera<P, T, E>)
        where
            P: Profile,
            T: AsyncTransport,
            E: Executor,
        {
            // Correct pairing
        }

        // This would NOT compile (if uncommented):
        // fn _incorrect_async<P, T>(_camera: &BlockingCamera<P, T>)
        // where
        //     P: Profile,
        //     T: SyncTransport,  // Wrong transport type for AsyncMode
        // {
        // }
    }

    #[cfg(not(feature = "async"))]
    {
        // This compiles: BlockingMode with SyncTransport
        fn _correct_blocking<P, T>(_camera: &BlockingCamera<P, T>)
        where
            P: Profile,
            T: SyncTransport,
        {
            // Correct pairing
        }

        // This would NOT compile (if uncommented):
        // fn _incorrect_blocking<P, T>(_camera: &BlockingCamera<P, T>)
        // where
        //     P: Profile,
        //     T: AsyncTransport,  // Wrong transport type for BlockingMode
        // {
        // }
    }
}
