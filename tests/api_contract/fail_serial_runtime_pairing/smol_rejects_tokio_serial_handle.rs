#![allow(dead_code)]

//! The serial payload of `TransportHandle<R>` is selected by `R`. A Tokio
//! serial transport therefore cannot be attached to a Smol runtime handle in
//! safe code, where it would otherwise be polled by the wrong reactor.

use grafton_visca::{
    runtime::SmolRuntime, runtime_adapters::tokio::SerialTransport, TransportHandle,
};

fn tokio_serial() -> SerialTransport {
    panic!("compile-only serial transport")
}

fn smol_cannot_hold_tokio_serial() {
    let _: TransportHandle<SmolRuntime> = TransportHandle::Serial(Box::new(tokio_serial()));
}

fn main() {
    let _ = smol_cannot_hold_tokio_serial;
}

//~ E0271
