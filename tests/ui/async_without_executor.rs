//! Test that async operations require an executor to be provided.

use grafton_visca::prelude::*;

#[tokio::main]
async fn main() {
    #[cfg(feature = "async")]
    {
        let transport = TcpTransport::new("192.168.1.100:5678");
        
        // This should fail: Cannot create async camera without executor
        let camera = Camera::<PTZOpticsG2, TcpTransport, ()>::new(transport); //~ ERROR
        
        // This should fail: Cannot use async methods without executor
        camera.send_command(PowerOn).await; //~ ERROR
    }
}