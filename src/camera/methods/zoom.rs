//! Zoom methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{ProfileMetadata, ValidationError, Zoom},
    command::{
        const_encoding::{commands, encode_speed, encode_u16_visca, CommandBuilder},
        Command, Response, ResponseType,
    },
    transport::gat_transport::Transport,
    Error,
};
use core::future::Future;

/// Zoom stop command.
struct ZoomStopCommand([u8; 7]);

impl ZoomStopCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(commands::ZOOM_STOP);
        Self(cmd.build())
    }
}

impl Command for ZoomStopCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Extension trait for CameraCore - provides future-returning methods.
pub trait ZoomCoreExt<P, T>
where
    P: ProfileMetadata + Zoom,
    T: Transport,
{
    /// Stop zooming - returns a future.
    fn zoom_stop(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Start zooming in - returns a future.
    fn zoom_in(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Start zooming out - returns a future.
    fn zoom_out(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set zoom to absolute position - returns a future.
    fn zoom_absolute(&self, position: f32) -> impl Future<Output = Result<(), Error>> + '_;
}

#[allow(clippy::manual_async_fn)]
impl<P, T> ZoomCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + Zoom,
    T: Transport,
{
    fn zoom_stop(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = ZoomStopCommand::new();
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn zoom_in(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            // Use medium speed by default
            let speed = P::ZOOM_SPEED_RANGE.end / 2;

            let mut cmd = CommandBuilder::<7>::new();
            cmd.append(commands::ZOOM_TELE_PREFIX)
                .push(encode_speed(speed));

            struct ZoomInCommand([u8; 7]);
            impl Command for ZoomInCommand {
                fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                    Ok(self.0.to_vec())
                }
                fn response_type(&self) -> Option<ResponseType> {
                    None
                }
            }

            let command = ZoomInCommand(cmd.build());
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn zoom_out(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            // Use medium speed by default
            let speed = P::ZOOM_SPEED_RANGE.end / 2;

            let mut cmd = CommandBuilder::<7>::new();
            cmd.append(commands::ZOOM_WIDE_PREFIX)
                .push(encode_speed(speed));

            struct ZoomOutCommand([u8; 7]);
            impl Command for ZoomOutCommand {
                fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                    Ok(self.0.to_vec())
                }
                fn response_type(&self) -> Option<ResponseType> {
                    None
                }
            }

            let command = ZoomOutCommand(cmd.build());
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn zoom_absolute(&self, position: f32) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            // Validate position
            if !(0.0..=1.0).contains(&position) {
                return Err(Error::ValidationError(ValidationError::OutOfRange {
                    parameter: "zoom position",
                    value: position as f64,
                    min: 0.0,
                    max: 1.0,
                }));
            }

            // Convert normalized position to VISCA units
            let max_zoom = P::DIGITAL_ZOOM_MAX.unwrap_or(P::OPTICAL_ZOOM_MAX);
            let zoom_pos = (position * max_zoom as f32) as u16;

            let mut cmd = CommandBuilder::<10>::new();
            cmd.append(commands::ZOOM_ABSOLUTE_PREFIX);
            encode_u16_visca(zoom_pos, &mut cmd);

            struct ZoomAbsoluteCommand([u8; 10]);
            impl Command for ZoomAbsoluteCommand {
                fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                    Ok(self.0.to_vec())
                }
                fn response_type(&self) -> Option<ResponseType> {
                    None
                }
            }

            let command = ZoomAbsoluteCommand(cmd.build());
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for async Camera facade.
pub trait ZoomAsyncExt<P, T>
where
    P: ProfileMetadata + Zoom,
    T: Transport,
{
    /// Stop zooming.
    fn zoom_stop(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Start zooming in (telephoto).
    fn zoom_in(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Start zooming out (wide).
    fn zoom_out(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    fn zoom_absolute(&self, position: f32) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> ZoomAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + Zoom + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn zoom_stop(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().zoom_stop().await }
    }

    fn zoom_in(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().zoom_in().await }
    }

    fn zoom_out(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().zoom_out().await }
    }

    fn zoom_absolute(&self, position: f32) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().zoom_absolute(position).await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait ZoomBlockingExt<P, T>
where
    P: ProfileMetadata + Zoom,
    T: Transport,
{
    /// Stop zooming.
    fn zoom_stop(&self) -> Result<(), Error>;

    /// Start zooming in (telephoto).
    fn zoom_in(&self) -> Result<(), Error>;

    /// Start zooming out (wide).
    fn zoom_out(&self) -> Result<(), Error>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    fn zoom_absolute(&self, position: f32) -> Result<(), Error>;
}

impl<P, T> ZoomBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + Zoom,
    T: Transport,
{
    fn zoom_stop(&self) -> Result<(), Error> {
        block_on(self.core().zoom_stop())
    }

    fn zoom_in(&self) -> Result<(), Error> {
        block_on(self.core().zoom_in())
    }

    fn zoom_out(&self) -> Result<(), Error> {
        block_on(self.core().zoom_out())
    }

    fn zoom_absolute(&self, position: f32) -> Result<(), Error> {
        block_on(self.core().zoom_absolute(position))
    }
}
