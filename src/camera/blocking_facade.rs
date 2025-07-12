//! Blocking facade for Camera that provides synchronous methods.

use crate::{
    camera::core::CameraCore, capabilities::ProfileMetadata, transport::core::BlockingTransport,
};

/// Blocking camera interface that wraps CameraCore with sync methods.
///
/// This provides a synchronous API by using a minimal executor to
/// block on the futures returned by CameraCore.
///
/// # Type Constraints
/// 
/// This type requires a `BlockingTransport` to ensure it's only used with
/// transports that return immediately-ready futures. Using an async transport
/// would cause a runtime panic.
///
/// # Compile-Time Safety
///
/// The type system prevents using async transports with `CameraBlocking`:
///
/// ```compile_fail
/// use grafton_visca::{CameraBlocking, profiles::PTZOpticsG2};
/// use grafton_visca::transport::tokio::Tcp;
/// 
/// // This will not compile because tokio::Tcp doesn't implement BlockingTransport
/// let transport = Tcp::connect("192.168.1.1:52381").await.unwrap();
/// let camera: CameraBlocking<PTZOpticsG2, Tcp> = CameraBlocking::new(transport);
/// ```
///
/// Only blocking transports can be used:
///
/// ```no_run
/// use grafton_visca::{CameraBlocking, profiles::PTZOpticsG2};
/// use grafton_visca::transport::blocking::Tcp;
/// 
/// // This compiles because blocking::Tcp implements BlockingTransport
/// let transport = Tcp::connect("192.168.1.1:52381").unwrap();
/// let camera = CameraBlocking::<PTZOpticsG2, _>::new(transport);
/// ```
#[derive(Debug)]
pub struct CameraBlocking<P, T>
where
    P: ProfileMetadata,
    T: BlockingTransport,
{
    core: CameraCore<P, T>,
}

impl<P, T> CameraBlocking<P, T>
where
    P: ProfileMetadata,
    T: BlockingTransport,
{
    /// Create a new blocking camera instance.
    pub fn new(transport: T) -> Self {
        Self {
            core: CameraCore::new(transport),
        }
    }

    /// Get a reference to the core camera.
    pub fn core(&self) -> &CameraCore<P, T> {
        &self.core
    }

    /// Get the camera profile information.
    pub fn profile_info(&self) -> &'static str {
        self.core.profile_info()
    }
}

// Extension traits will add blocking methods that call the core methods
// and block on the futures. For example:
//
// impl<P, T> ZoomMethodsExt for CameraBlocking<P, T>
// where
//     P: ProfileMetadata + Zoom,
//     T: Transport,
// {
//     fn zoom_stop(&self) -> Result<(), Error> {
//         let command = ZoomStopCommand::new();
//         block_on(self.core.send_command(&command))?;
//         Ok(())
//     }
// }

/// Extension trait to convert async Camera to blocking.
///
/// This trait is only implemented when the transport is a `BlockingTransport`,
/// preventing runtime panics from mismatched transport types.
pub trait BlockingExt<P, T>
where
    P: ProfileMetadata,
    T: BlockingTransport,
{
    /// Get a blocking view of this camera.
    fn blocking(self) -> CameraBlocking<P, T>;
}

impl<P, T> BlockingExt<P, T> for crate::camera::async_facade::CameraAsync<P, T>
where
    P: ProfileMetadata,
    T: BlockingTransport,
{
    fn blocking(self) -> CameraBlocking<P, T> {
        CameraBlocking { core: self.core }
    }
}
