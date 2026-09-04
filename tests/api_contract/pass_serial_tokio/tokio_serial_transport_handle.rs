#![allow(dead_code)]
#![cfg(feature = "transport-serial-tokio")]

//! A Tokio serial transport remains directly constructible in its matching
//! runtime handle. The body is type-only: opening an OS serial port is not
//! part of a public compile contract.

use grafton_visca::{
    runtime::TokioRuntime, runtime_adapters::tokio::SerialTransport, TransportHandle,
};

fn tokio_serial() -> SerialTransport {
    panic!("compile-only serial transport")
}

fn tokio_serial_handle() {
    let _: TransportHandle<TokioRuntime> = TransportHandle::Serial(Box::new(tokio_serial()));
}

fn main() {
    let _ = tokio_serial_handle;
}
