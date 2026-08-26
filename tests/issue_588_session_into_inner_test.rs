//! Issue #588: extracting the owned camera from a `CameraSession`.
//!
//! `Connect::open_tcp_async` and friends hand back a `CameraSession`, but the
//! owned-camera APIs — `IntoDynCamera::into_dyn()` above all — need the `Camera`
//! itself. The async session had `camera()`/`camera_mut()` only, so those APIs
//! were unreachable for every user of the convenience helpers.
//!
//! These tests pin the extractor's contract: `into_inner()` is a handoff, not a
//! close, so the camera it returns still drives the same live connection.

/// Compile-time proof that the blocking session exposes the timeout setter.
///
/// The blocking `CameraSession` has no public constructor today (blocking
/// connections return `BlockingClient`), so this surface can only be checked at
/// the type level.
#[cfg(not(feature = "mode-async"))]
fn _blocking_session_surface<P, Tr>(
    session: &mut grafton_visca::camera::CameraSession<grafton_visca::mode::Blocking, P, Tr, ()>,
    timeout_config: grafton_visca::timeout::TimeoutConfig,
) where
    P: grafton_visca::capabilities::Profile,
{
    session.set_timeout_config(timeout_config);
    let _camera: &grafton_visca::camera::Camera<grafton_visca::mode::Blocking, P, Tr, ()> =
        session.camera();
}

#[cfg(feature = "runtime-tokio")]
mod tokio_tests {
    use std::{
        net::SocketAddr,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Camera, CameraSession, Connect},
        mode::Async,
        runtime::{TokioRuntime, TransportHandle},
    };

    const HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xFF];
    const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF];
    const VISCA_TERMINATOR: u8 = 0xFF;

    type Frames = Arc<Mutex<Vec<Vec<u8>>>>;

    /// A minimal VISCA camera on loopback: it records every frame it receives and
    /// answers each command with ACK + completion on socket 1.
    async fn spawn_fake_camera() -> (SocketAddr, Frames) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind the fake camera");
        let addr = listener.local_addr().expect("read the fake camera address");
        let frames: Frames = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&frames);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept the camera client");
            let mut chunk = [0u8; 256];
            let mut frame = Vec::new();

            loop {
                let read = match stream.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => read,
                };

                for &byte in &chunk[..read] {
                    frame.push(byte);
                    if byte != VISCA_TERMINATOR {
                        continue;
                    }

                    let is_inquiry = frame.get(1) == Some(&0x09);
                    recorded
                        .lock()
                        .expect("record the received frame")
                        .push(std::mem::take(&mut frame));

                    if is_inquiry {
                        continue;
                    }
                    if stream
                        .write_all(&[0x90, 0x41, VISCA_TERMINATOR])
                        .await
                        .is_err()
                        || stream
                            .write_all(&[0x90, 0x51, VISCA_TERMINATOR])
                            .await
                            .is_err()
                    {
                        return;
                    }
                }
            }
        });

        (addr, frames)
    }

    fn recorded_frames(frames: &Frames) -> Vec<Vec<u8>> {
        frames.lock().expect("read the recorded frames").clone()
    }

    /// The extracted camera must still drive the connection the session opened.
    #[tokio::test]
    async fn into_inner_hands_over_a_live_camera() {
        let (addr, frames) = spawn_fake_camera().await;
        let runtime = TokioRuntime::from_current().expect("a tokio runtime");

        let session: CameraSession<
            Async,
            PtzOpticsG2,
            TransportHandle<TokioRuntime>,
            TokioRuntime,
        > = Connect::open_tcp_async::<PtzOpticsG2, _>(addr.to_string(), runtime)
            .await
            .expect("open the session");

        let camera: Camera<Async, PtzOpticsG2, TransportHandle<TokioRuntime>, TokioRuntime> =
            session.into_inner();

        // The session is gone, but its transport and runtime task are not: the
        // camera keeps driving the same connection.
        camera
            .pan_tilt()
            .home()
            .await
            .expect("the extracted camera still reaches the camera");
        camera
            .zoom()
            .stop()
            .await
            .expect("the extracted camera keeps working");

        let sent = recorded_frames(&frames);
        assert!(
            sent.contains(&HOME.to_vec()) && sent.contains(&ZOOM_STOP.to_vec()),
            "both commands should have reached the camera over the handed-over connection, got {sent:02X?}"
        );

        camera.shutdown().await.expect("shut the camera down");
    }

    /// `into_inner()` must not tear the connection down on its own: teardown
    /// travels with the returned camera and happens when that camera is dropped.
    #[tokio::test]
    async fn into_inner_defers_teardown_to_the_camera() {
        let (addr, frames) = spawn_fake_camera().await;
        let runtime = TokioRuntime::from_current().expect("a tokio runtime");

        let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(addr.to_string(), runtime)
            .await
            .expect("open the session")
            .into_inner();

        // A full command round trip proves the connection survived `into_inner`.
        camera
            .pan_tilt()
            .home()
            .await
            .expect("the connection is still open after into_inner");
        assert_eq!(recorded_frames(&frames), vec![HOME.to_vec()]);

        drop(camera);

        // Dropping the camera performs the teardown the session would have done.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            recorded_frames(&frames),
            vec![HOME.to_vec()],
            "teardown must not put anything else on the wire"
        );
    }

    /// The reason the extractor exists: reaching `into_dyn()` from `Connect`.
    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn into_inner_reaches_the_dyn_api() {
        use grafton_visca::dynapi::{DynCameraControl, IntoDynCamera};

        let (addr, frames) = spawn_fake_camera().await;
        let runtime = TokioRuntime::from_current().expect("a tokio runtime");

        let camera: Box<dyn DynCameraControl> = Box::new(
            Connect::open_tcp_async::<PtzOpticsG2, _>(addr.to_string(), runtime)
                .await
                .expect("open the session")
                .into_inner()
                .into_dyn(),
        );

        camera
            .pan_tilt()
            .pan_tilt_home(Some(Duration::from_secs(2)))
            .await
            .expect("the dyn camera drives the handed-over connection");

        assert_eq!(recorded_frames(&frames), vec![HOME.to_vec()]);
    }
}
