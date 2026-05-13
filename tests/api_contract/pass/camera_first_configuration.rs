use grafton_visca::{
    camera::{CameraConfig, Connect, ConnectBuilder},
    profiles::PtzOpticsG2,
    transport::{RetryConfig, TransportConfig},
};

fn main() {
    let builder: ConnectBuilder = Connect::builder().tcp("127.0.0.1").with_default_port();
    let _ = format!("{builder:?}");
    let udp_builder: ConnectBuilder = Connect::builder().udp("127.0.0.1:1259");
    let _ = format!("{udp_builder:?}");

    let config = CameraConfig::<PtzOpticsG2>::new()
        .tcp()
        .address("127.0.0.1")
        .timeouts(Default::default())
        .retry_config(RetryConfig::default())
        .transport_config(TransportConfig::default())
        .camera_id(1)
        .unwrap();
    let _ = config.clone();

    #[cfg(not(feature = "mode-async"))]
    {
        fn primary_blocking_path(addr: &str) -> Result<(), grafton_visca::Error> {
            let camera = Connect::open_udp_blocking::<PtzOpticsG2>(addr)?;
            camera.power().state()?;
            camera.zoom().stop()?;
            camera.close()
        }

        let _: fn(&str) -> Result<(), grafton_visca::Error> = primary_blocking_path;
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
