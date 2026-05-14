//! Tests demonstrating compile-time feature safety with camera profiles.
//!
//! In the mode-based Camera API, feature support is determined at compile-time
//! through trait bounds and mode markers rather than runtime checks.

#[cfg(feature = "mode-async")]
use grafton_visca::{camera::AsyncCamera, transport::AsyncTransport, Executor};

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{transport::BlockingTransportHandle, BlockingClient};

use grafton_visca::{
    capabilities::{HasMotionSync, HasNdFilter, HasVariableSpeed, Profile},
    NdFilterPosition,
};

// These tests demonstrate compile-time trait bounds and mode separation.

#[test]
fn test_nd_filter_compile_time_safety() {
    // Async version - functions that require ND filter support
    #[cfg(feature = "mode-async")]
    fn _set_nd_filter_async<P, T, E>(_camera: &AsyncCamera<P, T, E>, _position: NdFilterPosition)
    where
        P: Profile + HasNdFilter,
        T: AsyncTransport + Send + Sync + 'static,
        E: Executor,
    {
        // This function can only be called with cameras that have ND filter
        // The trait bound P: HasNdFilter enforces this at compile time
    }

    // Blocking version - functions that require ND filter support
    #[cfg(not(feature = "mode-async"))]
    fn _set_nd_filter_blocking<P>(
        _camera: &BlockingClient<P, BlockingTransportHandle>,
        _position: NdFilterPosition,
    ) where
        P: Profile + HasNdFilter,
    {
        // This function can only be called with cameras that have ND filter
        // The trait bound P: HasNdFilter enforces this at compile time
    }

    // Test passes if compilation succeeds
}

#[test]
fn test_motion_sync_compile_time_safety() {
    // Async version - functions that require motion sync support
    #[cfg(feature = "mode-async")]
    fn _enable_motion_sync_async<P, T, E>(
        _camera: &AsyncCamera<P, T, E>,
        _mode: grafton_visca::command::MotionSyncMode,
    ) where
        P: Profile + HasMotionSync,
        T: AsyncTransport + Send + Sync + 'static,
        E: Executor,
    {
        // Only cameras with typed Motion Sync support can compile this function
    }

    // Blocking version - functions that require motion sync support
    #[cfg(not(feature = "mode-async"))]
    fn _enable_motion_sync_blocking<P>(
        _camera: &BlockingClient<P, BlockingTransportHandle>,
        _mode: grafton_visca::command::MotionSyncMode,
    ) where
        P: Profile + HasMotionSync,
    {
        // Only cameras with typed Motion Sync support can compile this function
    }

    // Test passes if compilation succeeds
}

#[test]
fn test_variable_speed_compile_time_safety() {
    // Async version - functions that require variable speed support
    #[cfg(feature = "mode-async")]
    fn _set_variable_speed_mode_async<P, T, E>(
        _camera: &AsyncCamera<P, T, E>,
        _mode: grafton_visca::command::VariableSpeedMode,
    ) where
        P: Profile + HasVariableSpeed,
        T: AsyncTransport + Send + Sync + 'static,
        E: Executor,
    {
        // Only cameras with typed variable speed support can compile this function
    }

    // Blocking version - functions that require variable speed support
    #[cfg(not(feature = "mode-async"))]
    fn _set_variable_speed_mode_blocking<P>(
        _camera: &BlockingClient<P, BlockingTransportHandle>,
        _mode: grafton_visca::command::VariableSpeedMode,
    ) where
        P: Profile + HasVariableSpeed,
    {
        // Only cameras with typed variable speed support can compile this function
    }

    // Test passes if compilation succeeds
}

#[test]
fn test_basic_features_available_to_all() {
    // All cameras have basic features through the Profile trait

    #[cfg(feature = "mode-async")]
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

    #[cfg(not(feature = "mode-async"))]
    {
        fn _basic_operations_blocking<P>(_camera: &BlockingClient<P, BlockingTransportHandle>)
        where
            P: Profile,
        {
            // All cameras can use basic operations
        }
    }
}

#[test]
fn test_profile_specific_compile_time_checks() {
    use grafton_visca::camera::profiles::{GenericVisca, SonyFR7};

    // Functions that demonstrate profile-specific capabilities

    #[cfg(feature = "mode-async")]
    {
        // SonyFR7 has typed ND filter and variable speed support
        fn _sony_fr7_features_async<T, E>(_camera: &AsyncCamera<SonyFR7, T, E>)
        where
            T: AsyncTransport + Send + Sync + 'static,
            E: Executor,
        {
            // This compiles because SonyFR7 implements the support markers.
            fn requires_nd<P: HasNdFilter>() {}
            fn requires_var_speed<P: HasVariableSpeed>() {}

            requires_nd::<SonyFR7>();
            requires_var_speed::<SonyFR7>();
        }

        // GenericVisca only has basic features
        fn _generic_visca_features_async<T, E>(_camera: &AsyncCamera<GenericVisca, T, E>)
        where
            T: AsyncTransport + Send + Sync + 'static,
            E: Executor,
        {
            // GenericVisca doesn't have typed optional vendor feature support.
            // So we can only use basic Profile features
            fn requires_profile<P: Profile>() {}

            requires_profile::<GenericVisca>();
        }
    }

    #[cfg(not(feature = "mode-async"))]
    {
        // Same checks for blocking mode
        fn _sony_fr7_features_blocking(_camera: &BlockingClient<SonyFR7, BlockingTransportHandle>) {
            fn requires_nd<P: HasNdFilter>() {}
            fn requires_var_speed<P: HasVariableSpeed>() {}

            requires_nd::<SonyFR7>();
            requires_var_speed::<SonyFR7>();
        }

        fn _generic_visca_features_blocking(
            _camera: &BlockingClient<GenericVisca, BlockingTransportHandle>,
        ) {
            fn requires_profile<P: Profile>() {}

            requires_profile::<GenericVisca>();
        }
    }
}

#[test]
fn test_mode_transport_consistency() {
    // Ensure that async mode requires async transport and blocking mode requires blocking transport

    #[cfg(feature = "mode-async")]
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
        //     T: BlockingTransport,  // Wrong transport type for AsyncMode
        // {
        // }
    }

    #[cfg(not(feature = "mode-async"))]
    {
        // This compiles: BlockingMode with BlockingTransportHandle
        fn _correct_blocking<P>(_camera: &BlockingClient<P, BlockingTransportHandle>)
        where
            P: Profile,
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
