//! Canonical facade topology contracts.
//!
//! `blocking` and `async` are independently selectable. Runtime features
//! imply `async`, while the default feature set supplies `blocking`; therefore
//! one build can expose both owner facades without duplicate root names or
//! runtime parameters leaking into operation handles.
//!
//! The behavioral probe runs once per enabled runtime, as the cases
//! `runtime_coexistence::blocking_and_async_owners_open_use_and_close::tokio`
//! and `::smol`.

#[cfg(all(
    feature = "blocking",
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use grafton_visca_test_support::fake_camera;

#[cfg(all(
    feature = "blocking",
    any(
        not(feature = "async"),
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
fn assert_blocking_surface() {
    use grafton_visca::{
        blocking::{Camera, Operation, Session, SessionConfig},
        completion::{AppliedOnly, Targeted},
        profiles::PtzOpticsG2,
        Error, ProfileSpec,
    };

    let _: Option<Camera<PtzOpticsG2>> = None;
    let _: Option<Session> = None;
    let _: Option<SessionConfig> = None;
    let _: Option<Operation<Targeted>> = None;
    let _: Option<Operation<AppliedOnly>> = None;
    let _: fn(&mut Operation<Targeted>) -> Result<(), Error> = Operation::applied;
    let _: fn(&mut Operation<AppliedOnly>) -> Result<(), Error> = Operation::applied;
    let _: fn(ProfileSpec) -> SessionConfig = SessionConfig::new;
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
fn assert_async_surface() {
    use std::future::Future;

    use grafton_visca::{
        completion::Targeted, profiles::PtzOpticsG2, Camera, Operation, Session, SessionConfig,
    };

    let _: Option<Camera<PtzOpticsG2>> = None;
    let _: Option<Session> = None;
    let _: Option<SessionConfig> = None;
    let _: Option<Operation<Targeted>> = None;
    fn returns_future<K>(
        operation: &mut Operation<K>,
    ) -> impl Future<Output = grafton_visca::Result<()>> + '_
    where
        K: grafton_visca::completion::Kind,
    {
        operation.applied()
    }
    let _ = returns_future::<Targeted>;
}

#[cfg(all(feature = "blocking", not(feature = "async")))]
#[test]
fn blocking_only_surface_compiles() {
    assert_blocking_surface();
}

#[cfg(all(
    feature = "async",
    feature = "runtime-tokio",
    not(feature = "blocking")
))]
#[test]
fn async_only_tokio_surface_compiles() {
    assert_async_surface();
    let _: fn() -> grafton_visca::Result<grafton_visca::TokioRuntime> =
        || grafton_visca::TokioRuntime::from_current();
}

#[cfg(all(feature = "async", feature = "runtime-smol", not(feature = "blocking")))]
#[test]
fn async_only_smol_surface_compiles() {
    assert_async_surface();
    let _: fn() -> grafton_visca::SmolRuntime = grafton_visca::SmolRuntime::new;
}

#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-tokio"))]
#[test]
fn blocking_and_tokio_surfaces_coexist() {
    assert_blocking_surface();
    assert_async_surface();
}

#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-smol"))]
#[test]
fn blocking_and_smol_surfaces_coexist() {
    assert_blocking_surface();
    assert_async_surface();
}

#[cfg(all(
    feature = "blocking",
    feature = "async",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
#[test]
fn blocking_and_all_supported_executors_coexist() {
    assert_blocking_surface();
    assert_async_surface();
    let _: fn() -> grafton_visca::Result<grafton_visca::TokioRuntime> =
        || grafton_visca::TokioRuntime::from_current();
    let _: fn() -> grafton_visca::SmolRuntime = grafton_visca::SmolRuntime::new;
}

// Keep one behavioral coexistence probe in this crate. The type assertions
// above catch naming/feature regressions, while this probe proves that both
// owners can actually admit, use, and shut down in one process, under each
// enabled runtime.
#[cfg(all(
    feature = "blocking",
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod runtime_coexistence {
    use grafton_visca::{
        blocking::{Session as BlockingSession, SessionConfig as BlockingConfig},
        profiles::PtzOpticsG2,
        Executor, Session as AsyncSession, SessionConfig as AsyncConfig,
    };
    use grafton_visca_test_support::runtime_matrix;

    use crate::fake_camera::FakeCamera;

    async fn blocking_and_async_owners_open_use_and_close<E: Executor>(executor: E) {
        let blocking = BlockingSession::open(
            FakeCamera::acking(1).blocking_wire(),
            BlockingConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
        )
        .unwrap();
        blocking
            .camera::<PtzOpticsG2>()
            .unwrap()
            .zoom()
            .stop()
            .unwrap()
            .applied()
            .unwrap();

        let async_session = AsyncSession::open(
            FakeCamera::acking(1).async_wire(),
            AsyncConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
            executor,
        )
        .await
        .unwrap();
        async_session
            .camera::<PtzOpticsG2>()
            .unwrap()
            .zoom()
            .stop()
            .await
            .unwrap()
            .applied()
            .await
            .unwrap();

        blocking.close().unwrap();
        async_session.close().await.unwrap();
    }

    runtime_matrix!(blocking_and_async_owners_open_use_and_close);
}
