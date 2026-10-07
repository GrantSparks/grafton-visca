//! Test-case generators that run one scenario under every facade or runtime.
//!
//! Import the generators by name, e.g.
//! `use grafton_visca_test_support::facade_matrix;`.
//!
//! Each scenario becomes its own libtest case per runtime or facade, so one
//! failing scenario cannot hide the others: awaiting several scenarios in
//! sequence inside one test stops at the first panic, and a green run would
//! then prove only that nothing failed before it.

/// Runs each `async fn scenario<E: Executor>(executor: E)` once per enabled
/// runtime, as the cases `scenario::tokio` and `scenario::smol`.
///
/// The Tokio cases use the current-thread test runtime. Prefix the list with
/// `multi_thread:` to run them on a two-worker multi-thread runtime instead,
/// for scenarios that must also hold under Tokio's work-stealing scheduler.
#[macro_export]
macro_rules! runtime_matrix {
    (multi_thread: $($scenario:ident),+ $(,)?) => {
        $(
            $crate::runtime_matrix!(
                @case [tokio::test(flavor = "multi_thread", worker_threads = 2)] $scenario
            );
        )+
    };
    ($($scenario:ident),+ $(,)?) => {
        $($crate::runtime_matrix!(@case [tokio::test] $scenario);)+
    };
    (@case [$tokio_test:meta] $scenario:ident) => {
        mod $scenario {
            #[cfg(feature = "runtime-tokio")]
            #[$tokio_test]
            async fn tokio() {
                let executor =
                    grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
                super::$scenario(executor).await;
            }

            #[cfg(feature = "runtime-smol")]
            #[test]
            fn smol() {
                smol::block_on(super::$scenario(grafton_visca::SmolRuntime::new()));
            }
        }
    };
}

/// Writes a scenario once and runs it on the blocking facade and on the async
/// facade under each enabled runtime, as the cases `scenario::blocking`,
/// `scenario::tokio` and `scenario::smol`.
///
/// ```ignore
/// facade_matrix! {
///     fn applied_stop_completes() {
///         let camera = FakeCamera::acking(1);
///         let session = open!(camera, config()).expect("session");
///         let view = session.camera::<P>().expect("view");
///         wait!(view.zoom().stop()).expect("applied");
///     }
/// }
/// ```
///
/// The body is the same tokens on every facade; these macros spell the
/// difference:
///
/// * `wait!(expr)` — `expr.await` on the async facade, `expr` on the blocking
///   one;
/// * `open!(camera, config)` — opens the facade's `Session` over that
///   facade's wire onto the `FakeCamera` `camera` (async sessions run on the
///   case's runtime);
/// * `open_camera!(camera, &config)` — the same for the facade's single-camera
///   `CameraSession`, from a `&CameraConfig<P>`;
/// * `wait_for_writes!(camera, n)` / `wait_for_reads!(camera, n)` — the
///   facade's bounded wait helper;
/// * `pause!(duration)` — sleeps the test thread on the blocking facade and
///   waits on the runtime's timer on the async one;
/// * `within!(duration, expr)` — on the async facade, awaits `expr` under an
///   outer runtime deadline and panics if it elapses, so a hung step fails the
///   case; on the blocking facade it evaluates `expr`, which the call's own
///   deadlines bound (an outer bound would need a second thread);
/// * `now!()` — the instant the session's owner measures its deadlines
///   against: `Instant::now()` on the blocking facade, `Executor::now` on the
///   async one (virtual under `paused:`);
/// * `after!(duration, f)` — runs the `FnOnce() + Send + 'static` `f` once
///   `duration` has passed, on a helper thread on the blocking facade and on a
///   detached runtime task (so on the runtime's own clock) on the async one;
/// * `virtual_clock!()` — `true` only in a `paused:` async case, where elapsed
///   owner time is exact and may be asserted as such. A `paused:` blocking
///   case also runs on virtual time, but its worker notices a boundary only
///   between bounded reads, so its elapsed time is not exact;
/// * `FACADE` — `"blocking"` or `"async"`, for assertion messages.
///
/// Prefix the scenarios with `paused:` to run every case on virtual time: the
/// Tokio cases under `#[tokio::test(start_paused = true)]`, and, with
/// `test-utils`, the blocking and smol cases on a `ManualClock` (the blocking
/// session and its fake-camera wire wait on the clock; the smol session runs
/// on a `ManualClockExecutor` over smol). Every deadline, backoff and
/// `pause!` is then virtual and lapses as soon as everything is blocked on
/// the clock. Anything such a scenario waits for must be an owner deadline,
/// a matrix macro or an event, never another thread's real sleep. Without
/// `test-utils` the blocking and smol cases run in real time.
///
/// A scenario that only exists on one facade is not written here: it stays a
/// plain test, and the file header says why.
#[macro_export]
macro_rules! facade_matrix {
    (paused: $($scenarios:tt)+) => {
        $crate::facade_matrix!(@cases [tokio::test(start_paused = true)] true; $($scenarios)+);
    };
    ($(
        $(#[$meta:meta])*
        fn $scenario:ident() $body:block
    )+) => {
        $crate::facade_matrix!(@cases [tokio::test] false; $($(#[$meta])* fn $scenario() $body)+);
    };
    (@cases [$tokio_test:meta] $virtual:tt; $(
        $(#[$meta:meta])*
        fn $scenario:ident() $body:block
    )+) => {
        $(
            $(#[$meta])*
            mod $scenario {
                use super::*;

                #[cfg(feature = "blocking")]
                #[test]
                fn blocking() {
                    $crate::facade_matrix!(@blocking $virtual, $body);
                }

                #[cfg(all(feature = "async", feature = "runtime-tokio"))]
                #[$tokio_test]
                async fn tokio() {
                    let executor =
                        grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
                    $crate::facade_matrix!(@async executor, $virtual, $body);
                }

                #[cfg(all(feature = "async", feature = "runtime-smol"))]
                #[test]
                fn smol() {
                    $crate::facade_matrix!(@smol $virtual, $body);
                }
            }
        )+
    };
    (@blocking true, $body:block) => {{
        #[cfg(feature = "test-utils")]
        {
            let clock = grafton_visca::testing::testkit::ManualClock::new();
            let _driver = clock.enter();
            macro_rules! wait {
                ($e:expr) => {
                    $e
                };
            }
            macro_rules! open {
                ($camera:expr, $config:expr) => {
                    clock.open_blocking_session($camera.blocking_wire().on_clock(&clock), $config)
                };
            }
            macro_rules! open_camera {
                ($camera:expr, $config:expr) => {
                    clock.open_blocking_camera_session(
                        $camera.blocking_wire().on_clock(&clock),
                        $config,
                    )
                };
            }
            macro_rules! wait_for_writes {
                ($camera:expr, $count:expr) => {
                    $camera.wait_for_writes_on(&clock, $count)
                };
            }
            macro_rules! wait_for_reads {
                ($camera:expr, $count:expr) => {
                    $camera.wait_for_reads_on(&clock, $count)
                };
            }
            macro_rules! pause {
                ($duration:expr) => {
                    clock.sleep($duration)
                };
            }
            macro_rules! within {
                ($duration:expr, $e:expr) => {{
                    let _: std::time::Duration = $duration;
                    $e
                }};
            }
            macro_rules! now {
                () => {
                    clock.now()
                };
            }
            macro_rules! after {
                ($duration:expr, $f:expr) => {{
                    let duration: std::time::Duration = $duration;
                    let f = $f;
                    let timer = clock.clone();
                    clock.spawn(move || {
                        timer.sleep(duration);
                        f();
                    });
                }};
            }
            // Deterministic in outcome but not exact: the worker notices a
            // boundary only between bounded reads, so each boundary can add
            // up to one read slice of virtual time, as it does in real time.
            macro_rules! virtual_clock {
                () => {
                    false
                };
            }
            const FACADE: &str = "blocking";
            $body
        }
        #[cfg(not(feature = "test-utils"))]
        $crate::facade_matrix!(@blocking false, $body);
    }};
    (@blocking false, $body:block) => {{
        macro_rules! wait {
            ($e:expr) => {
                $e
            };
        }
        macro_rules! open {
            ($camera:expr, $config:expr) => {
                grafton_visca::blocking::Session::open($camera.blocking_wire(), $config)
            };
        }
        macro_rules! open_camera {
            ($camera:expr, $config:expr) => {
                grafton_visca::blocking::CameraSession::open($camera.blocking_wire(), $config)
            };
        }
        macro_rules! wait_for_writes {
            ($camera:expr, $count:expr) => {
                $camera.wait_for_writes($count)
            };
        }
        macro_rules! wait_for_reads {
            ($camera:expr, $count:expr) => {
                $camera.wait_for_reads($count)
            };
        }
        macro_rules! pause {
            ($duration:expr) => {
                std::thread::sleep($duration)
            };
        }
        macro_rules! within {
            ($duration:expr, $e:expr) => {{
                let _: std::time::Duration = $duration;
                $e
            }};
        }
        macro_rules! now {
            () => {
                std::time::Instant::now()
            };
        }
        macro_rules! after {
            ($duration:expr, $f:expr) => {{
                let duration: std::time::Duration = $duration;
                let f = $f;
                std::thread::spawn(move || {
                    std::thread::sleep(duration);
                    f();
                });
            }};
        }
        macro_rules! virtual_clock {
            () => {
                false
            };
        }
        const FACADE: &str = "blocking";
        $body
    }};
    (@smol true, $body:block) => {{
        #[cfg(feature = "test-utils")]
        {
            let clock = grafton_visca::testing::testkit::ManualClock::new();
            let executor = grafton_visca::testing::testkit::ManualClockExecutor::new(
                grafton_visca::SmolRuntime::new(),
                &clock,
            );
            grafton_visca::Executor::block_on(&executor, async {
                $crate::facade_matrix!(@async executor, true, $body);
            });
        }
        #[cfg(not(feature = "test-utils"))]
        $crate::facade_matrix!(@smol false, $body);
    }};
    (@smol false, $body:block) => {
        smol::block_on(async {
            let executor = grafton_visca::SmolRuntime::new();
            $crate::facade_matrix!(@async executor, false, $body);
        })
    };
    (@async $executor:ident, $virtual:tt, $body:block) => {{
        macro_rules! wait {
            ($e:expr) => {
                $e.await
            };
        }
        macro_rules! open {
            ($camera:expr, $config:expr) => {
                grafton_visca::Session::open($camera.async_wire(), $config, $executor.clone())
                    .await
            };
        }
        macro_rules! open_camera {
            ($camera:expr, $config:expr) => {
                grafton_visca::CameraSession::open($camera.async_wire(), $config, $executor.clone())
                    .await
            };
        }
        macro_rules! wait_for_writes {
            ($camera:expr, $count:expr) => {
                $camera.wait_for_writes_async(&$executor, $count).await
            };
        }
        macro_rules! wait_for_reads {
            ($camera:expr, $count:expr) => {
                $camera.wait_for_reads_async(&$executor, $count).await
            };
        }
        macro_rules! pause {
            ($duration:expr) => {
                grafton_visca::Executor::sleep(&$executor, $duration).await
            };
        }
        macro_rules! within {
            ($duration:expr, $e:expr) => {
                grafton_visca::Executor::timeout(&$executor, $duration, $e)
                    .await
                    .expect(concat!("step exceeded its outer deadline: ", stringify!($e)))
            };
        }
        macro_rules! now {
            () => {
                grafton_visca::Executor::now(&$executor)
            };
        }
        macro_rules! after {
            ($duration:expr, $f:expr) => {{
                let duration: std::time::Duration = $duration;
                let f = $f;
                let timer = $executor.clone();
                grafton_visca::Executor::spawn_bg(&$executor, async move {
                    grafton_visca::Executor::sleep(&timer, duration).await;
                    f();
                });
            }};
        }
        macro_rules! virtual_clock {
            () => {
                $virtual
            };
        }
        const FACADE: &str = "async";
        let _ = &$executor;
        $body
    }};
}
