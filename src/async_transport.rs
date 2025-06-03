use crate::{ViscaCommand, ViscaError};
use std::future::Future;
use std::pin::Pin;

/// Async version of the ViscaTransport trait for non-blocking I/O operations.
/// 
/// This trait provides the async equivalent of ViscaTransport, allowing for
/// concurrent command execution and non-blocking network operations.
#[cfg(feature = "async")]
pub trait AsyncViscaTransport: Send + Sync {
    /// Send a VISCA command to the camera asynchronously.
    fn send_command<'a>(&'a mut self, command: &'a dyn ViscaCommand) 
        -> Pin<Box<dyn Future<Output = Result<(), ViscaError>> + Send + 'a>>;
    
    /// Receive response frames from the camera asynchronously.
    /// 
    /// Returns a vector of response frames that have been received.
    /// May return an empty vector if no complete frames are available yet.
    fn receive_response<'a>(&'a mut self) 
        -> Pin<Box<dyn Future<Output = Result<Vec<Vec<u8>>, ViscaError>> + Send + 'a>>;
}