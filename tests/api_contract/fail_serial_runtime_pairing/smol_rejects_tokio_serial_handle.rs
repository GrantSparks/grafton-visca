#![allow(dead_code)]

//! The serial payload of `TransportHandle<R>` is selected by `R`. A Tokio
//! serial transport therefore cannot be attached to a Smol runtime handle in
//! safe code, where it would otherwise be polled by the wrong reactor.

use grafton_visca::{
    runtime::SmolRuntime, runtime::TransportHandle, transport::tokio::SerialTransport,
};

fn tokio_serial() -> SerialTransport {
    panic!("compile-only serial transport")
}

fn smol_cannot_hold_tokio_serial() {
    let _: TransportHandle<SmolRuntime> = TransportHandle::Serial(Box::new(tokio_serial()));
    //~^ E0271 "type mismatch resolving `<SmolRuntime as Runtime>::SerialTransport == Serial<TokioSerialAdapter>`"
}

fn main() {
    let _ = smol_cannot_hold_tokio_serial;
}
