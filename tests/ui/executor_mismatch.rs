//! This test verifies that cameras with different executor types cannot be mixed.

use grafton_visca::{
    camera::{Camera, profiles::PTZOpticsG2},
    executor_unified::{Executor, ExecError, TokioExecutor},
    transport::tokio::tcp::Tcp,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

// A mock executor for testing
struct MockExecutor;

impl Executor for MockExecutor {
    type Join<T: Send + 'static> = MockJoinHandle<T>;

    fn spawn<F>(&self, _future: F) -> Self::Join<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        MockJoinHandle { _phantom: std::marker::PhantomData }
    }

    fn sleep(&self, _duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        Box::pin(async {})
    }

    fn timeout<'a, F>(&self, _duration: Duration, future: F) -> Pin<Box<dyn Future<Output = Result<F::Output, ExecError>> + Send + 'a>>
    where
        F: Future + Send + 'a,
    {
        Box::pin(async move { Ok(future.await) })
    }
}

struct MockJoinHandle<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Send + 'static> Future for MockJoinHandle<T> {
    type Output = Result<T, ExecError>;

    fn poll(self: Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        std::task::Poll::Pending
    }
}

async fn main() {
    // Create two cameras with different executor types
    let transport1 = Tcp::connect("192.168.0.110:52381").await.unwrap();
    let transport2 = Tcp::connect("192.168.0.111:52381").await.unwrap();
    
    let tokio_executor = TokioExecutor::from_current().unwrap();
    let mock_executor = MockExecutor;
    
    let camera1 = Camera::<_, PTZOpticsG2, _, _>::with_executor(transport1, tokio_executor);
    let camera2 = Camera::<_, PTZOpticsG2, _, _>::with_executor(transport2, mock_executor);
    
    // This should fail to compile - cannot assign cameras with different executor types
    let _camera = camera1;
    let _camera = camera2; // Different executor type - should not compile!
}