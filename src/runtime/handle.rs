//! RuntimeHandle implementation for VISCA communication.

use flume::{Receiver, Sender};
use tracing::instrument;

use std::{
    future::Future,
    marker::PhantomData,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::{
    camera::inflight::CommandId,
    camera_id::CameraId,
    capabilities::Profile,
    command::{encode::EncodedCommand, response::Response},
    error::{Error, Result},
    executor::Executor,
    runtime::{
        async_adapter::{CompletionEvent, TxItem},
        core::Priority,
        loop_task::{runtime_loop_with_config, RuntimeLoopConfig},
    },
    timeout::TimeoutConfig,
    transport::{
        buffer::BufferManager, envelope::Envelope, AsyncTransport, HasTransportConfig, RetryConfig,
    },
    ViscaSocket,
};

#[cfg(feature = "test-utils")]
use crate::command::encode::ViscaCommand;
#[cfg(feature = "test-utils")]
use crate::runtime::async_adapter::MetricsSummary;

/// VISCA runtime handle.
///
/// This struct provides the main interface for communicating with a VISCA camera,
/// handling command submission, response processing, and protocol compliance.
/// Renamed from Camera to RuntimeHandle to avoid confusion with the main Camera type.
///
/// Note: This type is only available when the "async" feature is enabled,
/// as it requires async runtime support for communication.
#[derive(Debug)]
pub struct RuntimeHandle<P: Profile, E: Executor> {
    /// Inner shared state wrapped in Arc for safe cloning.
    inner: Arc<RuntimeHandleInner<P, E>>,
}

#[derive(Debug)]
struct RuntimeHandleInner<P: Profile, E: Executor> {
    /// Channel for submitting commands and inquiries.
    submit: Sender<TxItem>,
    /// Flag to track if runtime is shutdown.
    shutdown: Arc<AtomicBool>,
    /// Shutdown signal sender using flume for runtime-agnostic signaling.
    shutdown_tx: Sender<()>,
    /// Channel for requesting metrics from the runtime.
    #[cfg(feature = "test-utils")]
    metrics_tx: Sender<Sender<MetricsSummary>>,
    /// Channel for requesting completion event subscriptions from the runtime.
    completions_tx: Sender<Sender<Receiver<CompletionEvent>>>,
    /// Counter for generating unique command IDs.
    next_command_id: Arc<AtomicU32>,
    /// The executor used for sleep and timeout operations.
    executor: Arc<E>,
    /// Profile marker (zero-sized type).
    _profile: PhantomData<P>,
}

impl<P: Profile, E: Executor> Clone for RuntimeHandle<P, E> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<P: Profile + 'static, E: Executor + Send + Sync + 'static> RuntimeHandle<P, E> {
    /// Create a new camera runtime with the given transport.
    ///
    /// This spawns a background task to handle communication with the camera.
    #[cfg(feature = "test-utils")]
    pub async fn new<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: Arc<E>,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        Self::new_with_config(transport, executor, None, RetryConfig::default()).await
    }

    /// Create a new runtime handle with a transport and executor.
    ///
    /// This is an alias for `new` to match the expected API used by AsyncCamera.
    #[cfg(feature = "test-utils")]
    pub async fn spawn_with_transport<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: E,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        Self::new(transport, Arc::new(executor)).await
    }

    /// Create a new camera runtime with timeout config.
    ///
    /// This allows specifying custom timeouts.
    #[cfg(feature = "test-utils")]
    pub async fn new_with_timeout<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: Arc<E>,
        timeout_config: TimeoutConfig,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        Self::new_with_config(
            transport,
            executor,
            Some(timeout_config),
            RetryConfig::default(),
        )
        .await
    }

    /// Create a new camera runtime with configuration.
    ///
    /// # Arguments
    /// * `transport` - The transport to use for communication
    /// * `executor` - The async executor to spawn tasks on
    /// * `timeout_config` - Optional timeout configuration (defaults to TimeoutConfig::default())
    /// * `retry_config` - Retry configuration for the runtime
    #[instrument(
        level = "debug",
        skip(transport, executor, timeout_config, retry_config)
    )]
    pub async fn new_with_config<T: AsyncTransport + Send + 'static>(
        transport: T,
        executor: Arc<E>,
        timeout_config: Option<TimeoutConfig>,
        retry_config: RetryConfig,
    ) -> Result<Self>
    where
        for<'a> &'a T: HasTransportConfig,
    {
        // This is done before spawn to avoid lifetime issues
        let tcfg = *(&transport).transport_config();

        // Bound the submission channel to enforce backpressure at the handle→loop boundary.
        // This ensures `max_pending_queue_depth` is a real memory/backpressure guarantee,
        // preventing unbounded buffering before the adapter's admission control.
        let (submit_tx, submit_rx) = flume::bounded(tcfg.max_pending_queue_depth.get());

        // Keep other low-volume channels unbounded - they don't carry command traffic
        #[cfg(feature = "test-utils")]
        let (metrics_tx, metrics_rx) = flume::unbounded();
        #[cfg(not(feature = "test-utils"))]
        let metrics_rx = ();
        let (completions_tx, completions_rx) = flume::unbounded();
        let (shutdown_tx, shutdown_rx) = flume::unbounded();

        let envelope = P::Envelope::new(tcfg.addressing);
        let buffer_manager = BufferManager::new(tcfg.buffer_config);

        let timeout_config = timeout_config.unwrap_or_default();

        // Set max concurrent inquiries based on envelope's sequence correlation support.
        // Envelopes without sequence correlation (Raw VISCA) must serialize inquiries
        // to ensure responses can be reliably matched to requests.
        let max_concurrent_inquiries = if P::Envelope::SUPPORTS_SEQUENCE_CORRELATION {
            8 // Sony protocol: can handle multiple concurrent inquiries
        } else {
            1 // Raw VISCA: must serialize to ensure correct response matching
        };

        let config = RuntimeLoopConfig {
            envelope,
            buffer_manager,
            timeout_config,
            retry_config,
            write_timeout: tcfg.write_timeout,
            max_concurrent_inquiries,
            min_inquiry_spacing: P::MIN_INQUIRY_SPACING,
            min_command_spacing: P::MIN_COMMAND_SPACING,
            max_pending_queue_depth: tcfg.max_pending_queue_depth.get(),
        };

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
                #[cfg(feature = "test-utils")]
                metrics_tx,
                completions_tx,
                next_command_id: Arc::new(AtomicU32::new(1)),
                executor,
                _profile: PhantomData,
            }),
        })
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
            .map_err(Error::to_public_error)
    }

    /// Cancel a command by its ID.
    ///
    /// This method performs targeted, camera-correct cancellation:
    /// - For pending commands (not yet ACK'd): Removes from queue without sending VISCA cancel
    /// - For active commands (ACK'd on a socket): Sends VISCA cancel with correct camera ID
    /// - For unknown commands: Returns success (command may have already completed)
    ///
    /// The cancel command is addressed using the provided `camera_id`, ensuring
    /// correct multi-camera behavior. Cancellation is socket-scoped per VISCA semantics.
    ///
    /// # Arguments
    /// * `camera_id` - The camera address for the cancel message
    /// * `command_id` - The ID of the command to cancel (obtained from `send_command_with_id`
    ///   or `start_command_with_id`)
    ///
    /// # Returns
    /// `Ok(())` if the cancel request was processed (regardless of whether command was found)
    ///
    /// # Type Safety
    ///
    /// This method only accepts `CommandId` values returned by the library, preventing
    /// the sentinel-value foot-gun where callers could pass invalid IDs (like `0`)
    /// that would never match any command.
    pub async fn cancel(&self, camera_id: CameraId, command_id: CommandId) -> Result<()> {
        let cancel_item = TxItem::CancelById {
            camera_id,
            id: command_id,
        };

        self.inner
            .submit
            .send_async(cancel_item)
            .await
            .map_err(|_| Error::ChannelClosed)
            .map_err(Error::to_public_error)
    }

    /// Cancel all commands on a specific socket.
    ///
    /// This directly cancels the specified socket (S1 or S2) using the provided camera ID.
    /// The cancel command is addressed to the specified camera.
    ///
    /// # Arguments
    /// * `camera_id` - The camera address for the cancel message
    /// * `socket` - The VISCA socket to cancel (S1 or S2)
    ///
    /// # Returns
    /// Ok(()) if the cancel request was processed
    pub async fn cancel_socket(&self, camera_id: CameraId, socket: ViscaSocket) -> Result<()> {
        let cancel_item = TxItem::Cancel { camera_id, socket };

        self.inner
            .submit
            .send_async(cancel_item)
            .await
            .map_err(|_| Error::ChannelClosed)
            .map_err(Error::to_public_error)
    }

    /// Shutdown the runtime.
    pub async fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::Relaxed);
        let _ = self.inner.shutdown_tx.send_async(()).await;
    }

    /// Get current metrics from the runtime scheduler.
    ///
    /// Returns a snapshot of the current runtime metrics including queue depths,
    /// command counts, retry statistics, and more.
    #[cfg(feature = "test-utils")]
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
            .map_err(Error::to_public_error)
    }

    /// Subscribe to completion events from the runtime.
    ///
    /// Returns a receiver that will receive [`CompletionEvent`] notifications whenever
    /// a command completes. Used for event-driven movement detection.
    ///
    /// # Best-Effort Semantics
    ///
    /// Completion events are delivered on a **best-effort** basis. Each subscriber
    /// has a bounded buffer; if a subscriber cannot keep up with the event rate
    /// and its buffer becomes full, events will be dropped for that subscriber.
    /// This design ensures:
    ///
    /// - The runtime loop never blocks waiting for slow consumers
    /// - No unbounded memory growth from unread events
    /// - Subscribers that drain promptly receive all events
    ///
    /// If you need lossless event processing, ensure your consumer drains the
    /// receiver faster than events are produced.
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
            .map_err(Error::to_public_error)
    }

    /// Get a reference to the executor.
    ///
    /// This allows access to the executor for runtime-agnostic operations like sleep and timeout.
    pub fn executor(&self) -> &Arc<E> {
        &self.inner.executor
    }

    /// Sleep for a specified duration.
    ///
    /// This is a runtime-agnostic sleep that delegates to the injected executor.
    pub async fn sleep(&self, duration: Duration) {
        self.inner.executor.sleep(duration).await
    }

    /// Create a timeout future that will complete with an error if the given future
    /// doesn't complete within the specified duration.
    ///
    /// This delegates to the injected executor for consistent timeout behavior.
    pub async fn timeout<F, T>(&self, duration: Duration, fut: F) -> Result<T>
    where
        F: Future<Output = T> + Send,
        T: Send,
    {
        self.inner.executor.timeout(duration, fut).await
    }

    /// Send a VISCA command to the camera using the ViscaCommand trait.
    ///
    /// This method bridges the existing command system with the new runtime.
    ///
    /// # Arguments
    /// * `cmd` - A command implementing the ViscaCommand trait
    /// * `camera_id` - The camera ID to send the command to
    /// * `priority` - The priority level for the command (defaults to Normal)
    ///
    /// # Returns
    /// The response from the camera
    #[cfg(feature = "test-utils")]
    pub async fn send_command<C>(
        &self,
        cmd: &C,
        camera_id: CameraId,
        priority: Option<Priority>,
    ) -> Result<Response>
    where
        C: ViscaCommand,
    {
        let (_, response) = self.send_command_with_id(cmd, camera_id, priority).await?;
        response.await
    }

    /// Send a VISCA command to the camera and return a command ID and response future.
    ///
    /// This method allows canceling commands by their ID. Use this when you need
    /// to potentially cancel a command before it completes.
    ///
    /// # Arguments
    /// * `cmd` - A command implementing the ViscaCommand trait
    /// * `camera_id` - The camera ID to send the command to
    /// * `priority` - The priority level for the command (defaults to Normal)
    ///
    /// # Returns
    /// A tuple of (`CommandId`, response_future). The `CommandId` can be used with
    /// [`cancel`](Self::cancel) to cancel the command.
    ///
    /// # Note
    ///
    /// This method is for **commands only**, not inquiries. Inquiries complete
    /// immediately and cannot be canceled. Use [`send_inquiry`](Self::send_inquiry)
    /// for inquiry operations.
    #[cfg(feature = "test-utils")]
    pub async fn send_command_with_id<C>(
        &self,
        cmd: &C,
        camera_id: CameraId,
        priority: Option<Priority>,
    ) -> Result<(CommandId, impl Future<Output = Result<Response>>)>
    where
        C: ViscaCommand,
    {
        let prepared_command = Arc::new(EncodedCommand::new(cmd, camera_id).map_err(|e| {
            tracing::error!("Failed to prepare command: {e:?}");
            e
        })?);

        let command_id = self.allocate_command_id();

        let (response_tx, response_rx) = flume::bounded(1);

        let item = TxItem::Command {
            id: command_id,
            command: prepared_command.clone(),
            priority: priority.unwrap_or(Priority::Normal),
            camera_id,
            response_tx,
        };

        self.command(item).await?;

        let future = async move {
            response_rx
                .recv_async()
                .await
                .map_err(|_| Error::ChannelClosed)
                .map_err(Error::to_public_error)?
        };

        Ok((command_id, future))
    }

    /// Allocate a new unique command ID.
    ///
    /// This method ensures the returned ID is always non-zero by skipping
    /// zero on wraparound. IDs starting at 1 guarantees `NonZeroU32` invariant.
    fn allocate_command_id(&self) -> CommandId {
        loop {
            let id = self.inner.next_command_id.fetch_add(1, Ordering::Relaxed);
            // Skip zero on wraparound (when u32::MAX wraps to 0)
            if let Some(cmd_id) = CommandId::from_raw(id) {
                return cmd_id;
            }
            // id was 0, loop again to get the next value (1)
        }
    }

    /// Send a VISCA inquiry to the camera using the ViscaCommand trait.
    ///
    /// This method bridges the existing inquiry system with the new runtime.
    ///
    /// # Arguments
    /// * `inquiry` - An inquiry command implementing the ViscaCommand trait
    /// * `camera_id` - The camera ID to send the inquiry to
    ///
    /// # Returns
    /// The response from the camera
    ///
    /// # Note
    ///
    /// Inquiries are not cancelable and do not return a `CommandId`. They complete
    /// immediately without occupying a VISCA socket. For cancelable operations,
    /// use [`send_command_with_id`](Self::send_command_with_id).
    #[cfg(feature = "test-utils")]
    pub async fn send_inquiry<I>(&self, inquiry: &I, camera_id: CameraId) -> Result<Response>
    where
        I: ViscaCommand,
    {
        let prepared_command = Arc::new(EncodedCommand::new(inquiry, camera_id).map_err(|e| {
            tracing::error!("Failed to prepare inquiry: {e:?}");
            e
        })?);

        // Inquiries still need internal IDs for response correlation,
        // but these are not exposed externally since inquiries cannot be canceled.
        let inquiry_id = self.allocate_command_id();

        let (response_tx, response_rx) = flume::bounded(1);

        let item = TxItem::Inquiry {
            id: inquiry_id,
            command: prepared_command.clone(),
            camera_id,
            response_tx,
        };

        self.command(item).await?;

        response_rx
            .recv_async()
            .await
            .map_err(|_| Error::ChannelClosed)
            .map_err(Error::to_public_error)?
    }

    /// Send a pre-encoded command to the camera.
    ///
    /// This is an internal method used by Camera to submit already-encoded commands.
    /// The command is passed as an `Arc<EncodedCommand>` to avoid cloning.
    ///
    /// # Arguments
    /// * `prepared` - The pre-encoded command
    /// * `camera_id` - The camera ID to send the command to
    /// * `priority` - The priority level for the command (defaults to Normal)
    ///
    /// # Returns
    /// The response from the camera
    pub(crate) async fn send_command_prepared(
        &self,
        prepared: Arc<EncodedCommand>,
        camera_id: CameraId,
        priority: Option<Priority>,
    ) -> Result<Response> {
        let (_, response) = self
            .send_command_with_id_prepared(prepared, camera_id, priority)
            .await?;
        response.await
    }

    /// Send a pre-encoded inquiry to the camera.
    ///
    /// This is an internal method used by Camera to submit already-encoded inquiries.
    /// The inquiry is passed as an `Arc<EncodedCommand>` to avoid cloning.
    ///
    /// # Arguments
    /// * `prepared` - The pre-encoded inquiry command
    /// * `camera_id` - The camera ID to send the inquiry to
    ///
    /// # Returns
    /// The response from the camera
    pub(crate) async fn send_inquiry_prepared(
        &self,
        prepared: Arc<EncodedCommand>,
        camera_id: CameraId,
    ) -> Result<Response> {
        // Inquiries still need internal IDs for response correlation,
        // but these are not exposed externally since inquiries cannot be canceled.
        let inquiry_id = self.allocate_command_id();

        let (response_tx, response_rx) = flume::bounded(1);

        let item = TxItem::Inquiry {
            id: inquiry_id,
            command: prepared,
            camera_id,
            response_tx,
        };

        self.command(item).await?;

        response_rx
            .recv_async()
            .await
            .map_err(|_| Error::ChannelClosed)
            .map_err(Error::to_public_error)?
    }

    /// Send a pre-encoded command and return a command ID and response future.
    ///
    /// This is an internal method used by Camera to submit already-encoded commands
    /// while also returning the command ID for potential cancellation.
    ///
    /// # Arguments
    /// * `prepared` - The pre-encoded command
    /// * `camera_id` - The camera ID to send the command to
    /// * `priority` - The priority level for the command (defaults to Normal)
    ///
    /// # Returns
    /// A tuple of (`CommandId`, response_future)
    pub(crate) async fn send_command_with_id_prepared(
        &self,
        prepared: Arc<EncodedCommand>,
        camera_id: CameraId,
        priority: Option<Priority>,
    ) -> Result<(CommandId, impl Future<Output = Result<Response>>)> {
        let command_id = self.allocate_command_id();

        let (response_tx, response_rx) = flume::bounded(1);

        let item = TxItem::Command {
            id: command_id,
            command: prepared,
            priority: priority.unwrap_or(Priority::Normal),
            camera_id,
            response_tx,
        };

        self.command(item).await?;

        let future = async move {
            response_rx
                .recv_async()
                .await
                .map_err(|_| Error::ChannelClosed)
                .map_err(Error::to_public_error)?
        };

        Ok((command_id, future))
    }
}

// Helper function to spawn the runtime loop without trait bounds
// This avoids lifetime issues with HRTB (Rust issue #100013)
#[allow(clippy::too_many_arguments)]
fn spawn_runtime_loop<P, T, E>(
    executor: Arc<E>,
    transport: T,
    submit_rx: Receiver<TxItem>,
    metrics_rx: crate::runtime::loop_task::MetricsRequestReceiver,
    completions_rx: Receiver<Sender<Receiver<CompletionEvent>>>,
    shutdown_rx: Receiver<()>,
    task_executor: Arc<E>,
    config: RuntimeLoopConfig<P::Envelope>,
) where
    P: Profile + 'static,
    T: AsyncTransport + Send + 'static,
    E: Executor + Send + Sync + 'static,
{
    executor.spawn_bg(async move {
        match runtime_loop_with_config::<P, T, E>(
            transport,
            submit_rx,
            metrics_rx,
            completions_rx,
            shutdown_rx,
            task_executor,
            config,
        )
        .await
        {
            Ok(()) => tracing::debug!("Runtime loop exited cleanly"),
            Err(ref e) if matches!(e, Error::ConnectionClosed { .. }) => {
                tracing::error!("Runtime loop exited: {e} — all subsequent commands on this connection will fail");
            }
            Err(ref e) => {
                tracing::error!("Runtime loop exited unexpectedly: {e}");
            }
        }
    });
}

impl<P: Profile, E: Executor> Drop for RuntimeHandle<P, E> {
    fn drop(&mut self) {
        // Only send shutdown signal if this is the last reference
        if Arc::strong_count(&self.inner) == 1 {
            tracing::trace!("RuntimeHandle::drop -> last reference, sending shutdown");
            let _ = self.inner.shutdown_tx.send(());
            self.inner.shutdown.store(true, Ordering::Relaxed);
        }

        // Note: flume channels don't have a disconnect() method
        // The channels will be closed when all senders are dropped
        // The shutdown signal above is the primary mechanism for clean termination
    }
}
