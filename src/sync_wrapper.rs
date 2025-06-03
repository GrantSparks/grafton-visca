use crate::{ViscaCommand, ViscaError, ViscaResponse};

#[cfg(feature = "async")]
use crate::async_client::AsyncViscaClient;

/// Synchronous VISCA client wrapper around the async implementation.
///
/// This provides a blocking API for users who don't need async functionality.
/// Internally, it manages a tokio runtime to execute async operations.
#[cfg(all(feature = "sync", feature = "async"))]
pub struct ViscaClient {
    runtime: tokio::runtime::Runtime,
    async_client: AsyncViscaClient,
}

#[cfg(all(feature = "sync", feature = "async"))]
impl ViscaClient {
    /// Connect to a camera using UDP transport (blocking).
    pub fn connect_udp(camera_addr: &str) -> Result<Self, ViscaError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                ViscaError::Io(std::io::Error::other(format!(
                    "Failed to create tokio runtime: {}",
                    e
                )))
            })?;

        let async_client = runtime.block_on(AsyncViscaClient::connect_udp(camera_addr))?;

        Ok(Self {
            runtime,
            async_client,
        })
    }

    /// Connect to a camera using TCP transport (blocking).
    pub fn connect_tcp(camera_addr: &str) -> Result<Self, ViscaError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                ViscaError::Io(std::io::Error::other(format!(
                    "Failed to create tokio runtime: {}",
                    e
                )))
            })?;

        let async_client = runtime.block_on(AsyncViscaClient::connect_tcp(camera_addr))?;

        Ok(Self {
            runtime,
            async_client,
        })
    }

    /// Send a command and wait for the response (blocking).
    pub fn send_blocking(&self, command: &dyn ViscaCommand) -> Result<ViscaResponse, ViscaError> {
        self.runtime.block_on(self.async_client.send(command))
    }
}

/// Helper function to maintain backward compatibility with existing sync API.
///
/// This function provides the same interface as the original `send_command_and_wait`
/// but uses the new async implementation internally when both sync and async features
/// are enabled.
#[cfg(all(feature = "sync", feature = "async"))]
pub fn send_command_and_wait_compat(
    camera_addr: &str,
    command: &dyn ViscaCommand,
    use_tcp: bool,
) -> Result<ViscaResponse, ViscaError> {
    let client = if use_tcp {
        ViscaClient::connect_tcp(camera_addr)?
    } else {
        ViscaClient::connect_udp(camera_addr)?
    };

    client.send_blocking(command)
}
