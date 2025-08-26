//! Simple test to verify compilation succeeds with generic Camera implementation.

#[cfg(feature = "async")]
use grafton_visca::{
    camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    camera::{AsyncMode, Camera},
    transport::async_transport::AsyncTransport,
};

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::{BlockingMode, Camera},
    transport::BlockingTransport,
};

use grafton_visca::capabilities::{NDFilter, Profile};

#[test]
fn test_compilation_succeeds() {
    #[cfg(feature = "async")]
    type _G2Camera<T> = Camera<AsyncMode, PtzOpticsG2, T>;
    #[cfg(feature = "async")]
    type _FR7Camera<T> = Camera<AsyncMode, SonyFR7, T>;
    #[cfg(feature = "async")]
    type _GenericCamera<T> = Camera<AsyncMode, GenericVisca, T>;

    #[cfg(not(feature = "async"))]
    type _G2Camera<T> = grafton_visca::prelude::blocking::PtzOpticsG2Cam<T>;
    #[cfg(not(feature = "async"))]
    type _FR7Camera<T> = grafton_visca::prelude::blocking::SonyFR7Cam<T>;
    #[cfg(not(feature = "async"))]
    type _GenericCamera<T> = grafton_visca::prelude::blocking::GenericViscaCam<T>;
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
