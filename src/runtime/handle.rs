//! RuntimeHandle implementation for VISCA communication.

use flume::{Receiver, Sender};
use tracing::instrument;

use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

use crate::{
    capabilities::{Profile, ProtocolStyle},
    command::response::ViscaResponse,
    error::{Error, Result},
    runtime::{
        async_adapter::{CompletionEvent, MetricsSummary, TxItem},
        core::Priority,
        loop_task::{runtime_loop_with_config, RuntimeLoopConfig},
    },
    transport::{
        buffer::BufferManager, envelope::TransportEnvelope, AsyncTransport, HasTransportConfig,
    },
    ViscaSocket,
};

/// VISCA runtime handle.
///
/// This struct provides the main interface for communicating with a VISCA camera,
/// handling command submission, response processing, and protocol compliance.
/// Renamed from Camera to RuntimeHandle to avoid confusion with the main Camera type.
///
/// Note: This type is only available when the "async" feature is enabled,
/// as it requires async runtime support for communication.
#[derive(Debug)]
pub struct RuntimeHandle<P: Profile, E: crate::executor::Executor> {
    /// Inner shared state wrapped in Arc for safe cloning.
    inner: Arc<RuntimeHandleInner<P, E>>,
}

#[derive(Debug)]
struct RuntimeHandleInner<P: Profile, E: crate::executor::Executor> {
    /// Channel for submitting commands and inquiries.
    submit: Sender<TxItem>,
    /// Flag to track if runtime is shutdown.
    shutdown: Arc<AtomicBool>,
    /// Shutdown signal sender using flume for runtime-agnostic signaling.
    shutdown_tx: Sender<()>,
    /// Channel for requesting metrics from the runtime.
    metrics_tx: Sender<Sender<MetricsSummary>>,
    /// Channel for requesting completion event subscriptions from the runtime.
    completions_tx: Sender<Sender<Receiver<CompletionEvent>>>,
    /// Counter for generating unique command IDs.
    next_command_id: Arc<AtomicU32>,
    /// The executor used for sleep and timeout operations.
    executor: Arc<E>,
    /// Profile marker (zero-sized type).
    _profile: std::marker::PhantomData<P>,
}

impl<P: Profile, E: crate::executor::Executor> Clone for RuntimeHandle<P, E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<P: Profile + 'static, E: crate::executor::Executor + Send + Sync + 'static>
    RuntimeHandle<P, E>
{
    /// Create a new camera runtime with the given transport using raw VISCA protocol.
    ///
    /// This spawns a background task to handle communication with the camera.
    /// For compatibility, this defaults to raw VISCA protocol.
    pub async fn new<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: Arc<E>,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        Self::new_with_full_config(
            transport,
            executor,
            ProtocolStyle::RawVisca,
            None,
            crate::transport::RetryConfig::default(),
        )
        .await
    }

    /// Create a new runtime handle with a transport and executor.
    ///
    /// This is an alias for `new` to match the expected API used by AsyncCamera.
    pub async fn spawn_with_transport<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: E,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        Self::new(transport, Arc::new(executor)).await
    }

    /// Create a new camera runtime with explicit protocol style.
    ///
    /// This allows specifying whether to use raw VISCA or Sony encapsulated protocol.
    pub async fn new_with_style<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: Arc<E>,
        protocol_style: ProtocolStyle,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        Self::new_with_full_config(
            transport,
            executor,
            protocol_style,
            None,
            crate::transport::RetryConfig::default(),
        )
        .await
    }

    /// Create a new camera runtime with explicit protocol style and timeout config.
    ///
    /// This allows specifying both the protocol style and custom timeouts.
    pub async fn new_with_style_and_timeout<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: Arc<E>,
        protocol_style: ProtocolStyle,
        timeout_config: crate::timeout::TimeoutConfig,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        Self::new_with_full_config(
            transport,
            executor,
            protocol_style,
            Some(timeout_config),
            crate::transport::RetryConfig::default(),
        )
        .await
    }

    /// Create a new camera runtime with explicit protocol style, timeout config, and retry config.
    ///
    /// This allows specifying the protocol style, custom timeouts, and retry behavior.
    pub async fn new_with_style_timeout_and_retry<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: Arc<E>,
        protocol_style: ProtocolStyle,
        timeout_config: crate::timeout::TimeoutConfig,
        retry_config: crate::transport::RetryConfig,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        Self::new_with_full_config(
            transport,
            executor,
            protocol_style,
            Some(timeout_config),
            retry_config,
        )
        .await
    }

    /// Auto-detect the protocol style and create a new runtime.
    ///
    /// This probes the camera to determine whether it uses raw VISCA or Sony encapsulated protocol.
    pub async fn auto_detect<T: AsyncTransport + Send + 'static>(
        mut transport: T,
        executor: Arc<E>,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        use crate::transport::protocol_detection::{DetectionResult, ProtocolDetector};

        let detector = ProtocolDetector::new();
        let result = detector
            .detect_protocol(&mut transport, executor.as_ref())
            .await?;

        let protocol_style = match result {
            DetectionResult::SonyEncapsulated => ProtocolStyle::SonyEncapsulated,
            DetectionResult::RawVisca => ProtocolStyle::RawVisca,
            DetectionResult::NoResponse => {
                return Err(Error::TransportError(
                    "No response during protocol detection".into(),
                ));
            }
        };

        Self::new_with_style(transport, executor, protocol_style).await
    }

    /// Create a new camera runtime with full configuration.
    ///
    /// # Arguments
    /// * `transport` - The transport to use for communication
    /// * `executor` - The async executor to spawn tasks on
    /// * `protocol_style` - The protocol style to use (Raw VISCA or Sony encapsulated)
    /// * `timeout_config` - Optional timeout configuration (defaults to TimeoutConfig::default())
    /// * `retry_config` - Retry configuration for the runtime
    #[instrument(level = "debug", skip(transport, executor, timeout_config, retry_config), fields(protocol = ?protocol_style))]
    pub async fn new_with_full_config<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: Arc<E>,
        protocol_style: ProtocolStyle,
        timeout_config: Option<crate::timeout::TimeoutConfig>,
        retry_config: crate::transport::RetryConfig,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        let (submit_tx, submit_rx) = flume::unbounded();
        let (metrics_tx, metrics_rx) = flume::unbounded();
        let (completions_tx, completions_rx) = flume::unbounded();
        let (shutdown_tx, shutdown_rx) = flume::unbounded();

        // Extract config before creating the runtime config
        // This is done before spawn to avoid lifetime issues
        let tcfg = *(&transport).transport_config();

        // Create envelope and buffer manager for the runtime using transport's config
        let envelope = TransportEnvelope::new_with_addressing(protocol_style, tcfg.addressing);
        let buffer_manager = BufferManager::new(tcfg.buffer_config);

        // Use provided timeout config or default
        let timeout_config = timeout_config.unwrap_or_default();

        // Pre-bake a plain data config for the loop
        let config = RuntimeLoopConfig {
            envelope,
            buffer_manager,
            timeout_config,
            retry_config,
            write_timeout: tcfg.write_timeout,
        };

        // Clone executor for the runtime task
        let task_executor = Arc::clone(&executor);

        // Use a helper function to avoid lifetime issues with HRTB
        spawn_runtime_loop::<P, T, E>(
            Arc::clone(&executor),
            transport,
            submit_rx,
            metrics_rx,
            completions_rx,
            shutdown_rx,
            task_executor,
            config,
        );

        Ok(Self {
            inner: Arc::new(RuntimeHandleInner {
                submit: submit_tx,
                shutdown: Arc::new(AtomicBool::new(false)),
                shutdown_tx,
                metrics_tx,
                completions_tx,
                next_command_id: Arc::new(AtomicU32::new(1)),
                executor,
                _profile: std::marker::PhantomData,
            }),
        })
    }

    /// Create a new camera runtime with raw TCP transport (PtzOptics style).
    ///
    /// Note: This method requires the "rt-tokio" feature as it uses tokio-specific async transports.
    #[cfg(feature = "rt-tokio")]
    pub async fn new_tcp_raw(address: impl AsRef<str>, executor: Arc<E>) -> Result<Self> {
        // Use native tokio TCP transport for raw VISCA
        let transport = crate::transport::tokio::tcp::Tcp::connect(address.as_ref()).await?;
        Self::new(transport, executor).await
    }

    /// Create a new camera runtime with raw UDP transport (PtzOptics style).
    ///
    /// Note: This method requires the "rt-tokio" feature as it uses tokio-specific async transports.
    #[cfg(feature = "rt-tokio")]
    pub async fn new_udp_raw(address: impl AsRef<str>, executor: Arc<E>) -> Result<Self> {
        // Use native tokio UDP transport for raw VISCA
        let transport = crate::transport::tokio::udp::Udp::connect(address.as_ref()).await?;
        Self::new(transport, executor).await
    }

    /// Send a command item to the runtime.
    pub(crate) async fn command(&self, item: TxItem) -> Result<()> {
        if self.inner.shutdown.load(Ordering::Relaxed) {
            return Err(Error::RuntimeShutdown);
        }
        self.inner
            .submit
            .send_async(item)
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Cancel a command by its ID.
    ///
    /// This method performs targeted, camera-correct cancellation:
    /// - For pending commands (not yet ACK'd): Removes from queue without sending VISCA cancel
    /// - For active commands (ACK'd on a socket): Sends VISCA cancel with correct camera ID
    /// - For unknown commands: Returns success (command may have already completed)
    ///
    /// The cancel command uses the same camera ID as the original command, ensuring
    /// correct multi-camera behavior. Cancellation is socket-scoped per VISCA semantics.
    ///
    /// # Arguments
    /// * `command_id` - The ID of the command to cancel (obtained from send_command_with_id)
    ///
    /// # Returns
    /// Ok(()) if the cancel request was processed (regardless of whether command was found)
    pub async fn cancel(&self, command_id: u32) -> Result<()> {
        // Use the new CancelById variant for targeted cancellation
        let cancel_item = TxItem::CancelById { id: command_id };

        self.inner
            .submit
            .send_async(cancel_item)
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This directly cancels the specified socket (S1 or S2) without needing to know the command ID.
    /// Sends a VISCA cancel command using a hardcoded CAMERA_1 address since the socket-level
    /// cancel doesn't track which camera's command is currently active on the socket.
    ///
    /// For camera-correct cancellation, use `cancel(command_id)` instead.
    ///
    /// # Arguments
    /// * `socket` - The VISCA socket to cancel (S1 or S2)
    ///
    /// # Returns
    /// Ok(()) if the cancel request was processed
    pub async fn cancel_socket(&self, socket: ViscaSocket) -> Result<()> {
        let cancel_item = TxItem::Cancel { socket };

        self.inner
            .submit
            .send_async(cancel_item)
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Shutdown the runtime.
    pub async fn shutdown(&self) {
        // Set the shutdown flag
        self.inner.shutdown.store(true, Ordering::Relaxed);
        eprintln!("[RuntimeHandle] Sending shutdown signal");
        // Send shutdown signal to runtime loop
        let _ = self.inner.shutdown_tx.send_async(()).await;
        eprintln!("[RuntimeHandle] Shutdown signal sent");
    }

    /// Get current metrics from the runtime scheduler.
    ///
    /// Returns a snapshot of the current runtime metrics including queue depths,
    /// command counts, retry statistics, and more.
    pub async fn metrics(&self) -> Result<MetricsSummary> {
        let (response_tx, response_rx) = flume::bounded(1);
        self.inner
            .metrics_tx
            .send_async(response_tx)
            .await
            .map_err(|_| Error::ChannelClosed)?;
        response_rx
            .recv_async()
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Subscribe to completion events from the runtime.
    ///
    /// Returns a receiver that will receive CompletionEvent notifications whenever
    /// a command completes. Used for event-driven movement detection.
    pub async fn subscribe_completions(&self) -> Result<Receiver<CompletionEvent>> {
        let (response_tx, response_rx) = flume::bounded(1);
        self.inner
            .completions_tx
            .send_async(response_tx)
            .await
            .map_err(|_| Error::ChannelClosed)?;
        response_rx
            .recv_async()
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Sleep for a specified duration.
    ///
    /// This is a runtime-agnostic sleep that delegates to the injected executor.
    pub async fn sleep(&self, duration: std::time::Duration) {
        self.inner.executor.sleep(duration).await
    }

    /// Create a timeout future that will complete with an error if the given future
    /// doesn't complete within the specified duration.
    ///
    /// This delegates to the injected executor for consistent timeout behavior.
    pub async fn timeout<F, T>(&self, duration: std::time::Duration, fut: F) -> Result<T>
    where
        F: std::future::Future<Output = T> + Send,
        T: Send,
    {
        self.inner.executor.timeout(duration, fut).await
    }

    /// Send a VISCA command to the camera using the ViscaEncode trait.
    ///
    /// This method bridges the existing command system with the new runtime.
    ///
    /// # Arguments
    /// * `cmd` - A command implementing the ViscaEncode trait
    /// * `camera_id` - The camera ID to send the command to
    /// * `priority` - The priority level for the command (defaults to Normal)
    ///
    /// # Returns
    /// The response from the camera
    pub async fn send_command<C>(
        &self,
        cmd: &C,
        camera_id: crate::camera_id::CameraId,
        priority: Option<Priority>,
    ) -> Result<ViscaResponse>
    where
        C: crate::command::encode_visca::ViscaEncode + Clone + std::fmt::Debug + 'static,
    {
        let (_, response) = self.send_command_with_id(cmd, camera_id, priority).await?;
        response.await
    }

    /// Send a VISCA command to the camera and return a command ID and response future.
    ///
    /// This method allows canceling commands by their ID.
    ///
    /// # Arguments
    /// * `cmd` - A command implementing the ViscaEncode trait
    /// * `camera_id` - The camera ID to send the command to
    /// * `priority` - The priority level for the command (defaults to Normal)
    ///
    /// # Returns
    /// A tuple of (command_id, response_future)
    pub async fn send_command_with_id<C>(
        &self,
        cmd: &C,
        camera_id: crate::camera_id::CameraId,
        priority: Option<Priority>,
    ) -> Result<(
        u32,
        impl std::future::Future<Output = Result<ViscaResponse>>,
    )>
    where
        C: crate::command::encode_visca::ViscaEncode + Clone + std::fmt::Debug + 'static,
    {
        // Create pre-encoded command
        let prepared_command = Arc::new(
            crate::command::encode_visca::PreparedCommand::new(cmd.clone(), camera_id).map_err(
                |e| {
                    tracing::error!("Failed to prepare command: {:?}", e);
                    e
                },
            )?,
        );

        // Generate command ID
        let command_id = self.inner.next_command_id.fetch_add(1, Ordering::Relaxed);

        // Create response channel
        let (response_tx, response_rx) = flume::bounded(1);

        // Create the TxItem
        let item = TxItem::Command {
            id: command_id,
            command: prepared_command,
            priority: priority.unwrap_or(Priority::Normal),
            category: C::TIMEOUT_CATEGORY,
            camera_id,
            response_tx,
        };

        // Submit the command
        self.command(item).await?;

        // Return command ID and future
        let future = async move {
            response_rx
                .recv_async()
                .await
                .map_err(|_| Error::ChannelClosed)?
        };

        Ok((command_id, future))
    }

    /// Send a VISCA inquiry to the camera using the ViscaEncode trait.
    ///
    /// This method bridges the existing inquiry system with the new runtime.
    ///
    /// # Arguments
    /// * `inquiry` - An inquiry command implementing the ViscaEncode trait
    /// * `camera_id` - The camera ID to send the inquiry to
    ///
    /// # Returns
    /// The response from the camera
    pub async fn send_inquiry<I>(
        &self,
        inquiry: &I,
        camera_id: crate::camera_id::CameraId,
    ) -> Result<ViscaResponse>
    where
        I: crate::command::encode_visca::ViscaEncode + Clone + std::fmt::Debug + 'static,
    {
        // Create pre-encoded command
        let prepared_command = Arc::new(
            crate::command::encode_visca::PreparedCommand::new(inquiry.clone(), camera_id)
                .map_err(|e| {
                    tracing::error!("Failed to prepare inquiry: {:?}", e);
                    e
                })?,
        );

        // Generate inquiry ID
        let inquiry_id = self.inner.next_command_id.fetch_add(1, Ordering::Relaxed);

        // Create response channel
        let (response_tx, response_rx) = flume::bounded(1);

        // Create the TxItem
        let item = TxItem::Inquiry {
            id: inquiry_id,
            command: prepared_command,
            category: inquiry.timeout_kind(),
            camera_id,
            response_type: inquiry.response_type(),
            response_tx,
        };

        // Submit the inquiry (using command channel, not inquire)
        self.command(item).await?;

        // Wait for response
        response_rx
            .recv_async()
            .await
            .map_err(|_| Error::ChannelClosed)?
    }
}

// Helper function to spawn the runtime loop without trait bounds
// This avoids lifetime issues with HRTB (Rust issue #100013)
#[allow(clippy::too_many_arguments)] // This is an internal function with necessary parameters
fn spawn_runtime_loop<P, T, E>(
    executor: Arc<E>,
    transport: T,
    submit_rx: Receiver<TxItem>,
    metrics_rx: Receiver<Sender<MetricsSummary>>,
    completions_rx: Receiver<Sender<Receiver<CompletionEvent>>>,
    shutdown_rx: Receiver<()>,
    task_executor: Arc<E>,
    config: RuntimeLoopConfig,
) where
    P: Profile + 'static,
    T: AsyncTransport + Send + 'static,
    E: crate::executor::Executor + Send + Sync + 'static,
{
    executor.spawn_bg(async move {
        let _ = runtime_loop_with_config::<P, T, E>(
            transport,      // moved
            submit_rx,      // moved
            metrics_rx,     // moved
            completions_rx, // moved
            shutdown_rx,    // moved
            task_executor,  // moved Arc<E>
            config,         // plain data
        )
        .await;
    });
}

impl<P: Profile, E: crate::executor::Executor> Drop for RuntimeHandle<P, E> {
    fn drop(&mut self) {
        // Only send shutdown signal if this is the last reference
        if Arc::strong_count(&self.inner) == 1 {
            tracing::trace!("RuntimeHandle::drop -> last reference, sending shutdown");
            if std::env::var("RUNTIME_TRACE").is_ok() {
                eprintln!("[RuntimeHandle] Drop called on last reference, sending shutdown signal");
            }
            let _ = self.inner.shutdown_tx.send(());
            self.inner.shutdown.store(true, Ordering::Relaxed);
            if std::env::var("RUNTIME_TRACE").is_ok() {
                eprintln!("[RuntimeHandle] Shutdown signal sent via Drop");
            }
        } else if std::env::var("RUNTIME_TRACE").is_ok() {
            eprintln!(
                "[RuntimeHandle] Drop called but {} references remain, not shutting down",
                Arc::strong_count(&self.inner)
            );
        }

        // Note: flume channels don't have a disconnect() method
        // The channels will be closed when all senders are dropped
        // The shutdown signal above is the primary mechanism for clean termination
    }
}
