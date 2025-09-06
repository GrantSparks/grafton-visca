//! Camera construction methods using the Runtime trait for type-safe pairing.

use crate::{
    capabilities::Profile,
    runtime_trait::{Runtime, TransportHandle},
    transport::Transport,
    Error,
};

use super::Camera;

// Camera construction methods that use the Runtime trait for type-safe pairing
#[cfg(feature = "async")]
impl<P, R> Camera<crate::mode::Async, P, TransportHandle<R>, R>
where
    P: Profile + Default,
    R: Runtime,
{
    /// Create a new async camera instance with a TransportHandle.
    /// This extracts the RetryConfig from the TransportHandle and passes it to the RuntimeHandle.
    pub async fn new_async_runtime(
        transport: TransportHandle<R>,
        executor: R,
    ) -> Result<Self, Error> {
        Self::new_async_with_runtime_transport(transport, executor, P::PROTOCOL_STYLE).await
    }

    /// Create a new async camera instance with explicit protocol style and TransportHandle.
    /// This extracts the RetryConfig from the TransportHandle and passes it to the RuntimeHandle.
    async fn new_async_with_runtime_transport(
        transport: TransportHandle<R>,
        executor: R,
        protocol_style: crate::capabilities::ProtocolStyle,
    ) -> Result<Self, Error> {
        let camera_id = crate::camera_id::CameraId::new(1)?;

        // Extract the config from the TransportHandle
        let config = transport.config();
        let timeout_config = crate::timeout::TimeoutConfig::default();
        let retry_config = config.retry_config;

        // Create RuntimeHandle with the transport, executor, protocol style, and retry config
        let runtime_handle = crate::runtime::RuntimeHandle::new_with_style_timeout_and_retry(
            transport,
            std::sync::Arc::new(executor),
            protocol_style,
            timeout_config,
            retry_config,
        )
        .await?;

        // Use the from_runtime_handle method to create the camera
        Ok(Self::from_runtime_handle(
            camera_id,
            timeout_config,
            runtime_handle,
        ))
    }

    /// Connect to a camera with automatic protocol detection using a specific runtime.
    ///
    /// This method automatically detects whether the camera uses Sony encapsulated
    /// or raw VISCA format and creates a properly configured camera instance.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # #[cfg(feature = "rt-tokio")]
    /// use grafton_visca::{Camera, TokioRuntime};
    /// use grafton_visca::profiles::SonyFR7;
    ///
    /// # #[cfg(feature = "rt-tokio")]
    /// # #[tokio::main]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let runtime = TokioRuntime::from_current()?;
    /// let camera = Camera::<crate::mode::Async, SonyFR7, _, _>::connect_auto(
    ///     "192.168.0.110:5678",
    ///     runtime
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_auto(address: impl Into<String>, runtime: R) -> Result<Self, Error> {
        use crate::capabilities::ProtocolStyle;
        use crate::transport::protocol_detection::DetectionResult;

        let (transport, detection_result) =
            Transport::auto_detect(address, runtime.clone()).await?;

        // Map the detection result to protocol style
        let protocol_style = match detection_result {
            DetectionResult::SonyEncapsulated => ProtocolStyle::SonyEncapsulated,
            DetectionResult::RawVisca => ProtocolStyle::RawVisca,
            DetectionResult::NoResponse => {
                // This should not happen as auto_detect returns an error for NoResponse
                unreachable!("auto_detect returns Err for NoResponse")
            }
        };

        // Use the detected protocol style
        Self::new_async_with_runtime_transport(transport, runtime, protocol_style).await
    }

    /// Connect to a camera via TCP using a specific runtime.
    ///
    /// This method creates a TCP transport and camera instance with type-safe
    /// runtime pairing.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # #[cfg(feature = "rt-tokio")]
    /// use grafton_visca::{Camera, TokioRuntime};
    /// use grafton_visca::profiles::PtzOpticsG2;
    ///
    /// # #[cfg(feature = "rt-tokio")]
    /// # #[tokio::main]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let runtime = TokioRuntime::from_current()?;
    /// let camera = Camera::<crate::mode::Async, PtzOpticsG2, _, _>::connect_tcp(
    ///     "192.168.0.110:5678",
    ///     runtime
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_tcp(address: impl Into<String>, runtime: R) -> Result<Self, Error> {
        let transport = Transport::tcp()
            .address(address)
            .build_async_with(runtime.clone())
            .await?;

        Camera::new_async_runtime(transport, runtime).await
    }

    /// Connect to a camera via UDP using a specific runtime.
    ///
    /// This method creates a UDP transport and camera instance with type-safe
    /// runtime pairing.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # #[cfg(feature = "rt-tokio")]
    /// use grafton_visca::{Camera, TokioRuntime};
    /// use grafton_visca::profiles::GenericVisca;
    ///
    /// # #[cfg(feature = "rt-tokio")]
    /// # #[tokio::main]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let runtime = TokioRuntime::from_current()?;
    /// let camera = Camera::<crate::mode::Async, GenericVisca, _, _>::connect_udp(
    ///     "192.168.0.110:1259",
    ///     runtime
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect_udp(address: impl Into<String>, runtime: R) -> Result<Self, Error> {
        let transport = Transport::udp()
            .address(address)
            .build_async_with(runtime.clone())
            .await?;

        Camera::new_async_runtime(transport, runtime).await
    }
}
