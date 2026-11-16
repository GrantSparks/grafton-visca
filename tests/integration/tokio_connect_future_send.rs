//! Tokio standard constructors must produce movable, owned futures before I/O.
//!
//! This is deliberately a compile-only regression: polling would attempt real
//! network or serial-device setup. Boxing both a `Send + 'static` future and
//! its `Send + 'static` output proves the public constructors satisfy Tokio's
//! task-spawn boundary without starting I/O.

#![cfg(feature = "runtime-tokio")]

use std::{future::Future, pin::Pin};

use grafton_visca::{profiles::PtzOpticsG2, runtime::TokioRuntime};

fn boxed_send<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
) -> Pin<Box<dyn Future<Output = T> + Send + 'static>> {
    Box::pin(future)
}

#[test]
fn tokio_standard_connect_futures_are_send_before_polling() -> Result<(), Box<dyn std::error::Error>>
{
    let selected = tokio::runtime::Builder::new_current_thread().build()?;
    let runtime = TokioRuntime::from_handle(selected.handle().clone());

    drop(boxed_send(grafton_visca::r#async::CameraSession::<
        PtzOpticsG2,
    >::connect_tcp(
        String::from("127.0.0.1:5678"),
        runtime.clone(),
    )));
    drop(boxed_send(grafton_visca::r#async::CameraSession::<
        PtzOpticsG2,
    >::connect_udp(
        String::from("127.0.0.1:1259"),
        runtime.clone(),
    )));

    #[cfg(feature = "transport-serial-tokio")]
    drop(boxed_send(grafton_visca::r#async::CameraSession::<
        PtzOpticsG2,
    >::connect_serial(
        String::from("grafton-visca-no-poll-serial-device"),
        9_600,
        runtime,
    )));

    Ok(())
}
