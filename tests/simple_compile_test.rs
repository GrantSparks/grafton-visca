//! Simple test to verify compilation succeeds with generic Camera implementation.
//!
//! This test demonstrates the mode-based Camera API with compile-time mode selection.

use grafton_visca::{
    camera::Camera,
    capabilities::{NDFilter, Profile},
};

#[cfg(feature = "async")]
use grafton_visca::{
    camera::AsyncMode,
    prelude::r#async::{GenericViscaCam, PtzOpticsG2Cam, SonyFR7Cam},
    transport::async_transport::AsyncTransport,
};

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::BlockingMode,
    prelude::blocking::{GenericViscaCam, PtzOpticsG2Cam, SonyFR7Cam},
    transport::BlockingTransport,
};

#[test]
fn test_compilation_succeeds() {
    type _G2Camera<T> = PtzOpticsG2Cam<T>;
    type _FR7Camera<T> = SonyFR7Cam<T>;
    type _GenericCamera<T> = GenericViscaCam<T>;
}

#[test]
fn test_compile_time_safety() {
    #[cfg(feature = "async")]
    fn _use_nd_filter_async<P, T>(_camera: &Camera<AsyncMode, P, T>)
    where
        P: Profile + NDFilter,
        T: AsyncTransport + Send + Sync + 'static,
    {
    }

    #[cfg(not(feature = "async"))]
    fn _use_nd_filter_blocking<P, T>(_camera: &Camera<BlockingMode, P, T>)
    where
        P: Profile + NDFilter,
        T: BlockingTransport + Send + Sync + 'static,
    {
    }

    #[cfg(feature = "async")]
    fn _use_basic_features_async<P, T>(_camera: &Camera<AsyncMode, P, T>)
    where
        P: Profile,
        T: AsyncTransport + Send + Sync + 'static,
    {
    }

    #[cfg(not(feature = "async"))]
    fn _use_basic_features_blocking<P, T>(_camera: &Camera<BlockingMode, P, T>)
    where
        P: Profile,
        T: BlockingTransport + Send + Sync + 'static,
    {
    }
}

#[test]
fn test_mode_specific_apis() {
    #[cfg(feature = "async")]
    {
        fn _async_only<P, T>(_camera: &Camera<AsyncMode, P, T>)
        where
            P: Profile,
            T: AsyncTransport,
        {
        }
    }

    #[cfg(not(feature = "async"))]
    {
        fn _blocking_only<P, T>(_camera: &Camera<BlockingMode, P, T>)
        where
            P: Profile,
            T: BlockingTransport,
        {
        }
    }
}
