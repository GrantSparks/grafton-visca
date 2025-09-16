//! Simple test to verify compilation succeeds with generic Camera implementation.

#[cfg(feature = "mode-async")]
use grafton_visca::{
    camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    camera::AsyncCamera,
    transport::async_transport::AsyncTransport,
};

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{transport::SyncTransport, BlockingCamera};

use grafton_visca::capabilities::{NdFilter, Profile};

#[test]
fn test_compilation_succeeds() {
    #[cfg(feature = "mode-async")]
    type _G2Camera<T, E> = AsyncCamera<PtzOpticsG2, T, E>;
    #[cfg(feature = "mode-async")]
    type _FR7Camera<T, E> = AsyncCamera<SonyFR7, T, E>;
    #[cfg(feature = "mode-async")]
    type _GenericCamera<T, E> = AsyncCamera<GenericVisca, T, E>;

    #[cfg(not(feature = "mode-async"))]
    type _G2Camera<T> = grafton_visca::prelude::blocking::PtzOpticsG2Cam<T>;
    #[cfg(not(feature = "mode-async"))]
    type _FR7Camera<T> = grafton_visca::prelude::blocking::SonyFR7Cam<T>;
    #[cfg(not(feature = "mode-async"))]
    type _GenericCamera<T> = grafton_visca::prelude::blocking::GenericViscaCam<T>;
}

#[test]
fn test_compile_time_safety() {
    #[cfg(feature = "mode-async")]
    fn _use_nd_filter_async<P, T, E>(_camera: &AsyncCamera<P, T, E>)
    where
        P: Profile + NdFilter,
        T: AsyncTransport + Send + Sync + 'static,
        E: grafton_visca::Executor,
    {
    }

    #[cfg(not(feature = "mode-async"))]
    fn _use_nd_filter_blocking<P, T>(_camera: &BlockingCamera<P, T>)
    where
        P: Profile + NdFilter,
        T: SyncTransport + Send + Sync + 'static,
    {
    }

    #[cfg(feature = "mode-async")]
    fn _use_basic_features_async<P, T, E>(_camera: &AsyncCamera<P, T, E>)
    where
        P: Profile,
        T: AsyncTransport + Send + Sync + 'static,
        E: grafton_visca::Executor,
    {
    }

    #[cfg(not(feature = "mode-async"))]
    fn _use_basic_features_blocking<P, T>(_camera: &BlockingCamera<P, T>)
    where
        P: Profile,
        T: SyncTransport + Send + Sync + 'static,
    {
    }
}

#[test]
fn test_mode_specific_apis() {
    #[cfg(feature = "mode-async")]
    {
        fn _async_only<P, T, E>(_camera: &AsyncCamera<P, T, E>)
        where
            P: Profile,
            T: AsyncTransport,
            E: grafton_visca::Executor,
        {
        }
    }

    #[cfg(not(feature = "mode-async"))]
    {
        fn _blocking_only<P, T>(_camera: &BlockingCamera<P, T>)
        where
            P: Profile,
            T: SyncTransport,
        {
        }
    }
}
