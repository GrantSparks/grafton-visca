//! Test that blocking cameras cannot use async methods.

use grafton_visca::prelude::blocking::*;

fn main() {
    let transport = TcpTransportBlocking::new("192.168.1.100:5678").unwrap();
    let camera = BlockingCamera::<PTZOpticsG2, TcpTransportBlocking>::new(transport);
    
    // This should fail: Blocking camera doesn't have async methods
    let future = camera.send_command(PowerOn); //~ ERROR no method named `send_command` found
    
    // This should fail: Cannot await in non-async context
    future.await; //~ ERROR `await` is only allowed inside `async` functions and blocks
}