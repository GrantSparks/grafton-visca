#![allow(dead_code)]

use grafton_visca::{
    camera::{CameraConfig, CameraSession, Connect},
    mode::Async,
    profiles::{PtzOpticsG2, SonyFR7},
    runtime::Runtime,
    transport::TransportConfig,
    Error, Executor,
};

fn async_session_accessors<Tr, Exec>(session: &CameraSession<Async, PtzOpticsG2, Tr, Exec>)
where
    Exec: Executor,
{
    let _ = session.power();
    let _ = session.zoom();
    let _ = session.pan_tilt();
    let _ = session.focus();
    let _ = session.exposure();
    let _ = session.white_balance();
    let _ = session.image();
    let _ = session.presets();
    let _ = session.menu();
    let _ = session.system();
    let _ = session.advanced();
}

fn async_fr7_optional_accessors<Tr, Exec>(session: &CameraSession<Async, SonyFR7, Tr, Exec>)
where
    Exec: Executor,
{
    let _ = session.nd_filter();
    let _ = session.tally();
}

async fn async_connect_contract<R>(runtime: R) -> Result<(), Error>
where
    R: Runtime + Clone,
{
    let tcp_camera =
        Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110:5678", runtime.clone()).await?;
    tcp_camera.power().state().await?;
    tcp_camera.zoom().stop().await?;
    let _ = tcp_camera.close().await?;

    let udp_camera =
        Connect::open_udp_async::<PtzOpticsG2, _>("192.168.0.110:1259", runtime).await?;
    udp_camera.pan_tilt().home().await?;
    let _ = udp_camera.close().await?;

    Ok(())
}

async fn configured_async_contract<R>(runtime: R) -> Result<(), Error>
where
    R: Runtime + Clone,
{
    let camera = CameraConfig::<PtzOpticsG2>::tcp("192.168.0.110")
        .transport_config(TransportConfig::default())
        .open_async(runtime)
        .await?;
    camera.focus().auto().await?;
    let _ = camera.close().await?;

    Ok(())
}

fn main() {
    let _ = Connect::builder().tcp("192.168.0.110").with_default_port();
    let _ = Connect::builder().udp("192.168.0.110:1259");
}
