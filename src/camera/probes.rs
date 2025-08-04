//! Probe implementations for blocking and async cameras.

use std::{future::Ready, pin::Pin, time::Duration};

use crate::{capabilities::Profile, error::Error, transport::UnifiedTransport};

use super::{
    generic::Camera,
    movement_probe::{FocusProbe, MovementProbe, PanTiltPosition, ZoomProbe},
};

// ============= Blocking Probes =============

/// Ready future for blocking operations (immediately ready).
#[derive(Debug, Copy, Clone)]
pub struct BlockingSleep;

impl std::future::Future for BlockingSleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        std::task::Poll::Ready(())
    }
}

/// Blocking probe for pan/tilt movement.
#[derive(Debug)]
pub struct BlockingPanTiltProbe<'a, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    camera: &'a Camera<P, T>,
}

impl<'a, P, T> BlockingPanTiltProbe<'a, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    /// Create a new blocking pan/tilt probe.
    pub fn new(camera: &'a Camera<P, T>) -> Self {
        Self { camera }
    }
}

impl<P, T> MovementProbe for BlockingPanTiltProbe<'_, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    type Sleep<'b>
        = BlockingSleep
    where
        Self: 'b;
    type PositionFuture<'b>
        = Ready<Result<PanTiltPosition, Error>>
    where
        Self: 'b;

    fn get_position(&self) -> Self::PositionFuture<'_> {
        use crate::camera::methods::inquiry::PanTiltInquiryOpsBlocking;

        let result = self
            .camera
            .get_pan_tilt_position()
            .map(|(pan, tilt)| PanTiltPosition { pan, tilt });
        std::future::ready(result)
    }

    fn sleep(&self, duration: Duration) -> Self::Sleep<'_> {
        std::thread::sleep(duration);
        BlockingSleep
    }
}

/// Blocking probe for zoom movement.
#[derive(Debug)]
pub struct BlockingZoomProbe<'a, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    camera: &'a Camera<P, T>,
}

impl<'a, P, T> BlockingZoomProbe<'a, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    /// Create a new blocking zoom probe.
    pub fn new(camera: &'a Camera<P, T>) -> Self {
        Self { camera }
    }
}

impl<P, T> ZoomProbe for BlockingZoomProbe<'_, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    type Sleep<'b>
        = BlockingSleep
    where
        Self: 'b;
    type ZoomFuture<'b>
        = Ready<Result<u16, Error>>
    where
        Self: 'b;

    fn get_zoom(&self) -> Self::ZoomFuture<'_> {
        use crate::camera::methods::inquiry::InquiryOpsBlocking;

        let result = self.camera.get_zoom_position();
        std::future::ready(result)
    }

    fn sleep(&self, duration: Duration) -> Self::Sleep<'_> {
        std::thread::sleep(duration);
        BlockingSleep
    }
}

/// Blocking probe for focus movement.
#[derive(Debug)]
pub struct BlockingFocusProbe<'a, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    camera: &'a Camera<P, T>,
}

impl<'a, P, T> BlockingFocusProbe<'a, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    /// Create a new blocking focus probe.
    pub fn new(camera: &'a Camera<P, T>) -> Self {
        Self { camera }
    }
}

impl<P, T> FocusProbe for BlockingFocusProbe<'_, P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    type Sleep<'b>
        = BlockingSleep
    where
        Self: 'b;
    type FocusFuture<'b>
        = Ready<Result<u16, Error>>
    where
        Self: 'b;

    fn get_focus(&self) -> Self::FocusFuture<'_> {
        use crate::camera::methods::inquiry::InquiryOpsBlocking;

        let result = self.camera.get_focus_position();
        std::future::ready(result)
    }

    fn sleep(&self, duration: Duration) -> Self::Sleep<'_> {
        std::thread::sleep(duration);
        BlockingSleep
    }
}

// ============= Async Probes =============

#[cfg(feature = "tokio")]
/// Async probe for pan/tilt movement using tokio.
#[derive(Debug, Clone)]
pub struct TokioPanTiltProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    camera: Camera<P, T>,
}

#[cfg(feature = "tokio")]
impl<P, T> TokioPanTiltProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    /// Create a new tokio pan/tilt probe.
    pub fn new(camera: &Camera<P, T>) -> Self {
        Self {
            camera: camera.clone(),
        }
    }
}

#[cfg(feature = "tokio")]
impl<P, T> MovementProbe for TokioPanTiltProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    type Sleep<'b>
        = tokio::time::Sleep
    where
        Self: 'b;
    type PositionFuture<'b>
        = Pin<Box<dyn std::future::Future<Output = Result<PanTiltPosition, Error>> + Send + 'b>>
    where
        Self: 'b;

    fn get_position(&self) -> Self::PositionFuture<'_> {
        Box::pin(async move {
            use crate::camera::methods::inquiry::PanTiltInquiryOps;

            let (pan, tilt) = self.camera.get_pan_tilt_position().await?;
            Ok(PanTiltPosition { pan, tilt })
        })
    }

    fn sleep(&self, duration: Duration) -> Self::Sleep<'_> {
        tokio::time::sleep(duration)
    }
}

#[cfg(feature = "tokio")]
/// Async probe for zoom movement using tokio.
#[derive(Debug, Clone)]
pub struct TokioZoomProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    camera: Camera<P, T>,
}

#[cfg(feature = "tokio")]
impl<P, T> TokioZoomProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    /// Create a new tokio zoom probe.
    pub fn new(camera: &Camera<P, T>) -> Self {
        Self {
            camera: camera.clone(),
        }
    }
}

#[cfg(feature = "tokio")]
impl<P, T> ZoomProbe for TokioZoomProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    type Sleep<'b>
        = tokio::time::Sleep
    where
        Self: 'b;
    type ZoomFuture<'b>
        = Pin<Box<dyn std::future::Future<Output = Result<u16, Error>> + Send + 'b>>
    where
        Self: 'b;

    fn get_zoom(&self) -> Self::ZoomFuture<'_> {
        Box::pin(async move {
            use crate::camera::methods::inquiry::InquiryOps;

            self.camera.get_zoom_position().await
        })
    }

    fn sleep(&self, duration: Duration) -> Self::Sleep<'_> {
        tokio::time::sleep(duration)
    }
}

#[cfg(feature = "tokio")]
/// Async probe for focus movement using tokio.
#[derive(Debug, Clone)]
pub struct TokioFocusProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    camera: Camera<P, T>,
}

#[cfg(feature = "tokio")]
impl<P, T> TokioFocusProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    /// Create a new tokio focus probe.
    pub fn new(camera: &Camera<P, T>) -> Self {
        Self {
            camera: camera.clone(),
        }
    }
}

#[cfg(feature = "tokio")]
impl<P, T> FocusProbe for TokioFocusProbe<P, T>
where
    P: Profile,
    T: UnifiedTransport,
{
    type Sleep<'b>
        = tokio::time::Sleep
    where
        Self: 'b;
    type FocusFuture<'b>
        = Pin<Box<dyn std::future::Future<Output = Result<u16, Error>> + Send + 'b>>
    where
        Self: 'b;

    fn get_focus(&self) -> Self::FocusFuture<'_> {
        Box::pin(async move {
            use crate::camera::methods::inquiry::InquiryOps;

            self.camera.get_focus_position().await
        })
    }

    fn sleep(&self, duration: Duration) -> Self::Sleep<'_> {
        tokio::time::sleep(duration)
    }
}
