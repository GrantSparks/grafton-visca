// Experimental: Using IntoFuture for cleaner blocking API
// This would allow .await to work in async mode and direct ? in blocking mode

use std::future::{ready, Future, IntoFuture, Ready};
use std::ops::{ControlFlow, Try};

// Custom wrapper for blocking mode that implements Try
#[derive(Debug)]
pub struct BlockingReturn<T>(pub T);

impl<T> Future for BlockingReturn<T> {
    type Output = T;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // This is a bit hacky but works - we take the value out
        // In practice, we'd use Option<T> internally
        std::task::Poll::Ready(unsafe { std::ptr::read(&self.0) })
    }
}

impl<T> IntoFuture for BlockingReturn<T> {
    type Output = T;
    type IntoFuture = Ready<T>;

    fn into_future(self) -> Self::IntoFuture {
        ready(self.0)
    }
}

// Make it work with ? operator directly
impl<T, E> Try for BlockingReturn<Result<T, E>> {
    type Output = T;
    type Residual = Result<std::convert::Infallible, E>;

    fn from_output(output: Self::Output) -> Self {
        BlockingReturn(Ok(output))
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self.0 {
            Ok(v) => ControlFlow::Continue(v),
            Err(e) => ControlFlow::Break(Err(e)),
        }
    }
}

impl<T, E> std::ops::FromResidual<Result<std::convert::Infallible, E>>
    for BlockingReturn<Result<T, E>>
{
    fn from_residual(residual: Result<std::convert::Infallible, E>) -> Self {
        match residual {
            Err(e) => BlockingReturn(Err(e)),
            Ok(_) => unreachable!(),
        }
    }
}

// Alternative Mode trait that could use this
pub trait ModeV2 {
    type Ret<'a, T>: Future<Output = T> + Send + 'a
    where
        Self: 'a,
        T: Send + 'a;

    fn ret<T>(result: T) -> Self::Ret<'static, T>
    where
        T: Send + 'static;
}

pub struct AsyncMode;
pub struct BlockingMode;

impl ModeV2 for AsyncMode {
    type Ret<'a, T>
        = std::pin::Pin<Box<dyn Future<Output = T> + Send + 'a>>
    where
        T: Send + 'a;

    fn ret<T>(result: T) -> Self::Ret<'static, T>
    where
        T: Send + 'static,
    {
        Box::pin(ready(result))
    }
}

impl ModeV2 for BlockingMode {
    type Ret<'a, T>
        = BlockingReturn<T>
    where
        T: Send + 'a;

    fn ret<T>(result: T) -> Self::Ret<'static, T>
    where
        T: Send + 'static,
    {
        BlockingReturn(result)
    }
}
