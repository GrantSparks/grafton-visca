//! smol integration coverage for the dyn-api feature.

#![cfg(all(feature = "dyn-api", feature = "runtime-smol", feature = "test-utils"))]

mod common;

use std::{sync::Arc, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraBuilder},
    dynapi::{DynCameraControl, IntoDynCamera, OperationCategory},
    testing::testkit::{helpers, ScriptedTransport},
    SmolExecutor,
};

use crate::common::patterns;

#[test]
fn test_dyn_api_smol_pan_tilt_home() {
    smol::block_on(async {
        let executor = Arc::new(SmolExecutor::new());
        let transport: ScriptedTransport<SmolExecutor> =
            ScriptedTransport::new(vec![helpers::command_response(
                patterns::pan_tilt::HOME.to_vec(),
                1,
            )])
            .with_executor(executor.clone());

        let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        let dyn_camera = camera.into_dyn();
        let control: &dyn DynCameraControl = &dyn_camera;

        assert_eq!(control.capabilities().model_name, "PtzOptics G2");
        control
            .pan_tilt()
            .pan_tilt_home(None)
            .await
            .expect("smol dyn pan_tilt_home should succeed");
    });
}

#[test]
fn test_dyn_api_smol_operation_handle_settled() {
    smol::block_on(async {
        let executor = Arc::new(SmolExecutor::new());
        let transport: ScriptedTransport<SmolExecutor> =
            ScriptedTransport::new(vec![helpers::command_response(
                patterns::pan_tilt::HOME.to_vec(),
                1,
            )])
            .with_executor(executor.clone());

        let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");
        let dyn_camera = camera.into_dyn();
        let handle = dyn_camera
            .pan_tilt()
            .pan_tilt_home_op()
            .await
            .expect("smol dyn handle");

        assert_eq!(handle.category(), OperationCategory::PanTilt);
        handle
            .await_settled(Duration::from_secs(1))
            .await
            .expect("smol dyn settle");
    });
}
