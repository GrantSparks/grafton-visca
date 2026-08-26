//! Issue #575: menu control and timeout configuration on the async camera surface.
//!
//! The blocking and async `impl` blocks of `Camera` are hand-duplicated, and the
//! async half was missing `menu()` and `set_timeout_config()` entirely. These
//! tests keep both on the async surface.

#![cfg(feature = "mode-async")]

use grafton_visca::{
    camera::{AsyncCamera, MenuAccessor},
    capabilities::Profile,
    mode::Async,
    timeout::TimeoutConfig,
    Executor,
};

/// Compile-time proof that the async camera exposes both accessors.
///
/// The runtime tests below exercise them end to end; this function keeps the
/// surface check alive for builds without an executor runtime or test utilities.
fn _async_surface_exposes_menu_and_timeout_config<P, Tr, Exec>(
    camera: &mut AsyncCamera<P, Tr, Exec>,
    timeout_config: TimeoutConfig,
) where
    P: Profile + Default,
    Tr: Send + 'static,
    Exec: Executor,
{
    let _menu: MenuAccessor<'_, Async, P, Tr, Exec> = camera.menu();
    let _updated = camera.set_timeout_config(timeout_config);
}

#[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
mod tokio_tests {
    use std::{sync::Arc, time::Duration};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, AsyncCamera, CameraBuilder},
        command::PanTilt,
        testing::testkit::{helpers, ScriptedTransport, Step},
        timeout::TimeoutConfig,
        TokioExecutor,
    };

    const MENU_ON: &[u8] = &[0x81, 0x01, 0x06, 0x06, 0x02, 0xFF];
    const MENU_DOWN: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E, 0x03, 0x02, 0xFF];
    const MENU_SELECT: &[u8] = &[0x81, 0x01, 0x06, 0x06, 0x05, 0xFF];
    const MENU_OFF: &[u8] = &[0x81, 0x01, 0x06, 0x06, 0x03, 0xFF];
    const MENU_STATUS_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x06, 0xFF];
    const MENU_STATUS_OPEN: &[u8] = &[0x90, 0x50, 0x02, 0xFF];
    const HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xFF];

    async fn build_camera(
        steps: Vec<Step>,
    ) -> (
        AsyncCamera<PtzOpticsG2, ScriptedTransport<TokioExecutor>, TokioExecutor>,
        ScriptedTransport<TokioExecutor>,
    ) {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(steps).with_executor(Arc::clone(&executor));
        let observed = transport.clone();
        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("camera");
        (camera, observed)
    }

    #[tokio::test]
    async fn menu_accessor_drives_the_osd_menu() {
        let (camera, transport) = build_camera(vec![
            helpers::command_response(MENU_ON.to_vec(), 1),
            helpers::command_response(MENU_DOWN.to_vec(), 1),
            helpers::command_response(MENU_SELECT.to_vec(), 1),
            helpers::inquiry_response(MENU_STATUS_INQUIRY.to_vec(), 1, MENU_STATUS_OPEN.to_vec()),
            helpers::command_response(MENU_OFF.to_vec(), 1),
        ])
        .await;

        camera.menu().open().await.expect("open the menu");
        camera.menu().down().await.expect("move the cursor down");
        camera.menu().enter().await.expect("select the menu item");
        assert!(camera.menu().is_open().await.expect("read the menu status"));
        camera.menu().close().await.expect("close the menu");

        assert_eq!(
            transport.sent(),
            vec![
                MENU_ON.to_vec(),
                MENU_DOWN.to_vec(),
                MENU_SELECT.to_vec(),
                MENU_STATUS_INQUIRY.to_vec(),
                MENU_OFF.to_vec(),
            ]
        );
    }

    #[tokio::test]
    async fn set_timeout_config_updates_the_view_and_the_runtime() {
        // The transport never answers, so the only thing that can produce a second
        // send is the runtime timing out the ACK and retrying.
        let (mut camera, transport) = build_camera(vec![]).await;

        let long_ack = TimeoutConfig::builder()
            .ack_timeout(Duration::from_secs(30))
            .build();
        camera
            .set_timeout_config(long_ack)
            .await
            .expect("the runtime accepts the new timeout configuration");
        assert_eq!(camera.timeout_config().ack_timeout, Duration::from_secs(30));

        // Held rather than awaited: the command stays outstanding while the
        // runtime waits for an ACK that never arrives.
        let _pending = camera.submit(&PanTilt::Home).await.expect("submit home");

        // The default 500ms ACK timeout would have retried well within this window.
        tokio::time::sleep(Duration::from_millis(1200)).await;

        assert_eq!(
            transport.sent(),
            vec![HOME.to_vec()],
            "the runtime must honour the updated ACK timeout instead of the default"
        );
    }
}
