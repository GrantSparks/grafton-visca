#![allow(unused_imports)]

#[cfg(not(feature = "mode-async"))]
use grafton_visca::profiles::SonyFR7;
use grafton_visca::{
    camera::{
        AwaitConfig, Camera, CameraBuilder, CameraConfig, CameraSession, CommandId, Connect,
        FocusAccessor, FocusOperation, PanTiltAccessor, PanTiltOperation, PowerAccessor,
        PresetOperation, TcpConnectBuilder, TransportKind, TransportOptions, UdpConnectBuilder,
        ZoomAccessor, ZoomOperation,
    },
    profiles::PtzOpticsG2,
    transport::{RetryConfig, TransportConfig},
    CameraId,
};

fn main() {
    let builder: TcpConnectBuilder = Connect::builder().tcp("127.0.0.1").with_default_port();
    let _ = format!("{builder:?}");
    let udp_builder: UdpConnectBuilder = Connect::builder().udp("127.0.0.1:1259");
    let _ = format!("{udp_builder:?}");

    let config = CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1")
        .timeouts(Default::default())
        .retry_config(RetryConfig::default())
        .transport_config(TransportConfig::default())
        .try_camera_id(1)
        .unwrap();
    let _ = config.clone();
    let _ = CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1").camera_id(CameraId::CAMERA_1);
    let _ = CameraBuilder::new()
        .camera_id(CameraId::CAMERA_1)
        .try_camera_id(1)
        .unwrap();
    let _ = TransportKind::Tcp;
    let _ = TransportOptions::tcp("127.0.0.1");
    let _ = core::any::TypeId::of::<AwaitConfig>();
    let _ = core::any::TypeId::of::<CommandId>();
    let _ = core::any::TypeId::of::<PanTiltOperation>();
    let _ = core::any::TypeId::of::<ZoomOperation>();
    let _ = core::any::TypeId::of::<FocusOperation>();
    let _ = core::any::TypeId::of::<PresetOperation>();

    #[cfg(not(feature = "mode-async"))]
    {
        type Mode = grafton_visca::mode::Blocking;
        type Transport = grafton_visca::transport::BlockingTransportHandle;
        let _ = core::any::TypeId::of::<Camera<Mode, PtzOpticsG2, Transport, ()>>();
        let _ = core::any::TypeId::of::<CameraSession<Mode, PtzOpticsG2, Transport, ()>>();
        let _ = core::any::TypeId::of::<PowerAccessor<'static, Mode, PtzOpticsG2, Transport, ()>>();
        let _ = core::any::TypeId::of::<ZoomAccessor<'static, Mode, PtzOpticsG2, Transport, ()>>();
        let _ =
            core::any::TypeId::of::<PanTiltAccessor<'static, Mode, PtzOpticsG2, Transport, ()>>();
        let _ = core::any::TypeId::of::<FocusAccessor<'static, Mode, PtzOpticsG2, Transport, ()>>();
    }

    #[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
    {
        type Mode = grafton_visca::mode::Async;
        type Transport = grafton_visca::runtime::TransportHandle<grafton_visca::TokioRuntime>;
        type Exec = grafton_visca::TokioRuntime;
        let _ = core::any::TypeId::of::<Camera<Mode, PtzOpticsG2, Transport, Exec>>();
        let _ = core::any::TypeId::of::<CameraSession<Mode, PtzOpticsG2, Transport, Exec>>();
        let _ =
            core::any::TypeId::of::<PowerAccessor<'static, Mode, PtzOpticsG2, Transport, Exec>>();
        let _ =
            core::any::TypeId::of::<ZoomAccessor<'static, Mode, PtzOpticsG2, Transport, Exec>>();
        let _ =
            core::any::TypeId::of::<PanTiltAccessor<'static, Mode, PtzOpticsG2, Transport, Exec>>();
        let _ =
            core::any::TypeId::of::<FocusAccessor<'static, Mode, PtzOpticsG2, Transport, Exec>>();
        let _ = core::any::TypeId::of::<
            grafton_visca::camera::InFlight<'static, ZoomOperation, PtzOpticsG2, Exec>,
        >();
    }

    #[cfg(not(feature = "mode-async"))]
    {
        fn primary_blocking_path(addr: &str) -> Result<(), grafton_visca::Error> {
            let camera = Connect::open_udp_blocking::<PtzOpticsG2>(addr)?;
            camera.power().state()?;
            camera.zoom().stop()?;
            camera.close()
        }

        let _: fn(&str) -> Result<(), grafton_visca::Error> = primary_blocking_path;

        fn sony_udp_path(addr: &str) -> Result<(), grafton_visca::Error> {
            let camera = Connect::open_udp_blocking::<SonyFR7>(addr)?;
            camera.close()
        }

        fn sony_udp_builder_path(addr: &str) -> Result<(), grafton_visca::Error> {
            let camera = Connect::builder()
                .udp(addr)
                .with_default_port()
                .open::<SonyFR7>()?;
            camera.close()
        }

        let _: fn(&str) -> Result<(), grafton_visca::Error> = sony_udp_path;
        let _: fn(&str) -> Result<(), grafton_visca::Error> = sony_udp_builder_path;
    }

    #[cfg(feature = "runtime-tokio")]
    {
        async fn primary_tokio_path(addr: &str) -> Result<(), grafton_visca::Error> {
            let runtime = grafton_visca::runtime::TokioRuntime::from_current()?;
            let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(addr, runtime).await?;
            camera.power().state().await?;
            camera.zoom().stop().await?;
            camera.close().await?;
            Ok(())
        }

        let _ = primary_tokio_path;
    }

    #[cfg(feature = "runtime-smol")]
    {
        async fn primary_smol_path(addr: &str) -> Result<(), grafton_visca::Error> {
            let runtime = grafton_visca::runtime::SmolRuntime::new();
            let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(addr, runtime).await?;
            camera.power().state().await?;
            camera.zoom().stop().await?;
            camera.close().await?;
            Ok(())
        }

        let _ = primary_smol_path;
    }
}
