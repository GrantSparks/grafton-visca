//! Integration tests for the dyn-api feature.
//!
//! These tests verify that the dynamic trait object API works correctly,
//! including object safety, blanket implementations, and timeout handling.
//!
//! Note: Tests using explicit timeout parameters (`Some(Duration)`) are more complex
//! because they require simulating the movement idle detection mechanism. The basic
//! tests here focus on the `timeout: None` path which uses standard command completion.

#![cfg(all(feature = "dyn-api", feature = "runtime-tokio", feature = "test-utils"))]

mod common;

use std::sync::Arc;

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraBuilder},
    command::preset::PresetNumber,
    dynapi::{
        DynCameraControl, DynFocusControl, DynPanTiltControl, DynPresetsControl, DynZoomControl,
        IntoDynCamera,
    },
    runtime::TokioRuntime,
    testing::testkit::{helpers, ScriptedTransport},
    types::SpeedLevel,
    TokioExecutor,
};

use crate::common::patterns;

/// Verify that all dyn traits are object-safe by creating trait objects.
#[test]
fn test_dyn_traits_are_object_safe() {
    fn _assert_camera_control(_: &dyn DynCameraControl) {}
    fn _assert_pan_tilt_control(_: &dyn DynPanTiltControl) {}
    fn _assert_zoom_control(_: &dyn DynZoomControl) {}
    fn _assert_focus_control(_: &dyn DynFocusControl) {}
    fn _assert_presets_control(_: &dyn DynPresetsControl) {}

    // This test passes if it compiles - the functions above prove object safety.
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

    // Test capability accessors
    let camera_control: &dyn DynCameraControl = &dyn_camera;

    // PtzOpticsG2 has all capabilities
    assert!(
        camera_control.as_pan_tilt().is_some(),
        "Should have pan/tilt control"
    );
    assert!(
        camera_control.as_zoom().is_some(),
        "Should have zoom control"
    );
    assert!(
        camera_control.as_focus().is_some(),
        "Should have focus control"
    );
    assert!(
        camera_control.as_presets().is_some(),
        "Should have preset control"
    );
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
    let pt = dyn_camera
        .as_pan_tilt()
        .expect("Should have pan/tilt control");

    // Call pan_tilt_home without timeout (uses default timeout)
    let result = pt.pan_tilt_home(None).await;
    assert!(
        result.is_ok(),
        "pan_tilt_home should succeed: {:?}",
        result.err()
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
    let pt = dyn_camera
        .as_pan_tilt()
        .expect("Should have pan/tilt control");

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
    let pt = dyn_camera
        .as_pan_tilt()
        .expect("Should have pan/tilt control");

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
    let zoom = dyn_camera.as_zoom().expect("Should have zoom control");

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
    let focus = dyn_camera.as_focus().expect("Should have focus control");

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
    let presets = dyn_camera.as_presets().expect("Should have preset control");

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
    let presets = dyn_camera.as_presets().expect("Should have preset control");

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
        if let Some(pt) = camera.as_pan_tilt() {
            pt.pan_tilt_stop().await?;
        }
        if let Some(zoom) = camera.as_zoom() {
            zoom.zoom_stop().await?;
        }
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
    assert!(boxed.as_pan_tilt().is_some());
    assert!(boxed.as_zoom().is_some());
    assert!(boxed.as_focus().is_some());
    assert!(boxed.as_presets().is_some());
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
    if let Some(pt) = arc_clone.as_pan_tilt() {
        let result = pt.pan_tilt_stop().await;
        assert!(result.is_ok(), "Should succeed through Arc");
    }
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
    let pt = dyn_camera
        .as_pan_tilt()
        .expect("Should have pan/tilt control");

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
    let pt = dyn_camera
        .as_pan_tilt()
        .expect("Should have pan/tilt control");

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
