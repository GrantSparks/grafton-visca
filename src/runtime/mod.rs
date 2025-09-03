//! VISCA runtime implementation using flume channels.
//!
//! This module provides the core runtime for VISCA communication,
//! managing command scheduling, socket allocation, and protocol timing.

pub mod scheduler;

#[cfg(feature = "async")]
use flume::{Receiver, Sender};
#[cfg(feature = "async")]
use futures_lite;
#[cfg(feature = "async")]
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
#[cfg(feature = "async")]
use tracing::{debug, error, instrument, trace, warn};

#[cfg(feature = "async")]
use crate::{
    capabilities::ProtocolStyle,
    command::response::ViscaResponse,
    error::{Error, Result},
    timeout::TimeoutConfig,
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
        AsyncTransport,
    },
    ViscaSocket,
};
#[cfg(feature = "async")]
use scheduler::{Scheduler, SchedulerMetrics, TxItem};

pub use scheduler::{MetricsSummary, Priority};

/// Helper function to spawn runtime tasks properly for different executor types.
#[cfg(feature = "async")]
#[instrument(level = "debug", skip(executor, runtime_task))]
fn spawn_runtime_task_properly<E: crate::executor::Executor>(
    executor: &E,
    runtime_task: impl std::future::Future<Output = Result<(), Error>> + Send + 'static,
) {
    // Spawn the runtime loop as a background task
    tracing::debug!("Spawning runtime task using spawn_bg");
    executor.spawn_bg(runtime_task);
}

/// VISCA runtime handle.
///
/// This struct provides the main interface for communicating with a VISCA camera,
/// handling command submission, response processing, and protocol compliance.
/// Renamed from Camera to RuntimeHandle to avoid confusion with the main Camera type.
///
/// Note: This type is only available when the "async" feature is enabled,
/// as it requires async runtime support for communication.
#[cfg(feature = "async")]
#[derive(Debug, Clone)]
pub struct RuntimeHandle {
    /// Channel for submitting commands and inquiries.
    submit: Sender<TxItem>,
    /// Flag to track if runtime is shutdown.
    shutdown: Arc<AtomicBool>,
    /// Channel for requesting metrics from the runtime.
    metrics_tx: Sender<Sender<MetricsSummary>>,
    /// Counter for generating unique command IDs.
    next_command_id: Arc<AtomicU32>,
}

#[cfg(feature = "async")]
impl RuntimeHandle {
    /// Create a new camera runtime with the given transport using raw VISCA protocol.
    ///
    /// This spawns a background task to handle communication with the camera.
    /// For compatibility, this defaults to raw VISCA protocol.
    #[cfg(feature = "async")]
    pub async fn new<T: AsyncTransport + Send + 'static, E: crate::executor::Executor>(
        transport: T,
        executor: Arc<E>,
    ) -> Result<Self> {
        Self::new_with_style(transport, executor, ProtocolStyle::RawVisca).await
    }

    /// Create a new runtime handle with a transport and executor.
    ///
    /// This is an alias for `new` to match the expected API used by AsyncCamera.
    #[cfg(feature = "async")]
    pub async fn spawn_with_transport<
        T: AsyncTransport + Send + 'static,
        E: crate::executor::Executor,
    >(
        transport: T,
        executor: E,
    ) -> Result<Self> {
        Self::new(transport, Arc::new(executor)).await
    }

    /// Create a new camera runtime with explicit protocol style.
    ///
    /// This allows specifying whether to use raw VISCA or Sony encapsulated protocol.
    #[cfg(feature = "async")]
    pub async fn new_with_style<
        T: AsyncTransport + Send + 'static,
        E: crate::executor::Executor,
    >(
        transport: T,
        executor: Arc<E>,
        protocol_style: ProtocolStyle,
    ) -> Result<Self> {
        Self::with_tick_interval_and_style(transport, executor, None, protocol_style).await
    }

    /// Auto-detect the protocol style and create a new runtime.
    ///
    /// This probes the camera to determine whether it uses raw VISCA or Sony encapsulated protocol.
    #[cfg(feature = "async")]
    pub async fn auto_detect<T: AsyncTransport + Send + 'static, E: crate::executor::Executor>(
        mut transport: T,
        executor: Arc<E>,
    ) -> Result<Self> {
        use crate::transport::protocol_detection::{DetectionResult, ProtocolDetector};

        let detector = ProtocolDetector::new();
        let result = detector
            .detect_protocol(&mut transport, executor.as_ref())
            .await?;

        let protocol_style = match result {
            DetectionResult::SonyEncapsulated => {
                ProtocolStyle::SonyEncapsulated { use_sequence: true }
            }
            DetectionResult::RawVisca => ProtocolStyle::RawVisca,
            DetectionResult::NoResponse => {
                return Err(Error::TransportError(
                    "No response during protocol detection".into(),
                ));
            }
        };

        Self::new_with_style(transport, executor, protocol_style).await
    }

    /// Create a new camera runtime with a custom tick interval.
    ///
    /// The tick interval controls how often the runtime checks for timeouts
    /// and processes retries. Default is 50ms.
    /// For compatibility, this defaults to raw VISCA protocol.
    ///
    /// # Arguments
    /// * `transport` - The transport to use for communication
    /// * `executor` - The async executor to spawn tasks on
    /// * `tick_interval_ms` - Optional tick interval in milliseconds (default: 50ms)
    #[cfg(feature = "async")]
    #[instrument(level = "debug", skip(transport, executor), fields(tick_ms = tick_interval_ms))]
    pub async fn with_tick_interval<
        T: AsyncTransport + Send + 'static,
        E: crate::executor::Executor,
    >(
        transport: T,
        executor: Arc<E>,
        tick_interval_ms: Option<u64>,
    ) -> Result<Self> {
        Self::with_tick_interval_and_style(
            transport,
            executor,
            tick_interval_ms,
            ProtocolStyle::RawVisca,
        )
        .await
    }

    /// Create a new camera runtime with a custom tick interval and protocol style.
    ///
    /// # Arguments
    /// * `transport` - The transport to use for communication
    /// * `executor` - The async executor to spawn tasks on
    /// * `tick_interval_ms` - Optional tick interval in milliseconds (default: 50ms)
    /// * `protocol_style` - The protocol style to use (Raw VISCA or Sony encapsulated)
    #[cfg(feature = "async")]
    #[instrument(level = "debug", skip(transport, executor), fields(tick_ms = tick_interval_ms, protocol = ?protocol_style))]
    pub async fn with_tick_interval_and_style<
        T: AsyncTransport + Send + 'static,
        E: crate::executor::Executor,
    >(
        transport: T,
        executor: Arc<E>,
        tick_interval_ms: Option<u64>,
        protocol_style: ProtocolStyle,
    ) -> Result<Self> {
        let (submit_tx, submit_rx) = flume::unbounded();
        let (metrics_tx, metrics_rx) = flume::unbounded();

        // Create envelope and buffer manager for the runtime
        let envelope = TransportEnvelope::new(protocol_style);
        let buffer_config = BufferConfig::default();
        let buffer_manager = BufferManager::new(buffer_config);

        // Spawn the runtime task with configured tick interval and envelope
        let runtime_task = runtime_loop_with_config(
            transport,
            submit_rx,
            metrics_rx,
            tick_interval_ms,
            Arc::clone(&executor),
            envelope,
            buffer_manager,
        );

        // Spawn the task, with special handling for deterministic executors in test mode
        debug!("[RuntimeHandle::with_tick_interval_and_style] About to spawn runtime task");
        spawn_runtime_task_properly(executor.as_ref(), runtime_task);
        debug!("[RuntimeHandle::with_tick_interval_and_style] Runtime task spawned");

        Ok(Self {
            submit: submit_tx,
            shutdown: Arc::new(AtomicBool::new(false)),
            metrics_tx,
            next_command_id: Arc::new(AtomicU32::new(1)),
        })
    }

    /// Create a new camera runtime with raw TCP transport (PtzOptics style).
    ///
    /// Note: This method requires the "rt-tokio" feature as it uses tokio-specific async transports.
    #[cfg(feature = "rt-tokio")]
    pub async fn new_tcp_raw<E: crate::executor::Executor>(
        address: impl AsRef<str>,
        executor: Arc<E>,
    ) -> Result<Self> {
        // Use native tokio TCP transport for raw VISCA
        let transport = crate::transport::tokio::tcp::Tcp::connect(address.as_ref()).await?;
        Self::new(transport, executor).await
    }

    /// Create a new camera runtime with raw UDP transport (PtzOptics style).
    ///
    /// Note: This method requires the "rt-tokio" feature as it uses tokio-specific async transports.
    #[cfg(feature = "rt-tokio")]
    pub async fn new_udp_raw<E: crate::executor::Executor>(
        address: impl AsRef<str>,
        executor: Arc<E>,
    ) -> Result<Self> {
        // Use native tokio UDP transport for raw VISCA
        let transport = crate::transport::tokio::udp::Udp::connect(address.as_ref()).await?;
        Self::new(transport, executor).await
    }

    /// Create a new camera runtime with serial transport.
    ///
    /// Note: This method is available when using tokio runtime for async serial I/O.
    #[cfg(all(feature = "async", feature = "rt-tokio", feature = "tokio-serial"))]
    pub async fn new_serial<E: crate::executor::Executor>(
        port: impl AsRef<str>,
        camera_address: u8,
        executor: Arc<E>,
    ) -> Result<Self> {
        let config = crate::transport::serial_async::AsyncSerialConfig {
            port: port.as_ref().to_string(),
            camera_address,
            ..Default::default()
        };
        let transport = crate::transport::serial_async::AsyncSerialTransport::new(config).await?;
        Self::new(transport, executor).await
    }

    /// Send a command item to the runtime.
    pub(crate) async fn command(&self, item: TxItem) -> Result<()> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Err(Error::RuntimeShutdown);
        }
        self.submit
            .send_async(item)
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Send an inquiry item to the runtime.
    pub(crate) async fn inquire(&self, item: TxItem) -> Result<()> {
        self.submit
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

        self.submit
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

        self.submit
            .send_async(cancel_item)
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Shutdown the runtime.
    pub async fn shutdown(&self) {
        // Set the shutdown flag
        self.shutdown.store(true, Ordering::Relaxed);
        // Drop the submit channel to signal shutdown
        // This will cause the runtime loop to exit
        // Note: dropping a clone doesn't close the channel
        // We need to ensure no more sends can happen
    }

    /// Get current metrics from the runtime scheduler.
    ///
    /// Returns a snapshot of the current runtime metrics including queue depths,
    /// command counts, retry statistics, and more.
    pub async fn metrics(&self) -> Result<MetricsSummary> {
        let (response_tx, response_rx) = flume::bounded(1);
        self.metrics_tx
            .send_async(response_tx)
            .await
            .map_err(|_| Error::ChannelClosed)?;
        response_rx
            .recv_async()
            .await
            .map_err(|_| Error::ChannelClosed)
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
        C: crate::command::encode_visca::ViscaEncode,
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
        C: crate::command::encode_visca::ViscaEncode,
    {
        // Encode the command
        let mut buffer = vec![0u8; C::MAX_SIZE];
        let len = cmd.encode_into(camera_id, &mut buffer)?;
        buffer.truncate(len);

        // Generate command ID
        let command_id = self.next_command_id.fetch_add(1, Ordering::Relaxed);

        // Create response channel
        let (response_tx, response_rx) = flume::bounded(1);

        // Create the TxItem
        let item = TxItem::Command {
            id: command_id,
            bytes: buffer,
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
        I: crate::command::encode_visca::ViscaEncode,
    {
        // Encode the inquiry
        let mut buffer = vec![0u8; I::MAX_SIZE];
        let len = inquiry.encode_into(camera_id, &mut buffer)?;
        buffer.truncate(len);

        // Create response channel
        let (response_tx, response_rx) = flume::bounded(1);

        // Get the expected response type from the inquiry
        let response_type = inquiry.response_type();

        // Create the TxItem
        let item = TxItem::Inquiry {
            id: 0, // Will be assigned by scheduler
            bytes: buffer,
            camera_id,
            response_type,
            response_tx,
        };

        // Submit the inquiry
        self.inquire(item).await?;

        // Wait for response
        response_rx
            .recv_async()
            .await
            .map_err(|_| Error::ChannelClosed)?
    }
}

/// Main runtime loop with configurable tick interval.
#[cfg(feature = "async")]
#[instrument(level = "debug", name = "visca_runtime_loop", skip(transport, submit_rx, metrics_rx, executor, envelope, buffer_manager), fields(tick_ms = tick_interval_ms))]
async fn runtime_loop_with_config<
    T: AsyncTransport + Send + 'static,
    E: crate::executor::Executor,
>(
    mut transport: T,
    submit_rx: Receiver<TxItem>,
    metrics_rx: Receiver<Sender<MetricsSummary>>,
    tick_interval_ms: Option<u64>,
    executor: Arc<E>,
    envelope: TransportEnvelope,
    buffer_manager: BufferManager,
) -> Result<()> {
    let mut scheduler = Scheduler::with_timeout_config(submit_rx.clone(), TimeoutConfig::default());
    let mut response_buffer = Vec::new();
    let mut consecutive_retries = 0usize;

    debug!("[runtime_loop_with_config] VISCA runtime started");

    // Create a timer interval for periodic checks
    let tick_ms = tick_interval_ms.unwrap_or(50);
    let tick_duration = std::time::Duration::from_millis(tick_ms);

    loop {
        // Use select! style approach with explicit enum
        enum Operation {
            Recv(Result<bytes::Bytes, Error>),
            Tick,
        }

        // Check for submit items non-blockingly first
        if let Ok(item) = submit_rx.try_recv() {
            match handle_tx_item(
                &mut transport,
                &mut scheduler,
                item,
                executor.as_ref(),
                &envelope,
                &buffer_manager,
            )
            .await
            {
                Ok(_) => {
                    // Successfully handled a TX item - reset consecutive retries
                    consecutive_retries = 0;

                    // Process command queue with retry budget check
                    let allow_retry_defer = consecutive_retries < 8;
                    if let Err(e) = process_command_queue(
                        &mut transport,
                        &mut scheduler,
                        executor.as_ref(),
                        allow_retry_defer,
                        &envelope,
                        &buffer_manager,
                    )
                    .await
                    {
                        error!("Error processing command queue after TX item: {e}");
                    }
                }
                Err(e) => {
                    error!("Error handling TX item: {e}");
                }
            }
            continue;
        }

        // Check for metrics requests non-blockingly
        if let Ok(response_tx) = metrics_rx.try_recv() {
            let summary = scheduler.metrics.summary();
            let _ = response_tx.send(summary);
            continue;
        }

        // Pre-drain any retries that are due now to avoid race conditions
        // This ensures deterministic behavior when retry deadline == now
        let now = executor.as_ref().now();
        if scheduler.can_send_command() {
            while let Some(deadline) = scheduler.next_retry_deadline() {
                if deadline > now {
                    break; // No more retries due now
                }

                // Get the next retry that's due now
                if let Some(retry_cmd) = scheduler.get_next_retry(now) {
                    debug!(
                        "Pre-draining retry for command {id} (attempt {attempt})",
                        id = retry_cmd.id,
                        attempt = retry_cmd.attempt
                    );

                    // Re-submit the command for retry
                    let response_tx = if let Some(tx) =
                        scheduler.peek_response_channel(retry_cmd.id)
                    {
                        tx.clone()
                    } else {
                        warn!(
                            "Missing response channel for retry of command {id} - this indicates a bug",
                            id = retry_cmd.id
                        );
                        let (tx, _rx) = flume::bounded(1);
                        tx
                    };

                    // Create TxItem for the retry
                    let tx_item = TxItem::Command {
                        id: retry_cmd.id,
                        bytes: retry_cmd.bytes.clone(),
                        priority: retry_cmd.priority,
                        category: retry_cmd.category,
                        camera_id: retry_cmd.camera_id,
                        response_tx,
                    };

                    // Send the retry immediately
                    if let Err(e) = handle_tx_item(
                        &mut transport,
                        &mut scheduler,
                        tx_item,
                        executor.as_ref(),
                        &envelope,
                        &buffer_manager,
                    )
                    .await
                    {
                        error!("Error handling retry TX item: {e}");
                    }

                    // If can't send more commands, stop draining
                    if !scheduler.can_send_command() {
                        break;
                    }
                } else {
                    break; // No retry ready (shouldn't happen since we checked deadline)
                }
            }
        }

        // Dynamic tick scheduling: sleep until the earliest retry deadline or housekeeping tick
        let now = executor.as_ref().now();
        let sleep_dur = if let Some(deadline) = scheduler.next_retry_deadline() {
            // Wake exactly when a retry becomes eligible (but never later than the housekeeping tick)
            let until_retry = deadline.saturating_duration_since(now);
            std::cmp::min(until_retry, tick_duration)
        } else {
            tick_duration
        };

        // Use select to handle both recv and tick operations
        let operation = {
            use futures_lite::future;

            future::or(async { Operation::Recv(transport.recv().await) }, async {
                executor.sleep(sleep_dur).await;
                Operation::Tick
            })
            .await
        };

        match operation {
            Operation::Recv(recv_result) => {
                // Handle received data
                match recv_result {
                    Ok(bytes) => {
                        trace!("Received bytes from transport: {bytes:02X?}");
                        response_buffer.extend_from_slice(&bytes);

                        // Parse complete frames from the buffer
                        let (frames, remaining) =
                            crate::protocol::response::parse_frames(&response_buffer);
                        response_buffer = remaining;

                        for frame in frames {
                            // Extract the VISCA payload from the frame before parsing
                            let payload = match envelope.extract_response(&frame) {
                                Ok(p) => p,
                                Err(e) => {
                                    warn!("Failed to extract response from frame: {e}");
                                    continue;
                                }
                            };

                            if let Err(e) = handle_response(
                                &mut transport,
                                &mut scheduler,
                                &payload,
                                executor.as_ref(),
                                &envelope,
                                &buffer_manager,
                            )
                            .await
                            {
                                error!("Error handling response: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving from transport: {e}");
                    }
                }
            }
            Operation::Tick => {
                // Handle tick - check for timeouts and retries
                let now = executor.as_ref().now();

                // Check for commands that have timed out waiting for ACK
                let pending_ack_timeouts = scheduler.check_pending_ack_timeouts(now);
                for cmd_id in pending_ack_timeouts {
                    debug!("Command {cmd_id} timed out waiting for ACK");
                    // Notify the waiting high-level caller
                    if let Some(tx) = scheduler.get_response_channel(cmd_id) {
                        let _ = tx.send(Err(Error::Timeout));
                    }
                }

                // Check for commands in sockets that have timed out
                let timed_out = scheduler.check_timeouts(now);
                for (socket, cmd_id) in timed_out {
                    // First, notify the waiting high-level caller
                    if let Some(tx) = scheduler.get_response_channel(cmd_id) {
                        let _ = tx.send(Err(Error::Timeout));
                    }

                    // Finally, free the socket & clean up metadata
                    scheduler.free_socket(socket);
                }

                // Then check if we have retries to process
                if scheduler.has_retries() {
                    debug!(
                        "[runtime loop] Has {retry_count} retries pending, free socket: {has_free}, can_send: {can_send}, retry queue: {retry_queue:?}",
                        retry_count = scheduler.retry_queue.len(),
                        has_free = scheduler.has_free_socket(),
                        can_send = scheduler.can_send_command(),
                        retry_queue = scheduler
                            .retry_queue
                            .iter()
                            .map(|r| r.id)
                            .collect::<Vec<_>>()
                    );
                }
                if scheduler.can_send_command() && scheduler.has_retries() {
                    debug!("[runtime loop] Has free socket and retries to process");
                    // get_next_retry() now handles exhausted retries internally
                    let now = executor.as_ref().now();
                    if let Some(retry_cmd) = scheduler.get_next_retry(now) {
                        debug!(
                            "[runtime loop] Retrying command {id} (attempt {attempt})",
                            id = retry_cmd.id,
                            attempt = retry_cmd.attempt
                        );
                        debug!(
                            "Retrying command {id} (attempt {attempt})",
                            id = retry_cmd.id,
                            attempt = retry_cmd.attempt
                        );

                        // Re-submit the command for retry
                        let response_tx = if let Some(tx) =
                            scheduler.peek_response_channel(retry_cmd.id)
                        {
                            tx.clone()
                        } else {
                            warn!(
                                "Missing response channel for retry of command {id} - this indicates a bug",
                                id = retry_cmd.id
                            );
                            continue;
                        };
                        debug!(
                            "Using existing response channel for retry of command {id}",
                            id = retry_cmd.id
                        );

                        let item = TxItem::Command {
                            id: retry_cmd.id,
                            bytes: retry_cmd.bytes.clone(),
                            priority: retry_cmd.priority,
                            category: retry_cmd.category,
                            camera_id: retry_cmd.camera_id,
                            response_tx,
                        };

                        if let Err(e) = handle_tx_item(
                            &mut transport,
                            &mut scheduler,
                            item,
                            executor.as_ref(),
                            &envelope,
                            &buffer_manager,
                        )
                        .await
                        {
                            error!("Error retrying command {id}: {e}", id = retry_cmd.id);
                        } else {
                            // Successfully submitted a retry
                            consecutive_retries = consecutive_retries.saturating_add(1);
                        }
                    }
                }
            }
        }

        // Check if submit channel is closed and scheduler is idle for shutdown
        let disconnected = submit_rx.is_disconnected();
        let idle = scheduler.is_idle();
        if disconnected && idle {
            debug!(
                "[runtime loop] Submit channel closed and scheduler is idle, shutting down runtime"
            );
            break;
        }
    }

    // The loop above never exits normally
    #[allow(unreachable_code)]
    {
        debug!("VISCA runtime stopped");
    }
    Ok(())
}

/// Process queued commands when a socket becomes available.
#[cfg(feature = "async")]
#[instrument(
    level = "trace",
    skip(transport, scheduler, executor, envelope, buffer_manager),
    fields(allow_retry_defer)
)]
async fn process_command_queue<T: AsyncTransport + Send, E: crate::executor::Executor>(
    transport: &mut T,
    scheduler: &mut Scheduler,
    executor: &E,
    allow_retry_defer: bool,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
) -> Result<()> {
    // Process commands from the priority queue while we can send more
    // But check if there's a higher priority retry ready first
    while scheduler.can_send_command() && !scheduler.is_queue_empty() {
        // Check if there's a retry ready that has higher or equal priority than the next queued command
        let now = executor.now();
        if allow_retry_defer {
            if let Some(next_queue_priority) = scheduler.peek_queue_priority() {
                if let Some(retry_priority) = scheduler.peek_ready_retry_priority(now) {
                    // If retry has higher or equal priority, don't process queue yet
                    if retry_priority >= next_queue_priority {
                        debug!(
                            "Deferring queue processing - retry with priority {retry_priority:?} waiting (queue has {queue_priority:?})",
                            retry_priority = retry_priority, queue_priority = next_queue_priority
                        );
                        return Ok(());
                    }
                }
            }
        }

        if let Some(item) = scheduler.dequeue_command() {
            debug!(
                "Processing queued command from priority queue (remaining: {remaining})",
                remaining = scheduler.queue_size()
            );
            // Process the dequeued command
            if let Err(e) = handle_tx_item(
                transport,
                scheduler,
                item,
                executor,
                envelope,
                buffer_manager,
            )
            .await
            {
                error!("Error processing queued command: {e}");
            }
        }
    }
    Ok(())
}

/// Handle a submitted TX item.
#[cfg(feature = "async")]
#[instrument(level = "trace", skip(transport, scheduler, executor, envelope, buffer_manager), fields(item = ?item))]
async fn handle_tx_item<T: AsyncTransport + Send, E: crate::executor::Executor>(
    transport: &mut T,
    scheduler: &mut Scheduler,
    item: TxItem,
    executor: &E,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
) -> Result<()> {
    match item {
        TxItem::Command {
            mut id,
            bytes,
            priority,
            category,
            camera_id,
            response_tx,
        } => {
            // Assign ID if not set
            if id == 0 {
                id = scheduler.next_id();
            }

            trace!("Processing command {id} with priority {priority:?}");

            // Track metrics for command submission
            scheduler
                .metrics
                .commands_submitted
                .fetch_add(1, Ordering::Relaxed);
            let idx = SchedulerMetrics::priority_index(priority);
            scheduler.metrics.priority_counts[idx].fetch_add(1, Ordering::Relaxed);

            // Check if we can send immediately (not at 2-command limit)
            // NOTE: We don't allocate socket yet - camera assigns it in ACK
            let now = executor.now();
            if scheduler.can_send_command() {
                // Enforce command spacing
                scheduler.enforce_spacing_with(executor, now).await;

                // Frame and send command
                let framed_bytes = envelope.frame_command(&bytes, false, buffer_manager);
                debug!(
                    "[handle_tx_item] Sending command {id} (awaiting ACK): {bytes:02X?} (framed: {framed_bytes:02X?})"
                );
                trace!("Sending command {id} (awaiting ACK): {bytes:02X?}");
                if let Err(e) = transport.send(&framed_bytes).await {
                    error!("Failed to send command {id}: {e}");
                    scheduler
                        .metrics
                        .commands_failed
                        .fetch_add(1, Ordering::Relaxed);
                    let _ = response_tx.send(Err(Error::TransportError(e.to_string().into())));
                    return Ok(());
                }

                // Add to pending ACK list - socket will be assigned when ACK arrives
                scheduler.add_pending_ack(id, bytes.clone(), priority, category, now, camera_id);
                scheduler.store_command_channel(id, response_tx);
                debug!("[handle_tx_item] Command {id} added to pending ACK list");
            } else {
                // No socket available, add to priority queue
                debug!(
                    "No socket available for command {id}, adding to queue with priority {priority:?}"
                );

                // Store the response channel for when the command is eventually sent
                scheduler.store_command_channel(id, response_tx.clone());

                // Enqueue the command for later processing
                let now = executor.now();
                scheduler.enqueue_command(
                    TxItem::Command {
                        id,
                        bytes,
                        priority,
                        category,
                        camera_id,
                        response_tx,
                    },
                    now,
                );
            }
        }

        TxItem::Inquiry {
            mut id,
            bytes,
            camera_id: _,
            response_type,
            response_tx,
        } => {
            // Assign ID if not set
            if id == 0 {
                id = scheduler.next_id();
            }

            trace!("Processing inquiry {id}");

            // Track metrics for inquiry submission
            scheduler
                .metrics
                .inquiries_submitted
                .fetch_add(1, Ordering::Relaxed);

            // Inquiries don't need sockets
            let now = executor.now();
            scheduler.enforce_spacing_with(executor, now).await;

            // Frame and send inquiry
            let framed_bytes = envelope.frame_command(&bytes, true, buffer_manager);
            trace!("Sending inquiry {id}: {bytes:02X?} (framed: {framed_bytes:02X?})");
            if let Err(e) = transport.send(&framed_bytes).await {
                error!("Failed to send inquiry {id}: {e}");
                scheduler
                    .metrics
                    .commands_failed
                    .fetch_add(1, Ordering::Relaxed);
                let _ = response_tx.send(Err(Error::TransportError(e.to_string().into())));
                return Ok(());
            }

            // Store the response channel in the scheduler for later use
            scheduler.store_pending_inquiry(id, response_tx, response_type);
        }

        TxItem::Cancel { socket } => {
            trace!("Processing cancel for {socket:?}");

            // Send cancel command using typed command
            use crate::camera_id::CameraId;
            use crate::command::encode_visca::ViscaEncode;
            use crate::command::system::CommandCancelCommand;

            let cancel_cmd = CommandCancelCommand::new(socket);
            let mut cancel_bytes = vec![0u8; 16];
            // CommandCancelCommand is const-constructed and guaranteed to encode
            let len = cancel_cmd
                .encode_into(CameraId::CAMERA_1, &mut cancel_bytes)
                .map_err(|e| {
                    Error::TransportError(format!("Failed to encode cancel command: {e}").into())
                })?;
            let cancel_bytes = cancel_bytes[..len].to_vec();

            // Frame the cancel command as a regular command (not an inquiry)
            let framed_cancel = envelope.frame_command(&cancel_bytes, false, buffer_manager);
            if let Err(e) = transport.send(&framed_cancel).await {
                error!("Failed to send cancel: {e}");
                return Ok(());
            }

            // Free the socket and notify
            if let Some(_cmd_id) = scheduler.socket_command(socket) {
                scheduler.free_socket(socket);
            }
        }

        TxItem::CancelById { id } => {
            trace!("Processing cancel by ID for command {id}");

            use crate::camera_id::CameraId;
            use crate::command::encode_visca::ViscaEncode;
            use crate::command::system::CommandCancelCommand;

            // Check if command is in pending_ack - if so, just remove it without sending cancel
            if scheduler.is_pending_ack(id) {
                // Command hasn't been ACK'd yet - no socket assigned, no cancel to send
                scheduler.remove_pending_ack(id);
                if let Some(response_tx) = scheduler.get_response_channel(id) {
                    let _ = response_tx.send(Err(Error::CommandCanceled));
                }
                scheduler.remove_command_metadata(id);
                debug!("Canceled command {id} that was pending ACK");
                return Ok(());
            }

            // Check if command has been assigned a socket
            let mut found_socket = None;
            let mut found_camera_id = CameraId::CAMERA_1; // Default fallback

            for socket in [ViscaSocket::S1, ViscaSocket::S2] {
                if scheduler.socket_command(socket) == Some(id) {
                    found_socket = Some(socket);
                    // Get camera ID from metadata
                    if let Some((_, _, _, camera_id)) = scheduler.get_command_metadata(id) {
                        found_camera_id = *camera_id;
                    }
                    break;
                }
            }

            if let Some(socket) = found_socket {
                // Build cancel command with the correct camera ID
                let cancel_cmd = CommandCancelCommand::new(socket);
                let mut cancel_bytes = vec![0u8; 16];
                let len = cancel_cmd
                    .encode_into(found_camera_id, &mut cancel_bytes)
                    .map_err(|e| {
                        Error::TransportError(
                            format!("Failed to encode cancel command: {e}").into(),
                        )
                    })?;
                let cancel_bytes = cancel_bytes[..len].to_vec();

                // Frame and send the cancel command
                let framed_cancel = envelope.frame_command(&cancel_bytes, false, buffer_manager);
                if let Err(e) = transport.send(&framed_cancel).await {
                    error!("Failed to send cancel for command {id}: {e}");
                    return Ok(());
                }

                // Free the socket and notify
                scheduler.free_socket(socket);
                if let Some(response_tx) = scheduler.get_response_channel(id) {
                    let _ = response_tx.send(Err(Error::CommandCanceled));
                }
                scheduler.remove_command_metadata(id);
                debug!("Canceled command {id} on {socket:?} using camera {found_camera_id}");
            } else {
                // Command not found - might have already completed
                debug!("Command {id} not found for cancellation");
                if let Some(response_tx) = scheduler.get_response_channel(id) {
                    let _ =
                        response_tx.send(Err(Error::InvalidRequest("Command not found".into())));
                }
            }
        }
    }

    Ok(())
}

/// Handle a VISCA response frame.
#[cfg(feature = "async")]
#[instrument(
    level = "trace",
    skip(transport, scheduler, frame, executor, envelope, buffer_manager)
)]
async fn handle_response<T: AsyncTransport + Send, E: crate::executor::Executor>(
    transport: &mut T,
    scheduler: &mut Scheduler,
    frame: &[u8],
    executor: &E,
    envelope: &TransportEnvelope,
    buffer_manager: &BufferManager,
) -> Result<()> {
    use crate::protocol::response::{decode_basic, lift_inquiry, BasicKind};
    use crate::runtime::scheduler::ViscaError;

    let basic_response = match decode_basic(frame) {
        Some(resp) => resp,
        None => {
            warn!("Failed to decode VISCA frame: {frame:02X?}");
            return Ok(());
        }
    };
    debug!("[handle_response] Parsed response: {basic_response:?}");
    trace!("Parsed response: {basic_response:?}");

    match basic_response.kind {
        BasicKind::Ack => {
            let Some(socket) = basic_response.socket else {
                warn!("Received ACK without socket information");
                return Ok(());
            };
            // Camera has assigned a socket - handle the ACK
            let now = executor.now();
            if let Some(cmd_id) = scheduler.handle_ack(socket, now) {
                debug!("ACK received - command {cmd_id} assigned to {socket:?} by camera");

                // Don't send ACK to the response channel - wait for Completion
                // The response channel is expecting the final result, not intermediate ACKs
            } else {
                warn!("Received ACK for {socket:?} but no pending commands awaiting ACK");
            }
        }

        BasicKind::Completion => {
            let Some(socket) = basic_response.socket else {
                warn!("Received Completion without socket information");
                return Ok(());
            };
            if let Some(cmd_id) = scheduler.socket_command(socket) {
                debug!("Completion received for command {cmd_id} on {socket:?}");

                // Track successful completion
                scheduler
                    .metrics
                    .commands_completed
                    .fetch_add(1, Ordering::Relaxed);

                // Remove from retry queue if it was being retried
                scheduler.remove_from_retry_queue(cmd_id);

                // Notify the waiting command and free the socket
                if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                    debug!("Sending completion to response channel for command {cmd_id}");
                    if let Err(e) = response_tx.send(Ok(ViscaResponse::Completion {
                        socket: Some(socket),
                    })) {
                        warn!("Failed to send completion to response channel: {e:?}");
                    }
                } else {
                    warn!("No response channel found for command {cmd_id} completion");
                }
                scheduler.free_socket(socket);

                // Process any queued commands now that a socket is free
                if let Err(e) = process_command_queue(
                    transport,
                    scheduler,
                    executor,
                    true,
                    envelope,
                    buffer_manager,
                )
                .await
                {
                    error!("Error processing command queue after completion: {e}");
                }
            } else {
                warn!("Received completion for {socket:?} with no pending command");
            }
        }

        BasicKind::DataReply => {
            let data = basic_response.payload;
            debug!("Data reply received: {data:02X?}");

            // For inquiries, we need to match this with the pending inquiry
            // Since inquiries don't use sockets, we need a different mechanism
            // For now, assume the most recent inquiry is the one being responded to
            if let Some((_inquiry_id, response_tx, response_type)) = scheduler.get_pending_inquiry()
            {
                // Use the unified lift_inquiry function to parse the response
                let response = match lift_inquiry(&basic_response, response_type.as_ref()) {
                    Ok(parsed) => Ok(parsed),
                    Err(e) => {
                        warn!("Failed to parse inquiry response: {e}");
                        Ok(ViscaResponse::Unknown {
                            response_type,
                            data: data.to_vec(),
                        })
                    }
                };

                let _ = response_tx.send(response);
            } else {
                warn!("Received data reply with no pending inquiry");
            }
        }

        BasicKind::Error(error_code) => {
            let socket = basic_response.socket;
            let error = ViscaError::from_byte(error_code);
            warn!("Error response: {error:?} on socket {socket:?}");

            // First check if this is an error for a pending inquiry
            // Inquiries don't have sockets, so if there's no socket or no command on the socket,
            // and we have a pending inquiry, this error is for the inquiry
            let has_pending_inquiry = scheduler.get_pending_inquiry().is_some();
            let is_inquiry_error = socket.map_or(true, |s| scheduler.socket_command(s).is_none());

            if has_pending_inquiry && is_inquiry_error {
                // This error is for a pending inquiry
                if let Some((inquiry_id, response_tx, _response_type)) =
                    scheduler.get_pending_inquiry()
                {
                    debug!("Error {error:?} for inquiry {inquiry_id}");
                    let error_code = error.as_byte();
                    let _ = response_tx.send(Err(Error::from_code(error_code)));
                    scheduler
                        .metrics
                        .commands_failed
                        .fetch_add(1, Ordering::Relaxed);
                    return Ok(());
                }
            }

            // Handle errors for commands that haven't received ACK yet
            // Check if we have pending ACK commands and no command assigned to the socket yet
            let is_pending_ack_error = if let Some(sock) = socket {
                // If socket has error but no command assigned, it's a pending ACK error
                scheduler.socket_command(sock).is_none() && scheduler.pending_ack_count() > 0
            } else {
                // No socket specified - always check pending ACK
                scheduler.pending_ack_count() > 0
            };

            if is_pending_ack_error {
                if let Some((cmd_id, priority, category, bytes, camera_id)) =
                    scheduler.handle_pending_ack_error_with_bytes(error)
                {
                    debug!("{error:?} for pending ACK command {cmd_id}");

                    // Store metadata for potential retry (it wasn't stored since we never got ACK)
                    scheduler.store_command_metadata(
                        cmd_id,
                        bytes.clone(),
                        priority,
                        category,
                        camera_id,
                    );

                    // Queue for retry if it's a retryable error
                    let retryable = error.is_retryable(Some(category));

                    if retryable {
                        let now = executor.now();
                        let queued = scheduler
                            .queue_for_retry(cmd_id, bytes, priority, category, camera_id, now);
                        if !queued {
                            // Retries exhausted - error already sent to response channel by queue_for_retry
                            debug!("Command {cmd_id} exhausted retries");
                        } else {
                            // Successfully queued for retry
                            debug!("Command {cmd_id} queued for retry: {error:?}");
                        }
                    } else {
                        // Non-retryable error - send error response immediately
                        debug!("Non-retryable error {error:?} for command {cmd_id}");
                        if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                            // Convert ViscaError to Error using the byte code
                            let error_code = error.as_byte();
                            let _ = response_tx.send(Err(Error::from_code(error_code)));
                            scheduler
                                .metrics
                                .commands_failed
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    return Ok(());
                }
            }

            // Check if error is retryable based on error type and command context
            let should_retry = if let Some(sock) = socket {
                if let Some(cmd_id) = scheduler.socket_command(sock) {
                    // Get command category to determine if NotExecutable (0x41) is retryable
                    let category = scheduler
                        .get_command_for_retry(cmd_id)
                        .map(|(_, _, cat, _)| cat);
                    error.is_retryable(category)
                } else {
                    false
                }
            } else {
                false
            };

            if should_retry {
                if let Some(sock) = socket {
                    if let Some(cmd_id) = scheduler.socket_command(sock) {
                        debug!(
                            "[handle_response] Camera busy for command {cmd_id} on {sock:?}, will retry"
                        );
                        debug!("Camera busy for command {cmd_id} on {sock:?}, will retry");

                        // Get command metadata for retry and queue it BEFORE freeing socket
                        if let Some((bytes, priority, category, camera_id)) =
                            scheduler.get_command_for_retry(cmd_id)
                        {
                            debug!(
                                "Queueing command {cmd_id} for retry with priority {priority:?}"
                            );
                            // Queue the command for retry (returns false if exhausted)
                            let now = executor.now();
                            let queued = scheduler
                                .queue_for_retry(cmd_id, bytes, priority, category, camera_id, now);

                            if !queued {
                                debug!("Command {cmd_id} exhausted retries, not queuing");
                                // The queue_for_retry method has already sent the error response
                            }
                        } else {
                            warn!("No metadata found for command {cmd_id} to retry");
                        }

                        // Free the socket so it can be reused
                        // The free_socket method will check if command is queued for retry
                        scheduler.free_socket(sock);

                        // Don't send error to response channel if queued for retry
                        // The command will be retried automatically when a socket becomes available
                        // The response channel remains stored in the scheduler

                        // Process any queued commands now that a socket is free
                        // Keep consecutive_retries count, as this wasn't a successful queue operation
                        if let Err(e) = process_command_queue(
                            transport,
                            scheduler,
                            executor,
                            true,
                            envelope,
                            buffer_manager,
                        )
                        .await
                        {
                            error!("Error processing command queue after busy: {e}");
                        }
                    }
                }
            } else {
                // Handle other errors normally

                // Track command failure
                scheduler
                    .metrics
                    .commands_failed
                    .fetch_add(1, Ordering::Relaxed);

                if let Some(sock) = socket {
                    if let Some(cmd_id) = scheduler.socket_command(sock) {
                        // Remove from retry queue if it was being retried
                        scheduler.remove_from_retry_queue(cmd_id);

                        // Notify the waiting command and free the socket
                        if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                            // Convert ViscaError to Error using the byte code
                            let error_code = error.as_byte();
                            let _ = response_tx.send(Err(Error::from_code(error_code)));
                        }
                        scheduler.free_socket(sock);

                        // Process any queued commands now that a socket is free
                        if let Err(e) = process_command_queue(
                            transport,
                            scheduler,
                            executor,
                            true,
                            envelope,
                            buffer_manager,
                        )
                        .await
                        {
                            error!("Error processing command queue after error: {e}");
                        }
                    }
                } else {
                    // Broadcast error with no specific socket
                    // Try to route to the most recently sent command
                    // (VISCA cameras typically send broadcast errors for the last command)

                    // Get the most recent command ID from any socket
                    let recent_cmd_id = scheduler.most_recent_command();

                    if let Some(cmd_id) = recent_cmd_id {
                        debug!("Routing broadcast error to command {cmd_id}");

                        // Notify the waiting command
                        if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                            let error_code = error.as_byte();
                            let _ = response_tx.send(Err(Error::from_code(error_code)));
                        }

                        // Free any socket that might be associated with this command
                        scheduler.free_command_socket(cmd_id);
                    } else {
                        // No pending commands, just broadcast the error
                    }
                }
            }
        }

        BasicKind::NetworkChange => {
            debug!("Network change notification received");
            // Could trigger a re-initialization or status check
        }

        BasicKind::Unknown => {
            warn!(
                "Unknown response received: {payload:02X?}",
                payload = basic_response.payload
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "async")]
    #[test]
    fn test_socket_id() {
        use crate::ViscaSocket;
        assert_eq!(ViscaSocket::S1.as_index(), 0);
        assert_eq!(ViscaSocket::S2.as_index(), 1);
    }

    #[cfg(feature = "async")]
    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_response_parsing() {
        use crate::command::bytes::VISCA_TERMINATOR;
        use crate::protocol::response::{decode_basic, BasicKind};
        use crate::ViscaSocket;

        // Test ACK parsing
        let ack_frame = vec![0x90, 0x41, VISCA_TERMINATOR];
        let response = decode_basic(&ack_frame).unwrap();
        assert_eq!(response.kind, BasicKind::Ack);
        assert_eq!(response.socket, Some(ViscaSocket::S1));

        // Test Completion parsing
        let completion_frame = vec![0x90, 0x52, VISCA_TERMINATOR];
        let response = decode_basic(&completion_frame).unwrap();
        assert_eq!(response.kind, BasicKind::Completion);
        assert_eq!(response.socket, Some(ViscaSocket::S2));

        // Test Data Reply parsing
        let data_frame = vec![0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = decode_basic(&data_frame).unwrap();
        assert_eq!(response.kind, BasicKind::DataReply);
        assert_eq!(response.payload, &[0x02]);

        // Test Error parsing
        let error_frame = vec![0x90, 0x61, 0x03, VISCA_TERMINATOR];
        let response = decode_basic(&error_frame).unwrap();
        assert_eq!(response.kind, BasicKind::Error(0x03));
        assert_eq!(response.socket, Some(ViscaSocket::S1));
    }

    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
    #[tokio::test(start_paused = true)]
    #[allow(clippy::expect_used)]
    async fn test_runtime_sends_command() {
        use crate::command::response::ViscaResponse;
        use crate::runtime::scheduler::{Priority, TxItem};
        use crate::testing::testkit::{ScriptedTransport, Step};
        use crate::timeout::CommandCategory;
        use crate::TokioExecutor;
        use std::time::Duration;
        use tokio::runtime::Handle;

        // Create TokioExecutor and scripted transport
        let executor = Arc::new(TokioExecutor::from_handle(Handle::current()));
        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Power on command
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK
                vec![0x90, 0x51, 0xFF], // Completion
            ],
        }])
        .with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .expect("Failed to create runtime handle");

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send a command
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let command = TxItem::Command {
            id: 1,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF], // Power on
            priority: Priority::Normal,
            camera_id: crate::camera_id::CameraId::CAMERA_1,
            response_tx,
            category: CommandCategory::Quick,
        };

        runtime
            .command(command)
            .await
            .expect("Failed to send command");

        // Advance time to allow command processing
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for response
        let response = response_rx
            .recv_async()
            .await
            .expect("Failed to receive response");
        assert!(matches!(response, Ok(ViscaResponse::Completion { .. })));

        // Verify command was sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]); // Command with terminator
    }

    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
    #[tokio::test(start_paused = true)]
    #[allow(clippy::expect_used, clippy::panic)]
    async fn test_runtime_handles_inquiry() {
        use crate::command::response::{ViscaResponse, ViscaResponseType};
        use crate::runtime::scheduler::TxItem;
        use crate::testing::testkit::{ScriptedTransport, Step};
        use crate::TokioExecutor;
        use std::time::Duration;
        use tokio::runtime::Handle;

        // Create deterministic executor and scripted transport
        let executor = Arc::new(TokioExecutor::from_handle(Handle::current()));
        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
            responses: vec![
                vec![0x90, 0x50, 0x02, 0xFF], // Power on response
            ],
        }])
        .with_executor(executor.clone());

        let runtime = RuntimeHandle::new(transport.clone(), executor.clone())
            .await
            .expect("Failed to create runtime handle");

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send an inquiry
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let inquiry = TxItem::Inquiry {
            id: 1,
            bytes: vec![0x81, 0x09, 0x04, 0x00, 0xFF], // Power inquiry
            camera_id: crate::camera_id::CameraId::CAMERA_1,
            response_tx,
            response_type: Some(ViscaResponseType::Power),
        };

        runtime
            .inquire(inquiry)
            .await
            .expect("Failed to send inquiry");

        // Advance time to allow inquiry processing
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for response
        let response = response_rx
            .recv_async()
            .await
            .expect("Failed to receive response");

        // Should receive inquiry response
        match response {
            Ok(ViscaResponse::Inquiry(_)) => {
                // Expected - actual data would be in the InquiryResponse
            }
            Ok(ViscaResponse::Unknown { .. }) => {
                // Also acceptable for this test
            }
            unexpected => {
                panic!("Expected Inquiry or Unknown response, got: {unexpected:?}")
            }
        }

        // Verify inquiry was sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x09, 0x04, 0x00, 0xFF]); // Inquiry with terminator
    }

    #[cfg(feature = "rt-tokio")]
    #[tokio::test]
    #[allow(clippy::expect_used)]
    async fn test_runtime_loop_shutdown() {
        use bytes::Bytes;

        // Mock transport that never returns data
        struct MockTransport;

        impl AsyncTransport for MockTransport {
            async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
                Ok(())
            }

            async fn recv(&mut self) -> Result<Bytes, Error> {
                // Never return, simulating waiting for data
                std::future::pending().await
            }
        }

        let (submit_tx, submit_rx) = flume::unbounded();
        let (_metrics_tx, metrics_rx) = flume::unbounded();

        // Start runtime loop
        let executor = Arc::new(
            crate::executor::TokioExecutor::from_current()
                .expect("Failed to create TokioExecutor from current runtime"),
        );
        let envelope = TransportEnvelope::new(ProtocolStyle::RawVisca);
        let buffer_manager = BufferManager::new(BufferConfig::default());
        let runtime_task = runtime_loop_with_config(
            MockTransport,
            submit_rx,
            metrics_rx,
            None,
            executor,
            envelope,
            buffer_manager,
        );

        // Spawn the runtime
        let handle = tokio::spawn(runtime_task);

        // Drop the sender to trigger shutdown
        drop(submit_tx);

        // Runtime should shutdown gracefully
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), handle).await;

        assert!(
            result.is_ok(),
            "Runtime should shutdown when submit channel is closed"
        );
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
    #[tokio::test(start_paused = true)]
    #[allow(clippy::expect_used)]
    async fn test_runtime_sony_encapsulated_command() {
        use crate::capabilities::ProtocolStyle;
        use crate::command::response::ViscaResponse;
        use crate::runtime::scheduler::{Priority, TxItem};
        use crate::testing::testkit::{ScriptedTransport, Step};
        use crate::timeout::CommandCategory;
        use crate::TokioExecutor;
        use std::time::Duration;
        use tokio::runtime::Handle;

        // Create TokioExecutor and scripted transport
        let executor = Arc::new(TokioExecutor::from_handle(Handle::current()));

        // The framed command: 8-byte Sony header + VISCA command
        // Header: 0x01 0x00 (command type), 0x00 0x06 (length=6), 0x00 0x00 0x00 0x00 (sequence)
        // VISCA: 0x81 0x01 0x04 0x00 0x02 0xFF (Power on)
        let framed_command = vec![
            0x01, 0x00, // Command payload type
            0x00, 0x06, // Length = 6
            0x00, 0x00, 0x00, 0x00, // Sequence = 0
            0x81, 0x01, 0x04, 0x00, 0x02, 0xFF, // Power on command
        ];

        // Sony encapsulated responses
        let framed_ack = vec![
            0x01, 0x11, // Reply payload type
            0x00, 0x03, // Length = 3
            0x00, 0x00, 0x00, 0x00, // Sequence = 0
            0x90, 0x41, 0xFF, // ACK
        ];

        let framed_completion = vec![
            0x01, 0x11, // Reply payload type
            0x00, 0x03, // Length = 3
            0x00, 0x00, 0x00, 0x01, // Sequence = 1
            0x90, 0x51, 0xFF, // Completion
        ];

        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(framed_command.clone()),
            responses: vec![framed_ack, framed_completion],
        }])
        .with_executor(executor.clone());

        // Create runtime with Sony encapsulated protocol
        let runtime = RuntimeHandle::new_with_style(
            transport.clone(),
            executor.clone(),
            ProtocolStyle::SonyEncapsulated {
                use_sequence: false,
            },
        )
        .await
        .expect("Failed to create runtime handle");

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send a command
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let command = TxItem::Command {
            id: 1,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF], // Raw VISCA command
            priority: Priority::Normal,
            camera_id: crate::camera_id::CameraId::CAMERA_1,
            response_tx,
            category: CommandCategory::Quick,
        };

        runtime
            .command(command)
            .await
            .expect("Failed to send command");

        // Advance time to allow command processing
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for response
        let response = response_rx
            .recv_async()
            .await
            .expect("Failed to receive response");
        assert!(matches!(response, Ok(ViscaResponse::Completion { .. })));

        // Verify the framed command was sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], framed_command); // Should send the framed version
    }

    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
    #[tokio::test(start_paused = true)]
    #[allow(clippy::expect_used, clippy::panic)]
    async fn test_runtime_sony_encapsulated_inquiry() {
        use crate::capabilities::ProtocolStyle;
        use crate::command::response::{ViscaResponse, ViscaResponseType};
        use crate::runtime::scheduler::TxItem;
        use crate::testing::testkit::{ScriptedTransport, Step};
        use crate::TokioExecutor;
        use std::time::Duration;
        use tokio::runtime::Handle;

        // Create deterministic executor and scripted transport
        let executor = Arc::new(TokioExecutor::from_handle(Handle::current()));

        // The framed inquiry: 8-byte Sony header + VISCA inquiry
        // Header: 0x01 0x10 (inquiry type), 0x00 0x05 (length=5), 0x00 0x00 0x00 0x00 (sequence)
        // VISCA: 0x81 0x09 0x04 0x00 0xFF (Power inquiry)
        let framed_inquiry = vec![
            0x01, 0x10, // Inquiry payload type
            0x00, 0x05, // Length = 5
            0x00, 0x00, 0x00, 0x00, // Sequence = 0
            0x81, 0x09, 0x04, 0x00, 0xFF, // Power inquiry
        ];

        // Sony encapsulated response
        let framed_response = vec![
            0x01, 0x11, // Reply payload type
            0x00, 0x04, // Length = 4
            0x00, 0x00, 0x00, 0x00, // Sequence = 0
            0x90, 0x50, 0x02, 0xFF, // Power on response
        ];

        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(framed_inquiry.clone()),
            responses: vec![framed_response],
        }])
        .with_executor(executor.clone());

        // Create runtime with Sony encapsulated protocol
        let runtime = RuntimeHandle::new_with_style(
            transport.clone(),
            executor.clone(),
            ProtocolStyle::SonyEncapsulated {
                use_sequence: false,
            },
        )
        .await
        .expect("Failed to create runtime handle");

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send an inquiry
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let inquiry = TxItem::Inquiry {
            id: 1,
            bytes: vec![0x81, 0x09, 0x04, 0x00, 0xFF], // Raw VISCA inquiry
            camera_id: crate::camera_id::CameraId::CAMERA_1,
            response_tx,
            response_type: Some(ViscaResponseType::Power),
        };

        runtime
            .inquire(inquiry)
            .await
            .expect("Failed to send inquiry");

        // Advance time to allow inquiry processing
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for response
        let response = response_rx
            .recv_async()
            .await
            .expect("Failed to receive response");

        // Should receive inquiry response
        match response {
            Ok(ViscaResponse::Inquiry(_)) => {
                // Expected - actual data would be in the InquiryResponse
            }
            Ok(ViscaResponse::Unknown { .. }) => {
                // Also acceptable for this test
            }
            unexpected => {
                panic!("Expected Inquiry or Unknown response, got: {unexpected:?}")
            }
        }

        // Verify the framed inquiry was sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], framed_inquiry); // Should send the framed version
    }

    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
    #[tokio::test(start_paused = true)]
    #[allow(clippy::expect_used, clippy::panic)]
    async fn test_runtime_with_explicit_sony_protocol() {
        use crate::capabilities::ProtocolStyle;
        use crate::command::response::ViscaResponse;
        use crate::runtime::scheduler::{Priority, TxItem};
        use crate::testing::testkit::{ScriptedTransport, Step};
        use crate::timeout::CommandCategory;
        use crate::TokioExecutor;
        use std::time::Duration;
        use tokio::runtime::Handle;

        // Create TokioExecutor and scripted transport
        let executor = Arc::new(TokioExecutor::from_handle(Handle::current()));

        // After creating runtime with Sony protocol, it will use Sony framing for commands
        let framed_command = vec![
            0x01, 0x00, // Command type
            0x00, 0x06, // Length = 6
            0x00, 0x00, 0x00, 0x00, // Sequence = 0 (no sequence tracking)
            0x81, 0x01, 0x04, 0x00, 0x02, 0xFF, // Power on command
        ];

        let framed_ack = vec![
            0x01, 0x11, // Reply type
            0x00, 0x03, // Length = 3
            0x00, 0x00, 0x00, 0x00, // Sequence = 0
            0x90, 0x41, 0xFF, // ACK
        ];

        let framed_completion = vec![
            0x01, 0x11, // Reply type
            0x00, 0x03, // Length = 3
            0x00, 0x00, 0x00, 0x01, // Sequence = 1
            0x90, 0x51, 0xFF, // Completion
        ];

        let transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(framed_command.clone()),
            responses: vec![framed_ack, framed_completion],
        }])
        .with_executor(executor.clone());

        // Create runtime with explicit Sony protocol style
        let runtime = RuntimeHandle::new_with_style(
            transport.clone(),
            executor.clone(),
            ProtocolStyle::SonyEncapsulated {
                use_sequence: false,
            },
        )
        .await
        .expect("Failed to create runtime handle with Sony protocol");

        // Give the runtime a moment to start
        tokio::time::advance(Duration::from_millis(50)).await;

        // Send a command - should use Sony protocol
        let (response_tx, response_rx): (
            Sender<Result<ViscaResponse>>,
            Receiver<Result<ViscaResponse>>,
        ) = flume::bounded(1);
        let command = TxItem::Command {
            id: 1,
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF], // Raw VISCA command
            priority: Priority::Normal,
            camera_id: crate::camera_id::CameraId::CAMERA_1,
            response_tx,
            category: CommandCategory::Quick,
        };

        runtime
            .command(command)
            .await
            .expect("Failed to send command");

        // Advance time to allow command processing
        tokio::time::advance(Duration::from_millis(100)).await;

        // Wait for response
        let response = response_rx
            .recv_async()
            .await
            .expect("Failed to receive response");
        assert!(matches!(response, Ok(ViscaResponse::Completion { .. })));

        // Verify the framed command was sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], framed_command); // Should send the framed version
    }
}
