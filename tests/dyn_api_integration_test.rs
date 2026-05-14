//! Integration tests for the dyn-api feature.
//!
//! These tests verify that the dynamic trait object API works correctly,
//! including object safety, blanket implementations, and timeout handling.
//!
//! The tests cover both the default timeout path and explicit per-call
//! command-completion deadlines.

#![cfg(all(feature = "dyn-api", feature = "runtime-tokio", feature = "test-utils"))]

mod common;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::Arc,
    sync::Mutex,
    time::{Duration, Instant},
};

use grafton_visca::{
    camera::{
        controls::{
            focus::FocusControl, pan_tilt::PanTiltControl, presets::PresetsControl,
            zoom::ZoomControl,
        },
        profiles::PtzOpticsG2,
        Camera, CameraBuilder,
    },
    dynapi::{
        DynCameraControl, DynFocusControl, DynMotionControl, DynPanTiltControl, DynPresetsControl,
        DynZoomControl, IntoDynCamera,
    },
    mode::Async,
    runtime::TokioRuntime,
    testing::testkit::{helpers, scripted_transport::Step, ScriptedTransport},
    timeout::TimeoutConfig,
    types::{FocusPosition, SpeedLevel},
    Error, Normalized, PresetNumber, TokioExecutor, ZoomDomain,
};

use crate::common::patterns;

struct CountingAllocator;

static ALLOCATION_TEST_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static COUNTING_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| {
                count.set(count.get().saturating_add(1));
            });
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

struct AllocationCountingGuard;

impl Drop for AllocationCountingGuard {
    fn drop(&mut self) {
        let _ = COUNTING_ALLOCATIONS.try_with(|counting| counting.set(false));
    }
}

type TestCamera = Camera<Async, PtzOpticsG2, ScriptedTransport<TokioExecutor>, TokioRuntime>;

fn allocations_during(f: impl FnOnce()) -> usize {
    let _guard = ALLOCATION_TEST_LOCK
        .lock()
        .expect("allocation lock poisoned");
    ALLOCATION_COUNT.with(|count| count.set(0));
    COUNTING_ALLOCATIONS.with(|counting| counting.set(true));

    {
        let _counting_guard = AllocationCountingGuard;
        f();
    }

    ALLOCATION_COUNT.with(Cell::get)
}

fn future_construction_allocations<Fut>(make_future: impl FnOnce() -> Fut) -> usize {
    allocations_during(|| {
        let future = make_future();
        std::hint::black_box(&future);
    })
}

fn assert_dyn_future_allocations_match_static<S, D, SFut, DFut>(
    label: &str,
    static_call: S,
    dyn_call: D,
) where
    S: FnOnce() -> SFut,
    D: FnOnce() -> DFut,
{
    let static_allocations = future_construction_allocations(static_call);
    let dyn_allocations = future_construction_allocations(dyn_call);

    assert!(
        dyn_allocations <= static_allocations,
        "{label} dyn future construction allocated more than static: dyn={dyn_allocations}, static={static_allocations}"
    );
}

async fn new_test_camera(steps: impl Into<Vec<Step>>) -> TestCamera {
    new_test_camera_with_timeout(steps, TimeoutConfig::default()).await
}

async fn new_test_camera_with_timeout(
    steps: impl Into<Vec<Step>>,
    timeout_config: TimeoutConfig,
) -> TestCamera {
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(steps);
    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    CameraBuilder::with_executor(runtime)
        .timeout_config(timeout_config)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera")
}

/// Verify that all dyn traits are object-safe by creating trait objects.
#[test]
fn test_dyn_traits_are_object_safe() {
    fn _assert_camera_control(_: &dyn DynCameraControl) {}
    fn _assert_pan_tilt_control(_: &dyn DynPanTiltControl) {}
    fn _assert_zoom_control(_: &dyn DynZoomControl) {}
    fn _assert_focus_control(_: &dyn DynFocusControl) {}
    fn _assert_presets_control(_: &dyn DynPresetsControl) {}
    fn _assert_motion_control(_: &dyn DynMotionControl) {}

    // This test passes if it compiles - the functions above prove object safety.
}

/// Direct dyn forwarding should return the static boxed future without adding
/// an outer boxed adapter at construction time.
#[tokio::test]
async fn test_dyn_direct_forwarding_future_construction_allocations_match_static() {
    let static_camera = new_test_camera(vec![]).await;
    let dyn_camera = new_test_camera(vec![]).await.into_dyn();

    assert_dyn_future_allocations_match_static(
        "pan_tilt_stop",
        || static_camera.pan_tilt_stop(),
        || dyn_camera.pan_tilt().pan_tilt_stop(),
    );

    assert_dyn_future_allocations_match_static(
        "zoom_stop",
        || static_camera.zoom_stop(),
        || dyn_camera.zoom().zoom_stop(),
    );

    assert_dyn_future_allocations_match_static(
        "focus_auto",
        || static_camera.focus_auto(),
        || dyn_camera.focus().focus_auto(),
    );

    let preset = PresetNumber::new(1).expect("valid preset");
    assert_dyn_future_allocations_match_static(
        "preset_set",
        || static_camera.preset_set(preset),
        || dyn_camera.presets().preset_set(preset),
    );

    assert_dyn_future_allocations_match_static(
        "pan_tilt_home(None)",
        || static_camera.pan_tilt_home(),
        || dyn_camera.pan_tilt().pan_tilt_home(None),
    );
}

/// Test that DynCamera can be created from a concrete camera and used as a trait object.
#[tokio::test]
async fn test_dyn_camera_creation_and_capability_accessors() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::auto_respond_step()]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    // Convert to DynCamera
    let dyn_camera = camera.into_dyn();

    // Test capability metadata and core control accessors
    let camera_control: &dyn DynCameraControl = &dyn_camera;
    let capabilities = camera_control.capabilities();

    // PtzOpticsG2 has all capabilities
    assert!(capabilities.has_pan_tilt, "Should have pan/tilt control");
    assert!(capabilities.has_zoom, "Should have zoom control");
    assert!(capabilities.has_focus, "Should have focus control");
    assert!(capabilities.has_presets, "Should have preset control");
    assert_eq!(capabilities.model_name, "PtzOptics G2");

    let _ = camera_control.pan_tilt();
    let _ = camera_control.zoom();
    let _ = camera_control.focus();
    let _ = camera_control.presets();
    let _ = camera_control.motion();
}

/// Test pan/tilt operations through the dyn-api without timeout.
#[tokio::test]
async fn test_dyn_pan_tilt_home_no_timeout() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::command_response(
            patterns::pan_tilt::HOME.to_vec(),
            1,
        )]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let pt = dyn_camera.pan_tilt();

    // Call pan_tilt_home without timeout (uses default timeout)
    let result = pt.pan_tilt_home(None).await;
    assert!(
        result.is_ok(),
        "pan_tilt_home should succeed: {:?}",
        result.err()
    );
}

/// Explicit dyn method timeouts wait on the command response future and
/// propagate camera errors instead of treating any completed response as success.
#[tokio::test]
async fn test_dyn_method_explicit_timeout_path_propagates_command_errors() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::errors::syntax_error(1)]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let result = dyn_camera
        .pan_tilt()
        .pan_tilt_home(Some(Duration::from_secs(1)))
        .await;

    assert!(
        matches!(result, Err(Error::SyntaxError)),
        "camera syntax error should propagate through dyn method timeout path: {result:?}"
    );
}

/// Explicit timeout paths for zoom delegate through the static operation helper,
/// preserving command error propagation for paths that previously built commands
/// directly in dynapi.rs.
#[tokio::test]
async fn test_dyn_zoom_explicit_timeout_path_propagates_command_errors() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::errors::syntax_error(1)]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let result = dyn_camera
        .zoom()
        .zoom_tele(None, Some(Duration::from_secs(1)))
        .await;

    assert!(
        matches!(result, Err(Error::SyntaxError)),
        "camera syntax error should propagate through dyn zoom timeout path: {result:?}"
    );
}

/// Dyn no-timeout calls should use the runtime's default command timeout path,
/// not an explicit per-call deadline.
#[tokio::test]
async fn test_dyn_no_timeout_path_uses_default_command_timeout() {
    let camera = new_test_camera_with_timeout(
        vec![Step::OnSend {
            matches: Some(patterns::pan_tilt::HOME.to_vec()),
            responses: vec![helpers::ack(1)],
        }],
        TimeoutConfig::uniform(Duration::from_millis(50)),
    )
    .await;

    let dyn_camera = camera.into_dyn();
    let started = Instant::now();
    let result = dyn_camera.pan_tilt().pan_tilt_home(None).await;

    assert!(
        matches!(result, Err(Error::Timeout)),
        "missing completion should use default command timeout: {result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "default command timeout should come from TimeoutConfig"
    );
}

/// Static and dyn paths should reject the same validation edge cases before
/// command submission.
#[tokio::test]
async fn test_dyn_validation_parity_for_pan_tilt_focus_preset_and_zoom_domain() {
    let static_camera = new_test_camera(vec![]).await;
    let dyn_camera = new_test_camera(vec![]).await.into_dyn();

    let static_pan_tilt = static_camera
        .pan_tilt_absolute(171.0, 0.0, SpeedLevel::Medium)
        .await;
    let dyn_pan_tilt = dyn_camera
        .pan_tilt()
        .pan_tilt_absolute(171.0, 0.0, SpeedLevel::Medium, None)
        .await;
    assert!(
        matches!(
            static_pan_tilt,
            Err(Error::InvalidParameter {
                parameter: "degrees",
                ..
            })
        ),
        "static pan/tilt should reject out-of-range degrees: {static_pan_tilt:?}"
    );
    assert!(
        matches!(
            dyn_pan_tilt,
            Err(Error::InvalidParameter {
                parameter: "degrees",
                ..
            })
        ),
        "dyn pan/tilt should reject out-of-range degrees: {dyn_pan_tilt:?}"
    );

    let invalid_focus = FocusPosition::new(0x0FFF);
    let static_focus = static_camera.set_focus(invalid_focus).await;
    let dyn_focus = dyn_camera.focus().set_focus(invalid_focus, None).await;
    assert!(
        matches!(
            static_focus,
            Err(Error::ValidationError(
                grafton_visca::capabilities::ValidationError::OutOfRange {
                    parameter: "focus position",
                    ..
                }
            ))
        ),
        "static focus should reject out-of-range focus position: {static_focus:?}"
    );
    assert!(
        matches!(
            dyn_focus,
            Err(Error::ValidationError(
                grafton_visca::capabilities::ValidationError::OutOfRange {
                    parameter: "focus position",
                    ..
                }
            ))
        ),
        "dyn focus should reject out-of-range focus position: {dyn_focus:?}"
    );

    let invalid_profile_preset = PresetNumber::new(128).expect("raw preset should be valid");
    let static_preset = static_camera.preset_recall(invalid_profile_preset).await;
    let dyn_preset = dyn_camera
        .presets()
        .preset_recall(invalid_profile_preset, None)
        .await;
    assert!(
        matches!(
            static_preset,
            Err(Error::ParameterOutOfRange {
                parameter: "preset_number",
                value: 128,
                min: 0,
                max: 127,
            })
        ),
        "static preset should reject profile-invalid preset: {static_preset:?}"
    );
    assert!(
        matches!(
            dyn_preset,
            Err(Error::ParameterOutOfRange {
                parameter: "preset_number",
                value: 128,
                min: 0,
                max: 127,
            })
        ),
        "dyn preset should reject profile-invalid preset: {dyn_preset:?}"
    );

    let normalized = Normalized::new(0.5).expect("valid normalized value");
    let static_zoom = static_camera
        .zoom_absolute_normalized(normalized, ZoomDomain::OpticalPlusDigital)
        .await;
    let dyn_zoom = dyn_camera
        .zoom()
        .zoom_absolute_normalized(normalized, ZoomDomain::OpticalPlusDigital, None)
        .await;
    assert!(
        matches!(
            static_zoom,
            Err(Error::FeatureNotSupported {
                feature: "Digital zoom"
            })
        ),
        "static zoom should reject unsupported digital zoom domain: {static_zoom:?}"
    );
    assert!(
        matches!(
            dyn_zoom,
            Err(Error::FeatureNotSupported {
                feature: "Digital zoom"
            })
        ),
        "dyn zoom should reject unsupported digital zoom domain: {dyn_zoom:?}"
    );
}

/// Dyn operation handles preserve the exact command response future from the
/// static handle, so command errors are returned by await_completion.
#[tokio::test]
async fn test_dyn_inflight_await_completion_propagates_command_errors() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::errors::syntax_error(1)]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let handle = dyn_camera
        .pan_tilt()
        .pan_tilt_home_op()
        .await
        .expect("operation handle should be created before the response arrives");

    let result = handle.await_completion(Duration::from_secs(1)).await;
    assert!(
        matches!(result, Err(Error::SyntaxError)),
        "camera syntax error should propagate through InFlightDyn: {result:?}"
    );
}

/// Awaiting a dyn operation consumes its response future, matching the static
/// InFlight contract.
#[tokio::test]
async fn test_dyn_inflight_await_completion_is_one_shot() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::command_response(
            patterns::pan_tilt::HOME.to_vec(),
            1,
        )]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let handle = dyn_camera
        .pan_tilt()
        .pan_tilt_home_op()
        .await
        .expect("operation handle should be created");

    handle
        .await_completion(Duration::from_secs(1))
        .await
        .expect("first completion wait should succeed");

    let second = handle.await_completion(Duration::from_secs(1)).await;
    assert!(
        matches!(second, Err(Error::InvalidState(ref message)) if message.contains("await_completion called more than once")),
        "second completion wait should be rejected: {second:?}"
    );
}

/// Cancelling a dyn operation should fail that operation's completion future
/// with the VISCA cancellation error.
#[tokio::test]
async fn test_dyn_inflight_cancel_propagates_command_canceled() {
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        Step::OnSend {
            matches: Some(patterns::pan_tilt::HOME.to_vec()),
            responses: vec![helpers::ack(1)],
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]),
            responses: vec![vec![0x90, 0x61, 0x04, 0xFF]],
        },
    ]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let handle = dyn_camera
        .pan_tilt()
        .pan_tilt_home_op()
        .await
        .expect("operation handle should be created");

    tokio::time::sleep(Duration::from_millis(10)).await;
    handle.cancel().await.expect("cancel should be submitted");

    let result = handle.await_completion(Duration::from_secs(1)).await;
    assert!(
        matches!(result, Err(Error::CommandCanceled)),
        "cancelled dyn operation should resolve as CommandCanceled: {result:?}"
    );
}

/// Methods without a public static `_op` variant still apply explicit dyn
/// timeouts to command completion, not to a later idle poll.
#[tokio::test]
async fn test_dyn_zoom_tele_explicit_timeout_uses_command_completion_deadline() {
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),
        responses: vec![helpers::ack(1)],
    }]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let started = Instant::now();
    let result = dyn_camera
        .zoom()
        .zoom_tele(None, Some(Duration::from_millis(50)))
        .await;

    assert!(
        matches!(result, Err(Error::Timeout)),
        "missing completion should use explicit dyn timeout: {result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "explicit dyn timeout should not wait for the default command timeout"
    );
}

/// Test pan/tilt reset command.
#[tokio::test]
async fn test_dyn_pan_tilt_reset() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::command_response(
            patterns::pan_tilt::RESET.to_vec(),
            1,
        )]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let pt = dyn_camera.pan_tilt();

    // Call pan_tilt_reset without timeout (uses default timeout)
    let result = pt.pan_tilt_reset(None).await;
    assert!(
        result.is_ok(),
        "pan_tilt_reset should succeed: {:?}",
        result.err()
    );
}

/// Test pan/tilt stop command (no timeout needed).
#[tokio::test]
async fn test_dyn_pan_tilt_stop() {
    // Use auto_respond_step because pan_tilt_stop uses SpeedLevel::Medium
    // which generates different speed bytes than the hardcoded STOP pattern
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::auto_respond_step()]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let pt = dyn_camera.pan_tilt();

    let result = pt.pan_tilt_stop().await;
    assert!(
        result.is_ok(),
        "pan_tilt_stop should succeed: {:?}",
        result.err()
    );
}

/// Test zoom operations through the dyn-api.
#[tokio::test]
async fn test_dyn_zoom_operations() {
    let zoom_in_cmd = vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]; // Zoom tele standard

    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::command_response(patterns::zoom::STOP.to_vec(), 1),
        helpers::command_response(zoom_in_cmd, 2),
    ]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let zoom = dyn_camera.zoom();

    // Test zoom stop
    let stop_result = zoom.zoom_stop().await;
    assert!(
        stop_result.is_ok(),
        "zoom_stop should succeed: {:?}",
        stop_result.err()
    );

    // Test zoom tele without timeout
    let tele_result = zoom.zoom_tele(None, None).await;
    assert!(
        tele_result.is_ok(),
        "zoom_tele should succeed: {:?}",
        tele_result.err()
    );
}

/// Test focus operations through the dyn-api.
#[tokio::test]
async fn test_dyn_focus_operations() {
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::command_response(patterns::focus::AUTO.to_vec(), 1),
        helpers::command_response(patterns::focus::MANUAL.to_vec(), 2),
    ]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let focus = dyn_camera.focus();

    // Test focus auto
    let auto_result = focus.focus_auto().await;
    assert!(
        auto_result.is_ok(),
        "focus_auto should succeed: {:?}",
        auto_result.err()
    );

    // Test focus manual
    let manual_result = focus.focus_manual().await;
    assert!(
        manual_result.is_ok(),
        "focus_manual should succeed: {:?}",
        manual_result.err()
    );
}

/// Test preset operations through the dyn-api.
#[tokio::test]
async fn test_dyn_preset_operations() {
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::command_response(patterns::preset::SET_1.to_vec(), 1),
        helpers::command_response(patterns::preset::RECALL_1.to_vec(), 2),
    ]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let presets = dyn_camera.presets();

    let preset_num = PresetNumber::new(1).expect("Valid preset number");

    // Test preset set
    let set_result = presets.preset_set(preset_num).await;
    assert!(
        set_result.is_ok(),
        "preset_set should succeed: {:?}",
        set_result.err()
    );

    // Test preset recall without timeout
    let recall_result = presets.preset_recall(preset_num, None).await;
    assert!(
        recall_result.is_ok(),
        "preset_recall should succeed: {:?}",
        recall_result.err()
    );
}

/// Test preset reset operation.
#[tokio::test]
async fn test_dyn_preset_reset() {
    // Preset reset command for preset 0
    let preset_reset_0 = vec![0x81, 0x01, 0x04, 0x3F, 0x00, 0x00, 0xFF];
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::command_response(preset_reset_0, 1)]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let presets = dyn_camera.presets();

    let preset_num = PresetNumber::new(0).expect("Valid preset number");

    let result = presets.preset_reset(preset_num).await;
    assert!(
        result.is_ok(),
        "preset_reset should succeed: {:?}",
        result.err()
    );
}

/// Test using dyn-api in a function that accepts trait objects (runtime polymorphism).
#[tokio::test]
async fn test_dyn_api_polymorphism() {
    // This function demonstrates using trait objects for runtime polymorphism
    async fn control_camera(camera: &dyn DynCameraControl) -> grafton_visca::Result<()> {
        camera.pan_tilt().pan_tilt_stop().await?;
        camera.zoom().zoom_stop().await?;
        Ok(())
    }

    // Use auto_respond_step since command bytes vary by speed level
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::auto_respond_step(),
        helpers::auto_respond_step(),
    ]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();

    // Use the camera through trait object interface
    let result = control_camera(&dyn_camera).await;
    assert!(
        result.is_ok(),
        "control_camera should succeed: {:?}",
        result.err()
    );
}

/// Test that DynCamera can be stored in a Box and used as a trait object.
#[tokio::test]
async fn test_dyn_camera_in_box() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::auto_respond_step()]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();

    // Store in a Box as trait object
    let boxed: Box<dyn DynCameraControl> = Box::new(dyn_camera);

    // Use through the boxed trait object
    assert!(boxed.capabilities().has_pan_tilt);
    assert!(boxed.capabilities().has_zoom);
    assert!(boxed.capabilities().has_focus);
    assert!(boxed.capabilities().has_presets);
}

/// Test that DynCamera can be stored in an Arc for shared access.
#[tokio::test]
async fn test_dyn_camera_in_arc() {
    // Use auto_respond_step since command bytes vary by speed level
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::auto_respond_step()]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();

    // Store in an Arc for shared access
    let arc_camera: Arc<dyn DynCameraControl + Send + Sync> = Arc::new(dyn_camera);

    // Clone the Arc for multi-threaded access
    let arc_clone = Arc::clone(&arc_camera);

    // Use through the Arc
    let result = arc_clone.pan_tilt().pan_tilt_stop().await;
    assert!(result.is_ok(), "Should succeed through Arc");
}

/// Test pan_tilt_absolute through dyn-api.
#[tokio::test]
async fn test_dyn_pan_tilt_absolute() {
    // PtzOpticsG2 pan_tilt_absolute command for 0.0, 0.0 degrees at Fast speed
    // The actual bytes depend on the profile's coordinate conversion
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::auto_respond_step()]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let pt = dyn_camera.pan_tilt();

    // Test absolute positioning without timeout
    let result = pt
        .pan_tilt_absolute(0.0, 0.0, SpeedLevel::Medium, None)
        .await;
    assert!(
        result.is_ok(),
        "pan_tilt_absolute should succeed: {:?}",
        result.err()
    );
}

/// Test pan_tilt_relative through dyn-api without timeout.
#[tokio::test]
async fn test_dyn_pan_tilt_relative() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::auto_respond_step()]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();
    let pt = dyn_camera.pan_tilt();

    // Test relative positioning without timeout (uses default timeout)
    let result = pt
        .pan_tilt_relative(5.0, -2.0, SpeedLevel::Slow, None)
        .await;
    assert!(
        result.is_ok(),
        "pan_tilt_relative should succeed: {:?}",
        result.err()
    );
}

/// Test that DynCamera Debug implementation works.
#[tokio::test]
async fn test_dyn_camera_debug() {
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();

    // Verify Debug output contains expected information
    let debug_str = format!("{:?}", dyn_camera);
    assert!(
        debug_str.contains("DynCamera"),
        "Debug should contain DynCamera"
    );
    assert!(
        debug_str.contains("PtzOpticsG2"),
        "Debug should contain profile name"
    );
}

/// Test DynCamera into_inner conversion.
#[tokio::test]
async fn test_dyn_camera_into_inner() {
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![helpers::auto_respond_step()]);

    let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    let dyn_camera = camera.into_dyn();

    // Get reference to underlying camera
    let _camera_ref = dyn_camera.camera();

    // Convert back to concrete type
    let _concrete = dyn_camera
        .into_inner()
        .expect("Should be able to unwrap with no other references");

    // Can use concrete API again (would need to re-wrap for dyn API)
}
