//! Test that executor types must match between camera and socket manager.

use grafton_visca::prelude::*;
use grafton_visca::{TokioExecutor, Executor};
use std::sync::Arc;

// Different executor implementation
struct AlternativeExecutor;

#[cfg(feature = "async")]
impl Executor for AlternativeExecutor {
    type Join<T> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, grafton_visca::ExecError>> + Send + 'static>>
    where
        T: Send + 'static;
    
    fn spawn<F>(&self, _fut: F) -> Self::Join<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        unimplemented!()
    }
    
    fn block_on<F: std::future::Future>(&self, _fut: F) -> F::Output {
        unimplemented!()
    }
    
    fn sleep(&self, _duration: std::time::Duration) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        unimplemented!()
    }
    
    fn timeout<'a, F, T>(
        &'a self,
        _duration: std::time::Duration,
        _fut: F,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, grafton_visca::Error>> + Send + 'a>>
    where
        F: std::future::Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        unimplemented!()
    }
}

fn main() {
    #[cfg(all(feature = "rt-tokio", feature = "async"))]
    {
        let tokio_exec = Arc::new(TokioExecutor::from_current().unwrap());
        let alt_exec = Arc::new(AlternativeExecutor);
        
        // Create camera with Tokio executor
        let mut camera: Camera<PTZOpticsG2, TcpTransport, TokioExecutor> = 
            Camera::with_executor(TcpTransport::new("192.168.1.100:5678"), tokio_exec.clone());
        
        // This should fail: Cannot pass different executor type to socket manager
        // (In practice, the socket manager is created internally with the same executor,
        // but this test demonstrates the type safety)
        let _socket_manager = camera.ensure_socket_manager_with_executor(alt_exec); //~ ERROR mismatched types
    }
}