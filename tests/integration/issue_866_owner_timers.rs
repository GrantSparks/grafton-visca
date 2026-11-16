//! The actual local executor drives owners without selecting a native runtime.

#![cfg(all(feature = "async", feature = "test-utils"))]

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use grafton_visca::{
    config::SessionConfig, profiles::PtzOpticsG2, r#async::Session, runtime::Executor,
};
use grafton_visca_test_support::fake_camera::FakeCamera;

use crate::common::counting_executor;
use counting_executor::CountingExecutor;

#[test]
fn local_manual_clock_actor_reuses_four_timer_factories() {
    use grafton_visca::testing::testkit::{ManualClock, ManualClockExecutor};

    let clock = ManualClock::new();
    let timer_calls = Arc::new(AtomicUsize::new(0));
    let spawned = Arc::new(AtomicUsize::new(0));
    let executor = CountingExecutor {
        inner: ManualClockExecutor::local(&clock),
        spawned: Arc::clone(&spawned),
        timer_calls: Arc::clone(&timer_calls),
    };
    executor.block_on(async {
        let fake = FakeCamera::acking(1);
        let session = Session::open(
            fake.async_wire(),
            &SessionConfig::from_compile_time::<PtzOpticsG2>().expect("profile"),
            executor.clone(),
        )
        .await
        .expect("session on the runtime-free executor");
        let camera = session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera");
        assert_eq!(timer_calls.load(Ordering::SeqCst), 4);
        assert_eq!(spawned.load(Ordering::SeqCst), 1);
        for _ in 0..40 {
            camera
                .zoom()
                .stop()
                .await
                .expect("admission")
                .applied()
                .await
                .expect("application");
            session.metrics().await.expect("owner control turn");
            // Let bounded reads and receive pacing expire between commands.
            // This is the actual ManualClock backend, without a Tokio runtime.
            executor.sleep(Duration::from_secs(1)).await;
            assert_eq!(timer_calls.load(Ordering::SeqCst), 4);
        }
        assert_eq!(fake.write_count(), 40);
        assert_eq!(session.metrics().await.expect("metrics").writes, 40);
        session.close().await.expect("close");
    });
    assert_eq!(timer_calls.load(Ordering::SeqCst), 4);
}
