//! Camera implementation using Mode trait for async/blocking operations.
//!
//! This module provides a single Camera type that works with both blocking and async
//! operations through the Mode trait system, eliminating the need for separate
//! AsyncCamera and BlockingCamera types.

use core::marker::PhantomData;
#[cfg(feature = "mode-async")]
use std::{future::Future, pin::Pin, sync::Arc};

use crate::{
    camera_id::CameraId,
    capabilities::{Profile, ProtocolStyle},
    command::{typed::ResponseParser, ViscaCommand},
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

    _phantom_mode: PhantomData<M>,
    _phantom_profile: PhantomData<P>,
    _phantom_transport: PhantomData<Tr>,
}

/// Camera interface for VISCA protocol communication.
///
/// This type provides a uniform API for both blocking and async modes,
/// with runtime-agnostic execution through the Executor trait.
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
    /// Create a new async camera instance using the profile's protocol style.
    pub async fn new_async(transport: Tr, executor: impl Into<Arc<Exec>>) -> Result<Self, Error> {
        Self::new_async_with_style(transport, executor, P::PROTOCOL_STYLE).await
    }

    /// Create a new async camera instance with explicit protocol style.
    pub async fn new_async_with_style(
        transport: Tr,
        executor: impl Into<Arc<Exec>>,
        protocol_style: ProtocolStyle,
    ) -> Result<Self, Error> {
        let camera_id = CameraId::new(1)?;
        let timeout_config = TimeoutConfig::default();
        let executor: Arc<Exec> = executor.into();

        // Create RuntimeHandle with the transport, executor, and protocol style
        // The runtime handle encapsulates all transport, envelope, and buffer management
        let runtime_handle = crate::runtime::RuntimeHandle::new_with_style_and_timeout(
            transport,
            executor,
            protocol_style,
            timeout_config,
        )
        .await?;

        Ok(Self {
            camera_id,
            timeout_config,
            runtime: runtime_handle,
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
    pub fn new_blocking(transport: Tr) -> Result<Self, Error>
    where
        Tr: crate::transport::HasTransportConfig,
    {
        Self::new_blocking_with_style(transport, P::PROTOCOL_STYLE)
    }

    /// Create a new blocking camera instance with explicit protocol style.
    pub fn new_blocking_with_style(
        transport: Tr,
        protocol_style: ProtocolStyle,
    ) -> Result<Self, Error>
    where
        Tr: crate::transport::HasTransportConfig,
    {
        let camera_id = CameraId::new(1)?;
        let timeout_config = TimeoutConfig::default();

        // Get the transport's configuration
        let transport_config = transport.transport_config();
        let buffer_config = transport_config.buffer_config;
        let retry_config = transport_config.retry_config;
        let addressing = transport_config.addressing;

        // Create BlockingRunner with transport's configuration including addressing mode
        let blocking_runner = BlockingRunner::<P>::new_with_addressing(
            protocol_style,
            timeout_config,
            retry_config,
            buffer_config,
            addressing,
        );

        let shared_transport = crate::mode::Blocking::share(transport);

        Ok(Self {
            camera_id,
            timeout_config,
            transport: shared_transport,
            blocking_runner: std::cell::RefCell::new(blocking_runner),
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
    pub fn timeout_config(&self) -> &TimeoutConfig {
        &self.timeout_config
    }

    /// Set the timeout configuration.
    pub fn set_timeout_config(&mut self, timeout_config: TimeoutConfig) {
        self.timeout_config = timeout_config;
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
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::camera::Camera;
    /// use grafton_visca::camera::profiles::PtzOpticsG2;
    /// use grafton_visca::mode::BlockingFutureExt;
    ///
    /// let camera = Camera::open_tcp::<PtzOpticsG2>("192.168.0.110:5678")?;
    /// camera.power().on().block()?;
    /// camera.close()?;
    /// ```
    pub fn open_tcp(addr: impl Into<String>) -> Result<Self, Error> {
        let tcp = crate::transport::blocking::tcp::Tcp::connect(&addr.into())?;
        let transport = crate::transport::BlockingTransportHandle::Tcp(tcp);
        Self::new_blocking(transport)
    }

    /// Open a UDP connection to a camera.
    ///
    /// This is the primary convenience method for creating a blocking camera via UDP.
    /// It internally creates the transport and connects to the camera.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::camera::Camera;
    /// use grafton_visca::camera::profiles::GenericVisca;
    /// use grafton_visca::mode::BlockingFutureExt;
    ///
    /// let camera = Camera::open_udp::<GenericVisca>("192.168.0.110:1259")?;
    /// camera.power().on().block()?;
    /// camera.close()?;
    /// ```
    pub fn open_udp(addr: impl Into<String>) -> Result<Self, Error> {
        let udp = crate::transport::blocking::udp::Udp::connect(&addr.into())?;
        let transport = crate::transport::BlockingTransportHandle::Udp(udp);
        Self::new_blocking(transport)
    }

    /// Open a camera connection with automatic protocol detection.
    ///
    /// This method tries multiple transport/protocol combinations to find
    /// the one that the camera responds to:
    /// 1. UDP 52381 with Sony encapsulated (primary Sony path)
    /// 2. TCP 52381 with Sony encapsulated (some stacks support TCP)
    /// 3. UDP 1259 with raw VISCA (PTZOptics default)
    /// 4. TCP 5678 with raw VISCA (PTZOptics TCP)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use grafton_visca::camera::Camera;
    /// use grafton_visca::camera::profiles::PtzOpticsG2;
    /// use grafton_visca::mode::BlockingFutureExt;
    ///
    /// let camera = Camera::connect_auto::<PtzOpticsG2>("192.168.0.110")?;
    /// camera.power().on().block()?;
    /// ```
    pub fn connect_auto(host: impl Into<String>) -> Result<Self, Error> {
        let (transport, detected_style) =
            crate::transport::builder::auto_connect_and_detect_blocking(
                &host.into(),
                crate::transport::builder::TransportConfig::default(),
            )?;
        Self::new_blocking_with_style(transport, detected_style)
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
    pub fn send_command<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Fut<'static, Result<crate::command::response::Response, Error>>
    where
        C: ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let camera_id = self.camera_id;
        let command = command.clone();
        let runtime = self.runtime.clone();
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);

        Box::pin(async move {
            if is_inquiry {
                runtime.send_inquiry(&command, camera_id).await
            } else {
                runtime.send_command(&command, camera_id, None).await
            }
        })
    }

    /// Get a reference to the runtime handle (internal use).
    pub(crate) fn runtime(&self) -> &crate::runtime::RuntimeHandle<P, Exec> {
        &self.runtime
    }

    /// Send a typed command and return the response.
    pub fn send_command_typed<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Fut<'static, Result<<C as ResponseParser>::Response, Error>>
    where
        C: ResponseParser + ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
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

    /// Send a command and return a command ID and response future.
    ///
    /// This allows advanced users to track and potentially cancel commands.
    /// Most users should use the high-level trait methods instead.
    pub fn send_command_with_id<'a, C>(
        &'a self,
        command: &'a C,
    ) -> <crate::mode::Async as Mode>::Fut<
        'static,
        Result<(u32, crate::command::response::Response), Error>,
    >
    where
        C: ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let camera_id = self.camera_id;
        let command = command.clone();
        let runtime = self.runtime.clone();
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);

        Box::pin(async move {
            if is_inquiry {
                // Inquiries don't support command IDs in the current runtime
                // Just send and return ID 0 with response
                let response = runtime.send_inquiry(&command, camera_id).await?;
                Ok((0, response))
            } else {
                let (id, future) = runtime
                    .send_command_with_id(&command, camera_id, None)
                    .await?;
                let response = future.await?;
                Ok((id, response))
            }
        })
    }

    /// Send a command and return immediately with ID and response future.
    ///
    /// This method allows for mid-flight cancellation by returning the command ID
    /// immediately along with a future that can be awaited separately. This is useful
    /// for scenarios where you need to cancel a command while it's still in progress.
    ///
    /// # Example
    /// ```rust,ignore
    /// let (id, future) = camera.start_command_with_id(&cmd).await?;
    /// // Can cancel by ID here
    /// runtime.cancel(id).await?;
    /// // Future will resolve with Err(CommandCanceled)
    /// let result = future.await;
    /// ```
    pub async fn start_command_with_id<C>(
        &self,
        command: &C,
    ) -> Result<
        (
            u32,
            Pin<
                Box<
                    dyn Future<Output = Result<crate::command::response::Response, Error>>
                        + Send
                        + 'static,
                >,
            >,
        ),
        Error,
    >
    where
        C: ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        let camera_id = self.camera_id;
        let is_inquiry = matches!(command.command_kind(), crate::command::CommandKind::Inquiry);

        if is_inquiry {
            // Inquiries don't support command IDs in the current runtime
            // Return ID 0 with a future that immediately resolves
            let response = self.runtime.send_inquiry(command, camera_id).await?;
            let future = Box::pin(async move { Ok(response) });
            Ok((0, future))
        } else {
            // Return the ID and future directly without awaiting
            let (id, fut) = self
                .runtime
                .send_command_with_id(command, camera_id, None)
                .await?;
            let future = Box::pin(fut);
            Ok((id, future))
        }
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This method sends a cancel command to the specified VISCA socket.
    pub async fn cancel_socket(&self, socket: crate::ViscaSocket) -> Result<(), Error>
    where
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        self.runtime.cancel_socket(socket).await
    }

    /// Cancel a command by its ID.
    ///
    /// This cancels a specific command that was submitted with send_command_with_id
    /// or start_command_with_id. The command's future will resolve with CommandCanceled error.
    pub async fn cancel(&self, command_id: u32) -> Result<(), Error>
    where
        Tr: AsyncTransport + Send + Sync,
        Exec: Executor + Send + Sync + Clone,
    {
        self.runtime.cancel(command_id).await
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
    pub fn send_command<C>(
        &self,
        command: &C,
    ) -> <crate::mode::Blocking as Mode>::Fut<'_, Result<crate::command::response::Response, Error>>
    where
        C: ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
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

        // Determine command category
        let category = C::TIMEOUT_CATEGORY;

        // Send command through BlockingRunner (works for both protocols)
        match runner.send_command(&mut *transport, command, self.camera_id, category) {
            Ok(response) => std::future::ready(Ok(response)),
            Err(e) => std::future::ready(Err(e)),
        }
    }

    /// Send a typed command and return the response.
    pub fn send_command_typed<C>(
        &self,
        command: &C,
    ) -> <crate::mode::Blocking as Mode>::Fut<'_, Result<<C as ResponseParser>::Response, Error>>
    where
        C: ResponseParser + ViscaCommand + Send + Sync + Clone + std::fmt::Debug + 'static,
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

        // Determine command category
        let category = C::TIMEOUT_CATEGORY;

        // Send command through BlockingRunner (works for both protocols)
        match runner.send_command(&mut *transport, command, self.camera_id, category) {
            Ok(response) => std::future::ready(C::from_response(response)),
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
    use super::*;

    use crate::{
        camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        capabilities::ProfileMetadata,
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

    #[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
    #[tokio::test]
    async fn test_async_camera_with_explicit_protocol_style() -> Result<(), Error> {
        use crate::{executor::TokioExecutor, testing::camera_simulator::ViscaCameraSimulator};

        let executor = TokioExecutor::from_current()?;

        // Test overriding Sony profile to use RawVisca
        let transport = ViscaCameraSimulator::new();
        let _camera = Camera::<crate::mode::Async, SonyFR7, _, _>::new_async_with_style(
            transport,
            executor,
            ProtocolStyle::RawVisca,
        )
        .await?;
        // The envelope should be configured with RawVisca despite SonyFR7 default
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

    #[cfg(all(not(feature = "mode-async"), feature = "runtime-tokio"))]
    #[test]
    fn test_blocking_camera_with_explicit_protocol_style() {
        use crate::testing::camera_simulator::ViscaCameraSimulator;

        // Test overriding Sony profile to use RawVisca
        let transport = ViscaCameraSimulator::new();
        let _camera = Camera::<crate::mode::Blocking, SonyFR7, _, _>::new_blocking_with_style(
            transport,
            ProtocolStyle::RawVisca,
        )
        .unwrap();
        // The envelope should be configured with RawVisca despite SonyFR7 default
    }

    #[test]
    fn test_profile_protocol_style_constants() {
        // Verify that profiles declare the expected protocol styles
        assert_eq!(SonyFR7::PROTOCOL_STYLE, ProtocolStyle::SonyEncapsulated);
        assert_eq!(PtzOpticsG2::PROTOCOL_STYLE, ProtocolStyle::RawVisca);
        assert_eq!(GenericVisca::PROTOCOL_STYLE, ProtocolStyle::RawVisca);
    }
}
