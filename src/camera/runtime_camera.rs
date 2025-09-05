//! Camera construction methods using the Runtime trait for type-safe pairing.

use crate::{
    camera::Camera,
    capabilities::Profile,
    runtime_trait::{Runtime, TransportHandle},
    transport::Transport,
    Error,
};

// Camera construction methods that use the Runtime trait for type-safe pairing
#[cfg(feature = "async")]
impl<P, R> Camera<crate::mode::Async, P, TransportHandle<R>, R>
where
    P: Profile + Default,
    R: Runtime,
{
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
        let (transport, _detection_result) =
            Transport::auto_detect(address, runtime.clone()).await?;

        // Use the detected protocol style from the camera profile
        Self::new_async(transport, runtime).await
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

        Self::new_async(transport, runtime).await
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

        Self::new_async(transport, runtime).await
    }
}
