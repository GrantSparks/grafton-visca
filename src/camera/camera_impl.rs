//! Camera implementation using Mode trait for async/blocking operations.
//!
//! This module provides a unified Camera<M, P, Tr, Exec> type that works with both
//! blocking and async operations through the Mode trait system.

use core::marker::PhantomData;
#[cfg(feature = "mode-async")]
use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    camera_id::CameraId,
    capabilities::Profile,
    command::{ResponseParser, ViscaCommand},
    error::Error,
    mode::Mode,
    timeout::TimeoutConfig,
};
#[cfg(feature = "mode-async")]
use crate::{executor::Executor, transport::AsyncTransport};
#[cfg(not(feature = "mode-async"))]
use crate::{runtime::blocking_runner::BlockingRunner, transport::BlockingTransport};

/// Camera client that works in both blocking and async modes.
///
/// This struct provides type-safe camera control that adapts to the chosen
/// execution mode through the Mode trait system. All mode-specific behavior
/// is resolved at compile time through the Mode type parameter.
///
/// # Type Parameters
///
/// * `M` - Mode type (`crate::mode::Async` or `crate::mode::Blocking`)
/// * `P` - Camera profile implementing the `Profile` trait
/// * `Tr` - Transport implementing either `AsyncTransport` or `BlockingTransport`
/// * `Exec` - Executor type (only used in async mode)
///
/// # Multi-Client Usage
///
/// For applications serving multiple clients (e.g., web servers, MCP servers),
/// **share a single `Camera` instance per physical camera**. The `Camera` type is
/// `Clone` and handles concurrent command access internally through its runtime.
///
/// PTZ cameras typically cannot reliably handle multiple concurrent TCP/UDP
/// connections, which causes inquiry timeouts and unpredictable behavior.
///
/// ```rust,ignore
/// // BAD: Multiple connections to same camera
/// let cam1 = Camera::open_tcp::<Profile>("192.168.0.10")?; // Client 1
/// let cam2 = Camera::open_tcp::<Profile>("192.168.0.10")?; // Client 2 - AVOID!
///
/// // GOOD: Share a single camera instance
/// let camera = Camera::open_tcp::<Profile>("192.168.0.10")?;
/// let cam1 = camera.clone(); // Client 1 - lightweight handle
/// let cam2 = camera.clone(); // Client 2 - same underlying connection
/// ```
///
/// See the [`runtime`](crate::runtime) module documentation for connection pooling
/// patterns.
///
/// # Examples
///
/// ```rust,ignore
/// use grafton_visca::{Camera, mode::{Async, Blocking}, camera::profiles::PtzOpticsG2};
/// use grafton_visca::transport::Transport;
///
/// // Async camera
/// let transport = Transport::tcp().address("192.168.0.110:5678").open_async().await?;
/// let camera = Camera::<Async, PtzOpticsG2, _, _>::new_async(transport, executor).await?;
/// camera.power_on().await?;
///
/// // Blocking camera
/// let transport = Transport::tcp().address("192.168.0.110:5678").build_blocking()?;
/// let mut camera = Camera::<Blocking, PtzOpticsG2, _, ()>::new_blocking(transport)?;
/// camera.power_on().await?; // .await works for both modes via Mode trait
/// ```
#[cfg(feature = "mode-async")]
pub struct Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    camera_id: CameraId,
    timeout_config: TimeoutConfig,

    // For async mode: stores runtime handle (transport and envelope managed by runtime)
    runtime: crate::runtime::RuntimeHandle<P, Exec>,

    // State cache for write-only properties (auto slow shutter, spotlight, pan/tilt limits)
    state_cache: crate::cache::StateCache,

    _phantom_mode: PhantomData<M>,
    _phantom_profile: PhantomData<P>,
    _phantom_transport: PhantomData<Tr>,
}

/// Camera client that works in both blocking and async modes.
///
/// This struct provides type-safe camera control that adapts to the chosen
/// execution mode through the Mode trait system. All mode-specific behavior
/// is resolved at compile time through the Mode type parameter.
///
/// # Type Parameters
///
/// * `M` - Mode type (`crate::mode::Async` or `crate::mode::Blocking`)
/// * `P` - Camera profile implementing the `Profile` trait
/// * `Tr` - Transport implementing either `AsyncTransport` or `BlockingTransport`
/// * `Exec` - Executor type (only used in async mode, `()` for blocking)
///
/// # Multi-Client Usage
///
/// For applications serving multiple clients (e.g., web servers, MCP servers),
/// **share a single `Camera` instance per physical camera**. The `Camera` type is
/// `Clone` and handles concurrent command access internally through its runtime.
///
/// PTZ cameras typically cannot reliably handle multiple concurrent TCP/UDP
/// connections, which causes inquiry timeouts and unpredictable behavior.
///
/// ```rust,ignore
/// // BAD: Multiple connections to same camera
/// let cam1 = Camera::open_tcp::<Profile>("192.168.0.10")?; // Client 1
/// let cam2 = Camera::open_tcp::<Profile>("192.168.0.10")?; // Client 2 - AVOID!
///
/// // GOOD: Share a single camera instance
/// let camera = Camera::open_tcp::<Profile>("192.168.0.10")?;
/// let cam1 = camera.clone(); // Client 1 - lightweight handle
/// let cam2 = camera.clone(); // Client 2 - same underlying connection
/// ```
///
/// See the [`runtime`](crate::runtime) module documentation for connection pooling
/// patterns.
#[cfg(not(feature = "mode-async"))]
pub struct Camera<M, P, Tr, Exec = ()>
where
    M: Mode,
    P: Profile,
{
    camera_id: CameraId,
    timeout_config: TimeoutConfig,

    // For blocking mode: stores transport directly with BlockingRunner for state management
    transport: M::Shared<Tr>,
    blocking_runner: std::cell::RefCell<BlockingRunner<P>>,

    // State cache for write-only properties (auto slow shutter, spotlight, pan/tilt limits)
    state_cache: crate::cache::StateCache,

    _phantom_mode: PhantomData<M>,
    _phantom_profile: PhantomData<P>,
    _phantom_transport: PhantomData<Tr>,
    _phantom_exec: PhantomData<Exec>,
}

#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> Camera<crate::mode::Async, P, Tr, Exec>
where
    P: Profile + Default,
    Tr: AsyncTransport + crate::transport::HasTransportConfig + Send + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    /// Create a new async camera instance using the profile's envelope type.
    ///
    /// This constructor uses default timeout and retry configurations.
    /// For custom configurations, use [`new_async_with_config`](Self::new_async_with_config).
    pub async fn new_async(transport: Tr, executor: impl Into<Arc<Exec>>) -> Result<Self, Error> {
        Self::new_async_with_config(
            transport,
            executor,
            TimeoutConfig::default(),
            crate::transport::RetryConfig::default(),
        )
        .await
    }

    /// Create a new async camera instance with explicit timeout and retry configuration.
    ///
    /// This constructor ensures that the provided configurations are used by the runtime,
    /// making them the single source of truth for command timeouts and retry behavior.
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport to use for communication
    /// * `executor` - The async executor to spawn tasks on
    /// * `timeout_config` - Configuration for command timeouts
    /// * `retry_config` - Configuration for command retries
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::{Camera, mode::Async, camera::profiles::PtzOpticsG2};
    /// use grafton_visca::timeout::TimeoutConfig;
    /// use grafton_visca::transport::RetryConfig;
    ///
    /// let timeout_config = TimeoutConfig::builder()
    ///     .quick_timeout(Duration::from_secs(10))
    ///     .build();
    /// let retry_config = RetryConfig::default();
    ///
    /// let camera = Camera::<Async, PtzOpticsG2, _, _>::new_async_with_config(
    ///     transport,
    ///     executor,
    ///     timeout_config,
    ///     retry_config,
    /// ).await?;
    /// ```
    pub async fn new_async_with_config(
        transport: Tr,
        executor: impl Into<Arc<Exec>>,
        timeout_config: TimeoutConfig,
        retry_config: crate::transport::RetryConfig,
    ) -> Result<Self, Error> {
        let camera_id = CameraId::new(1)?;
        let executor: Arc<Exec> = executor.into();

        // Create RuntimeHandle with the transport, executor, and explicit configs
        // The runtime handle uses the profile's envelope type
        let runtime_handle = crate::runtime::RuntimeHandle::new_with_config(
            transport,
            executor,
            Some(timeout_config),
            retry_config,
        )
        .await?;

        Ok(Self {
            camera_id,
            timeout_config,
            runtime: runtime_handle,
            state_cache: crate::cache::StateCache::new(),
            _phantom_mode: PhantomData,
            _phantom_profile: PhantomData,
            _phantom_transport: PhantomData,
        })
    }
}

#[cfg(not(feature = "mode-async"))]
impl<P, Tr> Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + Default,
    Tr: BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    /// Create a new blocking camera instance using the profile's protocol style.
    ///
    /// This constructor uses default timeout and retry configurations from the transport.
    /// For custom configurations, use [`new_blocking_with_config`](Self::new_blocking_with_config).
    pub fn new_blocking(transport: Tr) -> Result<Self, Error>
    where
        Tr: crate::transport::HasTransportConfig,
    {
        let timeout_config = TimeoutConfig::default();
        let retry_config = transport.transport_config().retry_config;
        Self::new_blocking_with_config(transport, timeout_config, retry_config)
    }

    /// Create a new blocking camera instance with explicit timeout and retry configuration.
    ///
    /// This constructor ensures that the provided configurations are used by the blocking runner,
    /// making them the single source of truth for command timeouts and retry behavior.
    ///
    /// # Arguments
    ///
    /// * `transport` - The transport to use for communication
    /// * `timeout_config` - Configuration for command timeouts
    /// * `retry_config` - Configuration for command retries
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::{Camera, mode::Blocking, camera::profiles::PtzOpticsG2};
    /// use grafton_visca::timeout::TimeoutConfig;
    /// use grafton_visca::transport::RetryConfig;
    ///
    /// let timeout_config = TimeoutConfig::builder()
    ///     .quick_timeout(Duration::from_secs(10))
    ///     .build();
    /// let retry_config = RetryConfig::default();
    ///
    /// let camera = Camera::<Blocking, PtzOpticsG2, _, _>::new_blocking_with_config(
    ///     transport,
    ///     timeout_config,
    ///     retry_config,
    /// )?;
    /// ```
    pub fn new_blocking_with_config(
        transport: Tr,
        timeout_config: TimeoutConfig,
        retry_config: crate::transport::RetryConfig,
    ) -> Result<Self, Error>
    where
        Tr: crate::transport::HasTransportConfig,
    {
        let camera_id = CameraId::new(1)?;

        // Get the transport's configuration for buffer and addressing settings
        let transport_config = transport.transport_config();
        let buffer_config = transport_config.buffer_config;
        let addressing = transport_config.addressing;

        // Create BlockingRunner with explicit timeout and retry configs,
        // but use transport's buffer and addressing settings
        let blocking_runner = BlockingRunner::<P>::builder(timeout_config)
            .retry_config(retry_config)
            .buffer_config(buffer_config)
            .addressing(addressing)
            .build();

        let shared_transport = crate::mode::Blocking::share(transport);

        Ok(Self {
            camera_id,
            timeout_config,
            transport: shared_transport,
            blocking_runner: std::cell::RefCell::new(blocking_runner),
            state_cache: crate::cache::StateCache::new(),
            _phantom_mode: PhantomData,
            _phantom_profile: PhantomData,
            _phantom_transport: PhantomData,
            _phantom_exec: PhantomData,
        })
    }
}

#[cfg(feature = "mode-async")]
impl<M, P, Tr, Exec> Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    /// Get the camera ID.
    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    /// Set the camera ID.
    pub fn set_camera_id(&mut self, camera_id: CameraId) {
        self.camera_id = camera_id;
    }

    /// Get the current timeout configuration.
    ///
    /// Note: For async cameras, timeout configuration is set at construction time
    /// via [`Camera::new_async_with_config`] and cannot be changed after creation.
    /// This ensures the runtime's timeout behavior matches the configuration.
    pub fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Get the camera's capabilities.
    ///
    /// Returns a structured representation of all camera capabilities extracted
    /// from the compile-time profile traits. This is cached after initialization
    /// for efficient access.
    ///
    /// # Example
    /// ```ignore
    /// let camera = Camera::open_tcp::<PtzOpticsG2>("192.168.0.10")?;
    /// let caps = camera.capabilities();
    ///
    /// println!("Model: {}", caps.model_name);
    /// println!("Pan range: {:?}°", caps.pan_range_degrees);
    /// println!("Zoom range: 0x{:04X}", caps.zoom_range_optical.end());
    ///
    /// if caps.has_digital_zoom {
    ///     println!("Digital zoom supported");
    /// }
    /// ```
    pub fn capabilities(&self) -> crate::capabilities::Capabilities {
        crate::capabilities::Capabilities::from_profile::<P>()
    }

    // Accessor methods for noun-based control trait access

    /// Access power-related controls and inquiries.
    pub fn power(&self) -> crate::camera::accessors::PowerAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PowerAccessor::new(self)
    }

    /// Access zoom-related controls and inquiries.
    pub fn zoom(&self) -> crate::camera::accessors::ZoomAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ZoomAccessor::new(self)
    }

    /// Access system-related controls and inquiries.
    pub fn system(&self) -> crate::camera::accessors::SystemAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::SystemAccessor::new(self)
    }

    /// Access pan/tilt-related controls and inquiries.
    pub fn pan_tilt(&self) -> crate::camera::accessors::PanTiltAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PanTiltAccessor::new(self)
    }

    /// Access focus-related controls and inquiries.
    pub fn focus(&self) -> crate::camera::accessors::FocusAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::FocusAccessor::new(self)
    }

    /// Access exposure-related controls and inquiries.
    pub fn exposure(&self) -> crate::camera::accessors::ExposureAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ExposureAccessor::new(self)
    }

    /// Access white balance controls and inquiries.
    pub fn white_balance(
        &self,
    ) -> crate::camera::accessors::WhiteBalanceAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::WhiteBalanceAccessor::new(self)
    }

    /// Access image processing controls and inquiries.
    pub fn image(&self) -> crate::camera::accessors::ImageAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ImageAccessor::new(self)
    }

    /// Access preset-related controls.
    pub fn presets(&self) -> crate::camera::accessors::PresetsAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PresetsAccessor::new(self)
    }

    /// Access tally light controls and inquiries.
    pub fn tally(&self) -> crate::camera::accessors::TallyAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::TallyAccessor::new(self)
    }

    /// Access the state cache for write-only properties.
    ///
    /// The state cache tracks values for properties that have setter commands
    /// but no corresponding VISCA inquiry. Values are automatically updated
    /// when setter commands succeed.
    ///
    /// # Tracked Properties
    ///
    /// - Auto slow shutter (on/off)
    /// - Spotlight mode (on/off)
    /// - Pan/tilt movement limits
    ///
    /// # Example
    ///
    /// ```ignore
    /// camera.enable_auto_slow_shutter().await?;
    /// assert_eq!(camera.state_cache().auto_slow_shutter(), Some(true));
    ///
    /// camera.pan_tilt_limit_set(PanTiltLimitCorner::UpRight, pan, tilt).await?;
    /// let limits = camera.state_cache().pan_tilt_limits();
    /// assert!(limits.up_right().is_some());
    /// ```
    pub fn state_cache(&self) -> &crate::StateCache {
        &self.state_cache
    }
}

#[cfg(not(feature = "mode-async"))]
impl<M, P, Tr, Exec> Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: crate::executor::Executor,
{
    /// Get the camera ID.
    pub fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    /// Set the camera ID.
    pub fn set_camera_id(&mut self, camera_id: CameraId) {
        self.camera_id = camera_id;
    }

    /// Get the current timeout configuration.
    pub fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Set the timeout configuration.
    pub fn set_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.timeout_config = timeout_config;
        // Also update the BlockingRunner's timeout config
        if let Ok(mut runner) = self.blocking_runner.try_borrow_mut() {
            runner.update_timeout_config(timeout_config);
        }
    }

    /// Get the camera's capabilities.
    ///
    /// Returns a structured representation of all camera capabilities extracted
    /// from the compile-time profile traits. This is cached after initialization
    /// for efficient access.
    ///
    /// # Example
    /// ```ignore
    /// let camera = Camera::open_tcp::<PtzOpticsG2>("192.168.0.10")?;
    /// let caps = camera.capabilities();
    ///
    /// println!("Model: {}", caps.model_name);
    /// println!("Pan range: {:?}°", caps.pan_range_degrees);
    /// println!("Zoom range: 0x{:04X}", caps.zoom_range_optical.end());
    ///
    /// if caps.has_digital_zoom {
    ///     println!("Digital zoom supported");
    /// }
    /// ```
    pub fn capabilities(&self) -> crate::capabilities::Capabilities {
        crate::capabilities::Capabilities::from_profile::<P>()
    }

    // Accessor methods for noun-based control trait access

    /// Access power-related controls and inquiries.
    pub fn power(&self) -> crate::camera::accessors::PowerAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PowerAccessor::new(self)
    }

    /// Access zoom-related controls and inquiries.
    pub fn zoom(&self) -> crate::camera::accessors::ZoomAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ZoomAccessor::new(self)
    }

    /// Access pan/tilt-related controls and inquiries.
    pub fn pan_tilt(&self) -> crate::camera::accessors::PanTiltAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PanTiltAccessor::new(self)
    }

    /// Access focus-related controls and inquiries.
    pub fn focus(&self) -> crate::camera::accessors::FocusAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::FocusAccessor::new(self)
    }

    /// Access exposure-related controls and inquiries.
    pub fn exposure(&self) -> crate::camera::accessors::ExposureAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ExposureAccessor::new(self)
    }

    /// Access white balance controls.
    pub fn white_balance(
        &self,
    ) -> crate::camera::accessors::WhiteBalanceAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::WhiteBalanceAccessor::new(self)
    }

    /// Access menu navigation controls.
    pub fn menu(&self) -> crate::camera::accessors::MenuAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::MenuAccessor::new(self)
    }

    /// Access preset controls.
    pub fn presets(&self) -> crate::camera::accessors::PresetsAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::PresetsAccessor::new(self)
    }

    /// Access tally light controls.
    pub fn tally(&self) -> crate::camera::accessors::TallyAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::TallyAccessor::new(self)
    }

    /// Access system-related controls and inquiries.
    pub fn system(&self) -> crate::camera::accessors::SystemAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::SystemAccessor::new(self)
    }

    /// Access image-related controls and inquiries.
    pub fn image(&self) -> crate::camera::accessors::ImageAccessor<'_, M, P, Tr, Exec> {
        crate::camera::accessors::ImageAccessor::new(self)
    }

    /// Access the state cache for write-only properties.
    ///
    /// The state cache tracks values for properties that have setter commands
    /// but no corresponding VISCA inquiry. Values are automatically updated
    /// when setter commands succeed.
    ///
    /// # Tracked Properties
    ///
    /// - Auto slow shutter (on/off)
    /// - Spotlight mode (on/off)
    /// - Pan/tilt movement limits
    ///
    /// # Example
    ///
    /// ```ignore
    /// camera.enable_auto_slow_shutter()?;
    /// assert_eq!(camera.state_cache().auto_slow_shutter(), Some(true));
    ///
    /// camera.pan_tilt_limit_set(PanTiltLimitCorner::UpRight, pan, tilt)?;
    /// let limits = camera.state_cache().pan_tilt_limits();
    /// assert!(limits.up_right().is_some());
    /// ```
    pub fn state_cache(&self) -> &crate::StateCache {
        &self.state_cache
    }
}

// Unified constructor methods for blocking mode with BlockingTransportHandle (zero-cost)
#[cfg(not(feature = "mode-async"))]
impl<P> Camera<crate::mode::Blocking, P, crate::transport::BlockingTransportHandle, ()>
where
    P: Profile + Default,
{
    /// Open a TCP connection to a camera.
    ///
    /// This is the primary convenience method for creating a blocking camera via TCP.
    /// It internally creates the transport and connects to the camera.
    ///
    /// If no port is specified in the address, the profile's DEFAULT_TCP_PORT will be used.
    /// IPv6 addresses are properly canonicalized (e.g., `2001:db8::1:5678` becomes `[2001:db8::1]:5678`).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::camera::Camera;
    /// use grafton_visca::camera::profiles::PtzOpticsG2;
    /// use grafton_visca::mode::BlockingFutureExt;
    ///
    /// let camera = Camera::open_tcp::<PtzOpticsG2>("192.168.0.110:5678")?;
    /// camera.power().on()?;  // Direct Result<(), Error> with BlockingClient
    /// camera.close()?;
    ///
    /// // IPv6 with explicit port (canonicalized automatically)
    /// let camera = Camera::open_tcp::<PtzOpticsG2>("[::1]:5678")?;
    /// ```
    pub fn open_tcp(addr: impl Into<String>) -> Result<Self, Error> {
        // Canonicalize the address (handles IPv6 bracketing and default port)
        let canonical_addr = crate::transport::address::canonicalize_endpoint(
            &addr.into(),
            Some(P::DEFAULT_TCP_PORT),
        )?;

        let tcp = crate::transport::blocking::tcp::Tcp::connect(&canonical_addr)?;
        let transport = crate::transport::BlockingTransportHandle::Tcp(tcp);
        Self::new_blocking(transport)
    }

    /// Open a UDP connection to a camera.
    ///
    /// This is the primary convenience method for creating a blocking camera via UDP.
    /// It internally creates the transport and connects to the camera.
    ///
    /// If no port is specified in the address, the profile's DEFAULT_UDP_PORT will be used.
    /// IPv6 addresses are properly canonicalized (e.g., `2001:db8::1:1259` becomes `[2001:db8::1]:1259`).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::camera::Camera;
    /// use grafton_visca::camera::profiles::GenericVisca;
    /// use grafton_visca::mode::BlockingFutureExt;
    ///
    /// let camera = Camera::open_udp::<GenericVisca>("192.168.0.110:1259")?;
    /// camera.power().on()?;  // Direct Result<(), Error> with BlockingClient
    /// camera.close()?;
    ///
    /// // IPv6 with explicit port (canonicalized automatically)
    /// let camera = Camera::open_udp::<GenericVisca>("[::1]:1259")?;
    /// ```
    pub fn open_udp(addr: impl Into<String>) -> Result<Self, Error> {
        // Canonicalize the address (handles IPv6 bracketing and default port)
        let canonical_addr = crate::transport::address::canonicalize_endpoint(
            &addr.into(),
            Some(P::DEFAULT_UDP_PORT),
        )?;

        let udp = crate::transport::blocking::udp::Udp::connect(&canonical_addr)?;
        let transport = crate::transport::BlockingTransportHandle::Udp(udp);
        Self::new_blocking(transport)
    }
}

// Methods specific to blocking cameras regardless of transport bounds
#[cfg(not(feature = "mode-async"))]
impl<P, Tr> Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile,
    Tr: BlockingTransport,
{
    /// Get access to the transport for blocking mode.
    #[cfg(not(feature = "mode-async"))]
    pub(crate) fn transport(&self) -> &std::cell::RefCell<Tr> {
        &self.transport
    }
}

// Methods for async mode requiring Send + Sync transports
#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> Camera<crate::mode::Async, P, Tr, Exec>
where
    P: Profile + Default,
    Tr: Send + 'static,
    Exec: Executor,
{
    /// Send a command using the mode-specific return type.
    ///
    /// This method delegates to the runtime handle for proper sequence tracking,
    /// socket management, and concurrency control.
    ///
    /// The command is encoded eagerly (before creating the async future), so
    /// the returned future captures only `Arc<EncodedCommand>` and doesn't
    /// require `Clone` on the command type.
    pub fn send_command<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Fut<'static, Result<crate::command::Response, Error>>
    where
        C: ViscaCommand,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        use crate::command::encode::EncodedCommand;
        use std::sync::Arc;

        let camera_id = self.camera_id;
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);

        // Eager preparation: encode the command before creating the future.
        // This eliminates the Clone requirement by capturing Arc<EncodedCommand>
        // instead of the command itself.
        let prepared = EncodedCommand::new(command, camera_id);
        let runtime = self.runtime.clone();

        Box::pin(async move {
            let prepared_command = Arc::new(prepared?);
            if is_inquiry {
                runtime
                    .send_inquiry_prepared(prepared_command, camera_id)
                    .await
            } else {
                runtime
                    .send_command_prepared(prepared_command, camera_id, None)
                    .await
            }
        })
    }

    /// Execute an arbitrary VISCA command and require a successful completion.
    ///
    /// This is the public escape hatch for custom or newly added command types
    /// that are not yet covered by a typed control trait. Prefer the camera
    /// control traits and noun accessors for built-in operations.
    pub fn execute<C>(
        &self,
        command: C,
    ) -> <crate::mode::Async as Mode>::Fut<'static, Result<(), Error>>
    where
        C: ViscaCommand,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let response = self.send_command(&command);
        Box::pin(async move { response.await?.into_result() })
    }

    /// Get a reference to the runtime handle.
    pub(crate) fn runtime(&self) -> &crate::runtime::RuntimeHandle<P, Exec> {
        &self.runtime
    }

    /// Send a typed command and return the response.
    ///
    /// Like `send_command`, this method encodes eagerly and doesn't require `Clone`.
    pub fn send_command_typed<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Fut<'static, Result<<C as ResponseParser>::Response, Error>>
    where
        C: ResponseParser + ViscaCommand,
        <C as ResponseParser>::Response: Send + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let response_future = self.send_command(command);
        Box::pin(async move {
            let response = response_future.await?;
            C::from_response(response)
        })
    }

    /// Send a command and return a command ID and response.
    ///
    /// This allows advanced users to track and potentially cancel commands.
    /// Most users should use the high-level trait methods instead.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InquiryNotCancelable`] if called with an inquiry command.
    /// Inquiries cannot be canceled and should use [`send_inquiry`](Self::send_command)
    /// instead.
    ///
    /// # Type Safety
    ///
    /// The returned [`CommandId`](crate::camera::inflight::CommandId) is guaranteed
    /// to be valid and non-zero. It can be used with [`cancel`](Self::cancel) to
    /// cancel the command.
    pub fn send_command_with_id<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Fut<
        'static,
        Result<(crate::camera::inflight::CommandId, crate::command::Response), Error>,
    >
    where
        C: ViscaCommand,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        use crate::command::encode::EncodedCommand;
        use std::sync::Arc;

        let camera_id = self.camera_id;
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);
        let prepared = EncodedCommand::new(command, camera_id);
        let runtime = self.runtime.clone();

        Box::pin(async move {
            if is_inquiry {
                // Inquiries cannot be canceled - reject with clear error
                return Err(Error::InquiryNotCancelable);
            }

            let prepared_command = Arc::new(prepared?);
            let (id, future) = runtime
                .send_command_with_id_prepared(prepared_command, camera_id, None)
                .await?;
            let response = future.await?;
            Ok((id, response))
        })
    }

    /// Send a command and return immediately with ID and response future.
    ///
    /// This method allows for mid-flight cancellation by returning the command ID
    /// immediately along with a future that can be awaited separately. This is useful
    /// for scenarios where you need to cancel a command while it's still in progress.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InquiryNotCancelable`] if called with an inquiry command.
    /// Inquiries cannot be canceled and should use [`send_inquiry`](Self::send_command)
    /// instead.
    ///
    /// # Example
    /// ```rust,ignore
    /// use grafton_visca::camera::inflight::CommandId;
    ///
    /// let (id, future) = camera.start_command_with_id(&cmd).await?;
    /// // Can cancel by ID here
    /// camera.cancel(id).await?;
    /// // Future will resolve with Err(CommandCanceled)
    /// let result = future.await;
    /// ```
    pub async fn start_command_with_id<C>(
        &self,
        command: &C,
    ) -> Result<
        (
            crate::camera::inflight::CommandId,
            Pin<Box<dyn Future<Output = Result<crate::command::Response, Error>> + Send + 'static>>,
        ),
        Error,
    >
    where
        C: ViscaCommand,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        use crate::command::encode::EncodedCommand;
        use std::sync::Arc;

        let camera_id = self.camera_id;
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);

        if is_inquiry {
            // Inquiries cannot be canceled - reject with clear error
            return Err(Error::InquiryNotCancelable);
        }

        // Eager preparation: encode before async operations
        let prepared_command = Arc::new(EncodedCommand::new(command, camera_id)?);

        // Return the ID and future directly without awaiting
        let (id, fut) = self
            .runtime
            .send_command_with_id_prepared(prepared_command, camera_id, None)
            .await?;
        let future = Box::pin(fut);
        Ok((id, future))
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This method sends a cancel command to the specified VISCA socket,
    /// addressed to this camera's configured camera ID.
    pub async fn cancel_socket(&self, socket: crate::ViscaSocket) -> Result<(), Error>
    where
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        self.runtime.cancel_socket(self.camera_id, socket).await
    }

    /// Cancel a command by its ID.
    ///
    /// This cancels a specific command that was submitted with `send_command_with_id`
    /// or `start_command_with_id`. The command's future will resolve with
    /// [`Error::CommandCanceled`]. The cancel command is addressed to this camera's
    /// configured camera ID.
    ///
    /// # Type Safety
    ///
    /// This method only accepts [`CommandId`](crate::camera::inflight::CommandId)
    /// values returned by the library, preventing the sentinel-value foot-gun where
    /// callers could pass invalid IDs that would never match any command.
    pub async fn cancel(&self, command_id: crate::camera::inflight::CommandId) -> Result<(), Error>
    where
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        self.runtime.cancel(self.camera_id, command_id).await
    }

    /// Sleep for a specified duration using the runtime.
    ///
    /// This provides runtime-agnostic sleeping functionality.
    pub async fn sleep(&self, duration: std::time::Duration)
    where
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync,
    {
        self.runtime.sleep(duration).await
    }
}

#[cfg(not(feature = "mode-async"))]
impl<P, Tr> Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: Profile + Default,
    Tr: BlockingTransport + crate::transport::HasTransportConfig + Send + 'static,
{
    /// Send a command using the mode-specific return type.
    ///
    /// The command is passed by reference and encoded once at the start.
    /// No `Clone` is required on the command type.
    pub fn send_command<C>(
        &self,
        command: &C,
    ) -> <crate::mode::Blocking as Mode>::Fut<'_, Result<crate::command::Response, Error>>
    where
        C: ViscaCommand,
    {
        // Always use BlockingRunner for both Sony and Raw VISCA protocols
        let transport_cell = self.transport();
        let mut transport = match transport_cell.try_borrow_mut() {
            Ok(transport) => transport,
            Err(_) => {
                return std::future::ready(Err(Error::TransportBusy));
            }
        };

        let mut runner = match self.blocking_runner.try_borrow_mut() {
            Ok(runner) => runner,
            Err(_) => {
                return std::future::ready(Err(Error::TransportBusy));
            }
        };

        // Send command through BlockingRunner (works for both protocols)
        // Category is derived from the command's TIMEOUT_CATEGORY constant via EncodedCommand
        match runner.send_command(&mut *transport, command, self.camera_id) {
            Ok(response) => std::future::ready(Ok(response)),
            Err(e) => std::future::ready(Err(e)),
        }
    }

    /// Execute an arbitrary VISCA command and require a successful completion.
    ///
    /// This is the public escape hatch for custom or newly added command types
    /// that are not yet covered by a typed control trait. Prefer the camera
    /// control traits and noun accessors for built-in operations.
    pub fn execute<C>(
        &self,
        command: C,
    ) -> <crate::mode::Blocking as Mode>::Fut<'_, Result<(), Error>>
    where
        C: ViscaCommand,
    {
        use crate::mode::BlockingFutureExt;

        let response = self.send_command(&command).block();
        std::future::ready(response.and_then(crate::command::Response::into_result))
    }

    /// Send a typed command and return the response.
    ///
    /// Like `send_command`, this method doesn't require `Clone`.
    pub fn send_command_typed<C>(
        &self,
        command: &C,
    ) -> <crate::mode::Blocking as Mode>::Fut<'_, Result<<C as ResponseParser>::Response, Error>>
    where
        C: ResponseParser + ViscaCommand,
        <C as ResponseParser>::Response: Send + 'static,
    {
        // In blocking mode, send_command executes synchronously and returns a Ready future.
        // We need to execute it, get the result, transform it, and wrap in a new Ready.
        let transport_cell = self.transport();
        let mut transport = match transport_cell.try_borrow_mut() {
            Ok(transport) => transport,
            Err(_) => {
                return std::future::ready(Err(Error::TransportBusy));
            }
        };

        let mut runner = match self.blocking_runner.try_borrow_mut() {
            Ok(runner) => runner,
            Err(_) => {
                return std::future::ready(Err(Error::TransportBusy));
            }
        };

        // Send command through BlockingRunner (works for both protocols)
        // Category is derived from the command's TIMEOUT_CATEGORY constant via EncodedCommand
        match runner.send_command(&mut *transport, command, self.camera_id) {
            Ok(response) => std::future::ready(C::from_response(response)),
            Err(e) => std::future::ready(Err(e)),
        }
    }

    /// Send a command with an external deadline constraint.
    ///
    /// This variant is used by movement detection to ensure individual
    /// inquiries don't exceed the overall operation timeout budget.
    ///
    /// Returns `Error::Timeout` immediately if the deadline has already passed,
    /// or if the deadline is exceeded while waiting for a response.
    pub fn send_command_with_deadline<C>(
        &self,
        command: &C,
        deadline: crate::timeout::Deadline,
    ) -> <crate::mode::Blocking as Mode>::Fut<'_, Result<crate::command::Response, Error>>
    where
        C: ViscaCommand,
    {
        let transport_cell = self.transport();
        let mut transport = match transport_cell.try_borrow_mut() {
            Ok(transport) => transport,
            Err(_) => {
                return std::future::ready(Err(Error::TransportBusy));
            }
        };

        let mut runner = match self.blocking_runner.try_borrow_mut() {
            Ok(runner) => runner,
            Err(_) => {
                return std::future::ready(Err(Error::TransportBusy));
            }
        };

        // Category is derived from the command's TIMEOUT_CATEGORY constant via EncodedCommand
        match runner.send_command_with_deadline(
            &mut *transport,
            command,
            self.camera_id,
            Some(deadline),
        ) {
            Ok(response) => std::future::ready(Ok(response)),
            Err(e) => std::future::ready(Err(e)),
        }
    }
}

// Close/shutdown methods for blocking mode
#[cfg(not(feature = "mode-async"))]
impl<P, Tr, Exec> Camera<crate::mode::Blocking, P, Tr, Exec>
where
    P: Profile,
    Tr: BlockingTransport,
{
    /// Close the camera connection gracefully.
    ///
    /// This method performs an orderly shutdown of the camera connection,
    /// ensuring any pending operations are completed before closing.
    /// The camera object is consumed and cannot be used after this call.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::{Camera, mode::Blocking, camera::profiles::PtzOpticsG2};
    ///
    /// let camera = Camera::<Blocking, PtzOpticsG2, _, ()>::connect_tcp("192.168.0.110:5678")?;
    /// // Use the camera...
    /// camera.close()?;
    /// // Camera is now closed and cannot be used
    /// ```
    pub fn close(self) -> Result<(), Error> {
        // For blocking mode, we don't have explicit close on transports
        // The transport will be closed when dropped
        // We just consume self to ensure no further operations
        drop(self);
        Ok(())
    }
}

// Close/shutdown methods for async mode
#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> Camera<crate::mode::Async, P, Tr, Exec>
where
    P: Profile,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    /// Shutdown the camera connection gracefully.
    ///
    /// This method performs an orderly shutdown of the camera connection,
    /// ensuring any pending operations are completed before closing.
    /// The camera object is consumed and cannot be used after this call.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::{Camera, mode::Async, camera::profiles::PtzOpticsG2};
    ///
    /// let camera = Camera::<Async, PtzOpticsG2, _, _>::connect_tcp("192.168.0.110:5678", runtime).await?;
    /// // Use the camera...
    /// camera.shutdown().await?;
    /// // Camera is now closed and cannot be used
    /// ```
    pub async fn shutdown(self) -> Result<(), Error> {
        // Shutdown the runtime handle which will close the transport
        self.runtime.shutdown().await;
        Ok(())
    }
}

#[cfg(feature = "mode-async")]
impl<M, P, Tr, Exec> core::fmt::Debug for Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
    Exec: Executor,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Camera")
            .field("camera_id", &self.camera_id)
            .field("timeout_config", &self.timeout_config)
            .finish()
    }
}

#[cfg(not(feature = "mode-async"))]
impl<M, P, Tr, Exec> core::fmt::Debug for Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: Profile,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Camera")
            .field("camera_id", &self.camera_id)
            .field("timeout_config", &self.timeout_config)
            .field("mode", &std::any::type_name::<M>())
            .field("profile", &std::any::type_name::<P>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "runtime-tokio")]
    use super::Camera;
    #[cfg(feature = "runtime-tokio")]
    use crate::{
        camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        error::Error,
    };

    #[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
    #[tokio::test]
    async fn test_async_camera_uses_profile_protocol_style() -> Result<(), Error> {
        use crate::{executor::TokioExecutor, testing::camera_simulator::ViscaCameraSimulator};

        let executor = TokioExecutor::from_current()?;

        // Test that SonyFR7 uses Sony encapsulated protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Async, SonyFR7, _, _>::new_async(transport, executor.clone())
                .await?;
        // The envelope should be configured with SonyEncapsulated protocol

        // Test that PtzOpticsG2 uses RawVisca protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Async, PtzOpticsG2, _, _>::new_async(transport, executor.clone())
                .await?;
        // The envelope should be configured with RawVisca protocol

        // Test that GenericVisca uses RawVisca protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Async, GenericVisca, _, _>::new_async(transport, executor)
                .await?;
        // The envelope should be configured with RawVisca protocol
        Ok(())
    }

    #[cfg(all(not(feature = "mode-async"), feature = "runtime-tokio"))]
    #[test]
    fn test_blocking_camera_uses_profile_protocol_style() {
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        // Test that SonyFR7 uses Sony encapsulated protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Blocking, SonyFR7, _, _>::new_blocking(transport).unwrap();
        // The envelope should be configured with SonyEncapsulated protocol

        // Test that PtzOpticsG2 uses RawVisca protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Blocking, PtzOpticsG2, _, _>::new_blocking(transport).unwrap();
        // The envelope should be configured with RawVisca protocol

        // Test that GenericVisca uses RawVisca protocol
        let transport = ViscaCameraSimulator::new();
        let _camera =
            Camera::<crate::mode::Blocking, GenericVisca, _, _>::new_blocking(transport).unwrap();
        // The envelope should be configured with RawVisca protocol
    }
}
