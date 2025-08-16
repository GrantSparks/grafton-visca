//! VISCA runtime implementation using flume channels.
//!
//! This module provides the core runtime for VISCA communication,
//! managing command scheduling, socket allocation, and protocol timing.

pub mod scheduler;
#[cfg(feature = "async")]
mod time_utils;

pub use scheduler::{
    LinkEvent, MetricsSummary, Priority, RxEvent, Scheduler, SchedulerMetrics, SocketId, TxItem,
    ViscaError,
};

#[cfg(feature = "async")]
use flume::{Receiver, Sender};
#[cfg(feature = "async")]
use futures_lite;
#[cfg(feature = "async")]
use log::{debug, error, trace, warn};

#[cfg(feature = "async")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[cfg(feature = "async")]
use crate::{
    command::response::ViscaResponse,
    error::{Error, Result},
    transport::AsyncTransport,
};

/// Helper function to spawn runtime tasks properly for different executor types.
#[cfg(feature = "async")]
fn spawn_runtime_task_properly<E: crate::executor::Executor>(
    executor: &E,
    runtime_task: impl std::future::Future<Output = Result<(), Error>> + Send + 'static,
) {
    // For DeterministicExecutor in test mode, we need to detach the task
    #[cfg(any(feature = "rt-tokio", feature = "test-utils"))]
    {
        use std::any::Any;
        let executor_any: &dyn Any = executor;

        // Debug logging to see what type we have
        eprintln!(
            "[spawn_runtime_task_properly] Attempting to spawn runtime task, executor type: {:?}",
            std::any::type_name_of_val(&executor_any)
        );

        #[cfg(feature = "test-utils")]
        {
            // Try direct DeterministicExecutor
            if let Some(det_exec) =
                executor_any.downcast_ref::<crate::testing::testkit::DeterministicExecutor>()
            {
                eprintln!(
                    "[spawn_runtime_task_properly] Detected DeterministicExecutor, using spawn_bg"
                );
                // Use ExecutorExt::spawn_bg which detaches the task
                use crate::testing::testkit::deterministic_executor::ExecutorExt;
                det_exec.spawn_bg(async move {
                    eprintln!("[runtime task] Runtime task starting (DeterministicExecutor)");
                    match runtime_task.await {
                        Ok(()) => eprintln!("[runtime task] Runtime task completed successfully"),
                        Err(e) => eprintln!("[runtime task] Runtime task failed: {}", e),
                    }
                });
                eprintln!("[spawn_runtime_task_properly] spawn_bg called, returning");
                return;
            }

            // Try Arc<DeterministicExecutor>
            if let Some(arc_det) =
                executor_any.downcast_ref::<Arc<crate::testing::testkit::DeterministicExecutor>>()
            {
                log::debug!("Detected Arc<DeterministicExecutor>, using spawn_bg");
                // Use ExecutorExt::spawn_bg which detaches the task
                use crate::testing::testkit::deterministic_executor::ExecutorExt;
                arc_det.spawn_bg(async move {
                    log::debug!("Runtime task starting (Arc<DeterministicExecutor>)");
                    if let Err(e) = runtime_task.await {
                        log::error!("Runtime task failed: {}", e);
                    } else {
                        log::debug!("Runtime task completed successfully");
                    }
                });
                return;
            }
        }

        log::debug!("DeterministicExecutor not detected, falling through to regular spawn");
    }

    // For all other executors, use regular spawn (handle gets dropped)
    log::debug!("Using regular spawn for runtime task");
    let _handle = executor.spawn(runtime_task);
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
#[derive(Debug)]
pub struct RuntimeHandle {
    /// Channel for submitting commands and inquiries.
    submit: Sender<TxItem>,
    /// Channel for receiving events from the runtime.
    events: Receiver<RxEvent>,
    // Runtime handle not needed with flume-based design
    // Tasks are managed internally by the runtime loop
    /// Flag to track if runtime is shutdown.
    shutdown: Arc<AtomicBool>,
    /// Channel for requesting metrics from the runtime.
    metrics_tx: Sender<Sender<MetricsSummary>>,
}

#[cfg(feature = "async")]
impl RuntimeHandle {
    /// Create a new camera runtime with the given transport.
    ///
    /// This spawns a background task to handle communication with the camera.
    #[cfg(feature = "async")]
    pub async fn new<T: AsyncTransport + 'static, E: crate::executor::Executor>(
        transport: T,
        executor: Arc<E>,
    ) -> Result<Self> {
        Self::with_tick_interval(transport, executor, None).await
    }

    /// Create a new camera runtime with a custom tick interval.
    ///
    /// The tick interval controls how often the runtime checks for timeouts
    /// and processes retries. Default is 50ms.
    ///
    /// # Arguments
    /// * `transport` - The transport to use for communication
    /// * `executor` - The async executor to spawn tasks on
    /// * `tick_interval_ms` - Optional tick interval in milliseconds (default: 50ms)
    #[cfg(feature = "async")]
    pub async fn with_tick_interval<T: AsyncTransport + 'static, E: crate::executor::Executor>(
        transport: T,
        executor: Arc<E>,
        tick_interval_ms: Option<u64>,
    ) -> Result<Self> {
        let (submit_tx, submit_rx) = flume::unbounded();
        // Make event channel bounded to avoid unbounded memory growth
        let (event_tx, event_rx) = flume::bounded(1000);
        let (metrics_tx, metrics_rx) = flume::unbounded();

        // Spawn the runtime task with configured tick interval
        let runtime_task = runtime_loop_with_config(
            transport,
            submit_rx,
            event_tx,
            metrics_rx,
            tick_interval_ms,
            Arc::clone(&executor),
        );

        // Spawn the task, with special handling for deterministic executors in test mode
        eprintln!("[RuntimeHandle::with_tick_interval] About to spawn runtime task");
        spawn_runtime_task_properly(executor.as_ref(), runtime_task);
        eprintln!("[RuntimeHandle::with_tick_interval] Runtime task spawned");

        Ok(Self {
            submit: submit_tx,
            events: event_rx,
            shutdown: Arc::new(AtomicBool::new(false)),
            metrics_tx,
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
        let config = crate::transport::ip_raw::RawIpConfig {
            address: address.as_ref().to_string(),
            ..Default::default()
        };
        let transport = crate::transport::ip_raw::AsyncRawTcpTransport::connect(config).await?;
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
        let config = crate::transport::ip_raw::RawIpConfig {
            address: address.as_ref().to_string(),
            ..Default::default()
        };
        let transport = crate::transport::ip_raw::AsyncRawUdpTransport::connect(config).await?;
        Self::new(transport, executor).await
    }

    /// Create a new camera runtime with Sony TCP transport.
    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    pub async fn new_tcp_sony<E: crate::executor::Executor>(
        address: impl AsRef<str>,
        executor: Arc<E>,
    ) -> Result<Self> {
        let config = crate::transport::ip_sony::SonyIpConfig {
            address: address.as_ref().to_string(),
            use_tcp: true,
            ..Default::default()
        };
        let transport = crate::transport::ip_sony::AsyncSonyTcpTransport::connect(config).await?;
        Self::new(transport, executor).await
    }

    /// Create a new camera runtime with Sony UDP transport.
    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    pub async fn new_udp_sony<E: crate::executor::Executor>(
        address: impl AsRef<str>,
        executor: Arc<E>,
    ) -> Result<Self> {
        let config = crate::transport::ip_sony::SonyIpConfig {
            address: address.as_ref().to_string(),
            use_tcp: false,
            ..Default::default()
        };
        let transport = crate::transport::ip_sony::AsyncSonyUdpTransport::connect(config).await?;
        Self::new(transport, executor).await
    }

    /// Create a new camera runtime with serial transport.
    #[cfg(all(feature = "async", feature = "serial", feature = "rt-tokio"))]
    pub async fn new_serial<E: crate::executor::Executor>(
        port: impl AsRef<str>,
        camera_address: u8,
        executor: Arc<E>,
    ) -> Result<Self> {
        let config = crate::transport::serial::SerialConfig {
            port: port.as_ref().to_string(),
            camera_address,
            ..Default::default()
        };
        let transport = crate::transport::serial::AsyncSerialTransport::new(config).await?;
        Self::new(transport, executor).await
    }

    /// Send a command item to the runtime.
    pub async fn command(&self, item: TxItem) -> Result<()> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Err(Error::RuntimeShutdown);
        }
        self.submit
            .send_async(item)
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Send an inquiry item to the runtime.
    pub async fn inquire(&self, item: TxItem) -> Result<()> {
        self.submit
            .send_async(item)
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    /// Cancel a command on the specified socket.
    pub async fn cancel(&self, _command_id: u32) -> Result<()> {
        // For now, we don't have a direct way to cancel by command ID
        // This would need to be implemented in the scheduler
        warn!("Cancel by command_id not yet implemented");
        Ok(())
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

    /// Get the next event from the runtime.
    ///
    /// This can be used for monitoring or custom event handling.
    pub async fn next_event(&self) -> Option<RxEvent> {
        self.events.recv_async().await.ok()
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

    /// Send a VISCA command to the camera using the EncodeVisca trait.
    ///
    /// This method bridges the existing command system with the new runtime.
    ///
    /// # Arguments
    /// * `cmd` - A command implementing the EncodeVisca trait
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
        C: crate::command::encode_visca::EncodeVisca,
    {
        // Encode the command
        let mut buffer = vec![0u8; C::MAX_SIZE];
        let len = cmd.encode_into(camera_id, &mut buffer)?;
        buffer.truncate(len);

        // Create response channel
        let (response_tx, response_rx) = flume::bounded(1);

        // Create the TxItem
        let item = TxItem::Command {
            id: 0, // Will be assigned by scheduler
            bytes: buffer,
            priority: priority.unwrap_or(Priority::Normal),
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(30),
            category: C::TIMEOUT_CATEGORY,
            response_tx,
        };

        // Submit the command
        self.command(item).await?;

        // Wait for response
        response_rx
            .recv_async()
            .await
            .map_err(|_| Error::ChannelClosed)?
    }

    /// Send a VISCA inquiry to the camera using the EncodeVisca trait.
    ///
    /// This method bridges the existing inquiry system with the new runtime.
    ///
    /// # Arguments
    /// * `inquiry` - An inquiry command implementing the EncodeVisca trait
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
        I: crate::command::encode_visca::EncodeVisca,
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
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
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
async fn runtime_loop_with_config<T: AsyncTransport, E: crate::executor::Executor>(
    transport: T,
    submit_rx: Receiver<TxItem>,
    event_tx: Sender<RxEvent>,
    metrics_rx: Receiver<Sender<MetricsSummary>>,
    tick_interval_ms: Option<u64>,
    executor: Arc<E>,
) -> Result<()> {
    let mut scheduler = Scheduler::new(submit_rx.clone(), event_tx.clone());
    let mut response_buffer = Vec::new();
    let mut consecutive_retries = 0usize;

    eprintln!("[runtime_loop_with_config] VISCA runtime started");

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
                &transport,
                &mut scheduler,
                item,
                &event_tx,
                executor.as_ref(),
            )
            .await
            {
                Ok(_) => {
                    // Successfully handled a TX item - reset consecutive retries
                    consecutive_retries = 0;

                    // Process command queue with retry budget check
                    let allow_retry_defer = consecutive_retries < 8;
                    if let Err(e) = process_command_queue(
                        &transport,
                        &mut scheduler,
                        &event_tx,
                        executor.as_ref(),
                        allow_retry_defer,
                    )
                    .await
                    {
                        error!("Error processing command queue after TX item: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error handling TX item: {}", e);
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
        let now = time_utils::now_from_executor_arc(&executor);
        if scheduler.has_free_socket() {
            while let Some(deadline) = scheduler.next_retry_deadline() {
                if deadline > now {
                    break; // No more retries due now
                }

                // Get the next retry that's due now
                if let Some(retry_cmd) = scheduler.get_next_retry(now) {
                    debug!(
                        "Pre-draining retry for command {} (attempt {})",
                        retry_cmd.id, retry_cmd.attempt
                    );

                    // Re-submit the command for retry
                    let response_tx = if let Some(tx) =
                        scheduler.peek_response_channel(retry_cmd.id)
                    {
                        tx.clone()
                    } else {
                        warn!(
                            "Missing response channel for retry of command {} - this indicates a bug",
                            retry_cmd.id
                        );
                        let (tx, _rx) = flume::bounded(1);
                        tx
                    };

                    // Create TxItem for the retry (use a fresh deadline based on category timeout)
                    let timeout_duration = retry_cmd.category.default_timeout();
                    let deadline = now + timeout_duration;
                    let tx_item = TxItem::Command {
                        id: retry_cmd.id,
                        bytes: retry_cmd.bytes.clone(),
                        priority: retry_cmd.priority,
                        deadline,
                        category: retry_cmd.category,
                        response_tx,
                    };

                    // Send the retry immediately
                    if let Err(e) = handle_tx_item(
                        &transport,
                        &mut scheduler,
                        tx_item,
                        &event_tx,
                        executor.as_ref(),
                    )
                    .await
                    {
                        error!("Error handling retry TX item: {}", e);
                    }

                    // If no more free sockets, stop draining
                    if !scheduler.has_free_socket() {
                        break;
                    }
                } else {
                    break; // No retry ready (shouldn't happen since we checked deadline)
                }
            }
        }

        // Set up the transport receive future
        let recv_fut = async { Operation::Recv(transport.recv().await) };

        // Dynamic tick scheduling: sleep until the earliest retry deadline or housekeeping tick
        let now = time_utils::now_from_executor_arc(&executor);
        let sleep_dur = if let Some(deadline) = scheduler.next_retry_deadline() {
            // Wake exactly when a retry becomes eligible (but never later than the housekeeping tick)
            let until_retry = deadline.saturating_duration_since(now);
            std::cmp::min(until_retry, tick_duration)
        } else {
            tick_duration
        };
        let tick_fut = async {
            executor.sleep(sleep_dur).await;
            Operation::Tick
        };

        // Race recv and tick futures
        let operation = futures_lite::future::race(recv_fut, tick_fut).await;

        match operation {
            Operation::Recv(recv_result) => {
                // Handle received data
                match recv_result {
                    Ok(bytes) => {
                        trace!("Received bytes from transport: {:02X?}", bytes);
                        response_buffer.extend_from_slice(&bytes);

                        // Parse complete frames from the buffer
                        let (frames, remaining) =
                            crate::protocol::decode::parse_frames(&response_buffer);
                        response_buffer = remaining;

                        for frame in frames {
                            if let Err(e) = handle_response(
                                &transport,
                                &mut scheduler,
                                &frame,
                                &event_tx,
                                executor.as_ref(),
                            )
                            .await
                            {
                                error!("Error handling response: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving from transport: {}", e);
                    }
                }
            }
            Operation::Tick => {
                // eprintln!("[runtime loop] Tick fired");
                // Handle tick - check for timeouts and retries
                let now = time_utils::now_from_executor_arc(&executor);
                let timed_out = scheduler.check_timeouts(now);
                for (socket, cmd_id) in timed_out {
                    // First, notify the waiting high-level caller
                    if let Some(tx) = scheduler.get_response_channel(cmd_id) {
                        let _ = tx.send(Err(Error::Timeout));
                    }

                    // Then emit the structured RxEvent for observers/metrics
                    let _ = event_tx
                        .send_async(RxEvent::Error {
                            code: ViscaError::Timeout,
                            socket: Some(socket),
                            id: Some(cmd_id),
                        })
                        .await;

                    // Finally, free the socket & clean up metadata
                    scheduler.free_socket(socket);
                }

                // Then check if we have retries to process
                if scheduler.has_retries() {
                    debug!(
                        "Has {} retries pending, free socket: {}, retry queue: {:?}",
                        scheduler.retry_queue.len(),
                        scheduler.has_free_socket(),
                        scheduler
                            .retry_queue
                            .iter()
                            .map(|r| r.id)
                            .collect::<Vec<_>>()
                    );
                }
                if scheduler.has_free_socket() && scheduler.has_retries() {
                    // eprintln!("[runtime loop] Has free socket and retries to process");
                    // get_next_retry() now handles exhausted retries internally
                    let now = time_utils::now_from_executor_arc(&executor);
                    if let Some(retry_cmd) = scheduler.get_next_retry(now) {
                        eprintln!(
                            "[runtime loop] Retrying command {} (attempt {})",
                            retry_cmd.id, retry_cmd.attempt
                        );
                        debug!(
                            "Retrying command {} (attempt {})",
                            retry_cmd.id, retry_cmd.attempt
                        );

                        // Re-submit the command for retry
                        let response_tx = if let Some(tx) =
                            scheduler.peek_response_channel(retry_cmd.id)
                        {
                            tx.clone()
                        } else {
                            // This shouldn't happen in normal operation - it means the response channel
                            // was lost somehow. Create a new one just to send the error.
                            warn!(
                                "Missing response channel for retry of command {} - this indicates a bug",
                                retry_cmd.id
                            );
                            // Skip this retry and continue
                            continue;
                        };
                        debug!(
                            "Using existing response channel for retry of command {}",
                            retry_cmd.id
                        );

                        let now = time_utils::now_from_executor_arc(&executor);
                        let item = TxItem::Command {
                            id: retry_cmd.id,
                            bytes: retry_cmd.bytes.clone(),
                            priority: retry_cmd.priority,
                            category: retry_cmd.category,
                            deadline: now + std::time::Duration::from_secs(30),
                            response_tx,
                        };

                        if let Err(e) = handle_tx_item(
                            &transport,
                            &mut scheduler,
                            item,
                            &event_tx,
                            executor.as_ref(),
                        )
                        .await
                        {
                            error!("Error retrying command {}: {}", retry_cmd.id, e);
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
            eprintln!(
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
async fn process_command_queue<T: AsyncTransport, E: crate::executor::Executor>(
    transport: &T,
    scheduler: &mut Scheduler,
    event_tx: &Sender<RxEvent>,
    executor: &E,
    allow_retry_defer: bool,
) -> Result<()> {
    // Process commands from the priority queue while we have free sockets
    // But check if there's a higher priority retry ready first
    while scheduler.has_free_socket() && !scheduler.is_queue_empty() {
        // Check if there's a retry ready that has higher or equal priority than the next queued command
        let now = time_utils::now_from_executor(executor);
        if allow_retry_defer {
            if let Some(next_queue_priority) = scheduler.peek_queue_priority() {
                if let Some(retry_priority) = scheduler.peek_ready_retry_priority(now) {
                    // If retry has higher or equal priority, don't process queue yet
                    if retry_priority >= next_queue_priority {
                        debug!(
                            "Deferring queue processing - retry with priority {:?} waiting (queue has {:?})",
                            retry_priority, next_queue_priority
                        );
                        return Ok(());
                    }
                }
            }
        }

        if let Some(item) = scheduler.dequeue_command() {
            debug!(
                "Processing queued command from priority queue (remaining: {})",
                scheduler.queue_size()
            );
            // Process the dequeued command
            if let Err(e) = handle_tx_item(transport, scheduler, item, event_tx, executor).await {
                error!("Error processing queued command: {}", e);
            }
        }
    }
    Ok(())
}

/// Handle a submitted TX item.
#[cfg(feature = "async")]
async fn handle_tx_item<T: AsyncTransport, E: crate::executor::Executor>(
    transport: &T,
    scheduler: &mut Scheduler,
    item: TxItem,
    event_tx: &Sender<RxEvent>,
    executor: &E,
) -> Result<()> {
    use crate::protocol::encode::VISCA_TERMINATOR;
    use scheduler::ViscaError;

    match item {
        TxItem::Command {
            mut id,
            bytes,
            priority,
            deadline: _,
            category,
            response_tx,
        } => {
            // Assign ID if not set
            if id == 0 {
                id = scheduler.next_id();
            }

            trace!("Processing command {} with priority {:?}", id, priority);

            // Track metrics for command submission
            scheduler
                .metrics
                .commands_submitted
                .fetch_add(1, Ordering::Relaxed);
            let idx = SchedulerMetrics::priority_index(priority);
            scheduler.metrics.priority_counts[idx].fetch_add(1, Ordering::Relaxed);

            // Check if we have a free socket
            let now = time_utils::now_from_executor(executor);
            if let Some(socket) = scheduler.allocate_socket(id, category, now) {
                // Enforce command spacing
                scheduler.enforce_spacing_with(executor, now).await;

                // Send command
                trace!("Sending command {} on {:?}: {:02X?}", id, socket, bytes);
                if let Err(e) = transport.send(&bytes).await {
                    error!("Failed to send command {}: {}", id, e);
                    scheduler.free_socket(socket);
                    scheduler
                        .metrics
                        .commands_failed
                        .fetch_add(1, Ordering::Relaxed);
                    let _ = response_tx.send(Err(Error::TransportError(e.to_string().into())));
                    return Ok(());
                }

                // Store the response channel and metadata in the scheduler for later use
                scheduler.store_command_channel(id, response_tx);
                scheduler.store_command_metadata(id, bytes.clone(), priority, category);
            } else {
                // No socket available, add to priority queue
                debug!(
                    "No socket available for command {}, adding to queue with priority {:?}",
                    id, priority
                );

                // Store the response channel for when the command is eventually sent
                scheduler.store_command_channel(id, response_tx.clone());

                // Enqueue the command for later processing
                let now = time_utils::now_from_executor(executor);
                scheduler.enqueue_command(
                    TxItem::Command {
                        id,
                        bytes,
                        priority,
                        deadline: now + std::time::Duration::from_secs(30),
                        category,
                        response_tx,
                    },
                    now,
                );
            }
        }

        TxItem::Inquiry {
            mut id,
            bytes,
            deadline: _,
            response_type,
            response_tx,
        } => {
            // Assign ID if not set
            if id == 0 {
                id = scheduler.next_id();
            }

            trace!("Processing inquiry {}", id);

            // Track metrics for inquiry submission
            scheduler
                .metrics
                .inquiries_submitted
                .fetch_add(1, Ordering::Relaxed);

            // Inquiries don't need sockets
            let now = time_utils::now_from_executor(executor);
            scheduler.enforce_spacing_with(executor, now).await;

            // Send inquiry
            trace!("Sending inquiry {}: {:02X?}", id, bytes);
            if let Err(e) = transport.send(&bytes).await {
                error!("Failed to send inquiry {}: {}", id, e);
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
            trace!("Processing cancel for {:?}", socket);

            // Send cancel command
            let cancel_bytes = vec![0x81, socket.as_byte() | 0x20, VISCA_TERMINATOR];

            if let Err(e) = transport.send(&cancel_bytes).await {
                error!("Failed to send cancel: {}", e);
                return Ok(());
            }

            // Free the socket and notify
            if let Some(cmd_id) = scheduler.socket_command(socket) {
                scheduler.free_socket(socket);
                let _ = event_tx
                    .send_async(RxEvent::Error {
                        code: ViscaError::CommandCancelled,
                        socket: Some(socket),
                        id: Some(cmd_id),
                    })
                    .await;
            }
        }
    }

    Ok(())
}

/// Handle a VISCA response frame.
#[cfg(feature = "async")]
async fn handle_response<T: AsyncTransport, E: crate::executor::Executor>(
    transport: &T,
    scheduler: &mut Scheduler,
    frame: &[u8],
    event_tx: &Sender<RxEvent>,
    executor: &E,
) -> Result<()> {
    use crate::protocol::decode::{parse_response, ViscaResponse};
    use crate::protocol::encode::VISCA_TERMINATOR;

    let response = parse_response(frame);
    trace!("Parsed response: {:?}", response);

    match response {
        ViscaResponse::Ack { socket } => {
            if let Some(cmd_id) = scheduler.socket_command(socket) {
                debug!("ACK received for command {} on {:?}", cmd_id, socket);
                let _ = event_tx
                    .send_async(RxEvent::Ack { socket, id: cmd_id })
                    .await;

                // Don't send ACK to the response channel - wait for Completion
                // The response channel is expecting the final result, not intermediate ACKs
            } else {
                warn!("Received ACK for {:?} with no pending command", socket);
            }
        }

        ViscaResponse::Completion { socket } => {
            if let Some(cmd_id) = scheduler.socket_command(socket) {
                debug!("Completion received for command {} on {:?}", cmd_id, socket);
                let _ = event_tx
                    .send_async(RxEvent::Completion { socket, id: cmd_id })
                    .await;

                // Track successful completion
                scheduler
                    .metrics
                    .commands_completed
                    .fetch_add(1, Ordering::Relaxed);

                // Remove from retry queue if it was being retried
                scheduler.remove_from_retry_queue(cmd_id);

                // Notify the waiting command and free the socket
                if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                    debug!(
                        "Sending completion to response channel for command {}",
                        cmd_id
                    );
                    if let Err(e) =
                        response_tx.send(Ok(crate::command::response::ViscaResponse::Completion))
                    {
                        warn!("Failed to send completion to response channel: {:?}", e);
                    }
                } else {
                    warn!(
                        "No response channel found for command {} completion",
                        cmd_id
                    );
                }
                scheduler.free_socket(socket);

                // Process any queued commands now that a socket is free
                if let Err(e) =
                    process_command_queue(transport, scheduler, event_tx, executor, true).await
                {
                    error!("Error processing command queue after completion: {}", e);
                }
            } else {
                warn!(
                    "Received completion for {:?} with no pending command",
                    socket
                );
            }
        }

        ViscaResponse::DataReply { data } => {
            debug!("Data reply received: {:02X?}", data);

            // For inquiries, we need to match this with the pending inquiry
            // Since inquiries don't use sockets, we need a different mechanism
            // For now, assume the most recent inquiry is the one being responded to
            if let Some((inquiry_id, response_tx, response_type)) = scheduler.get_pending_inquiry()
            {
                let _ = event_tx
                    .send_async(RxEvent::DataReply {
                        id: inquiry_id,
                        data: data.clone(),
                    })
                    .await;

                // Parse the response based on the expected type
                let response = if let Some(response_type) = response_type {
                    // Build the full response frame with header and terminator
                    let mut full_frame = vec![0x90, 0x50];
                    full_frame.extend_from_slice(&data);
                    full_frame.push(VISCA_TERMINATOR);

                    // Parse with the expected response type
                    match crate::command::response::ViscaResponse::parse_with_type(
                        &full_frame,
                        &response_type,
                    ) {
                        Ok(parsed) => Ok(parsed),
                        Err(e) => {
                            warn!("Failed to parse inquiry response: {}", e);
                            Ok(crate::command::response::ViscaResponse::Unknown {
                                response_type: Some(response_type),
                                data: data.clone(),
                            })
                        }
                    }
                } else {
                    // No response type stored, return unknown
                    Ok(crate::command::response::ViscaResponse::Unknown {
                        response_type: None,
                        data: data.clone(),
                    })
                };

                let _ = response_tx.send(response);
            } else {
                warn!("Received data reply with no pending inquiry");
            }
        }

        ViscaResponse::Error { socket, error } => {
            warn!("Error response: {:?} on socket {:?}", error, socket);

            // Handle busy error specially - retry the command
            // Per VISCA spec, error 0x41 (NotExecutable) includes camera busy conditions
            // We map 0x41 to ViscaError::Busy in from_byte for retry logic
            if matches!(error, ViscaError::Busy) {
                eprintln!("[handle_response] Received BUSY error");
                if let Some(sock) = socket {
                    if let Some(cmd_id) = scheduler.socket_command(sock) {
                        eprintln!(
                            "[handle_response] Camera busy for command {} on {:?}, will retry",
                            cmd_id, sock
                        );
                        debug!(
                            "Camera busy for command {} on {:?}, will retry",
                            cmd_id, sock
                        );

                        // Get command metadata for retry and queue it BEFORE freeing socket
                        if let Some((bytes, priority, category)) =
                            scheduler.get_command_for_retry(cmd_id)
                        {
                            debug!(
                                "Queueing command {} for retry with priority {:?}",
                                cmd_id, priority
                            );
                            // Queue the command for retry (returns false if exhausted)
                            let now = time_utils::now_from_executor(executor);
                            let queued =
                                scheduler.queue_for_retry(cmd_id, bytes, priority, category, now);

                            if !queued {
                                debug!("Command {} exhausted retries, not queuing", cmd_id);
                                // The queue_for_retry method has already sent the error response
                            }
                        } else {
                            warn!("No metadata found for command {} to retry", cmd_id);
                        }

                        // Free the socket so it can be reused
                        // The free_socket method will check if command is queued for retry
                        scheduler.free_socket(sock);

                        // Don't send error to response channel if queued for retry
                        // The command will be retried automatically when a socket becomes available
                        // The response channel remains stored in the scheduler

                        // Process any queued commands now that a socket is free
                        // Keep consecutive_retries count, as this wasn't a successful queue operation
                        if let Err(e) =
                            process_command_queue(transport, scheduler, event_tx, executor, true)
                                .await
                        {
                            error!("Error processing command queue after busy: {}", e);
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
                        let _ = event_tx
                            .send_async(RxEvent::Error {
                                code: error,
                                socket: Some(sock),
                                id: Some(cmd_id),
                            })
                            .await;

                        // Notify the waiting command and free the socket
                        if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                            // Convert ViscaError to Error using the byte code
                            let error_code = error.to_byte();
                            let _ = response_tx.send(Err(Error::from_code(error_code)));
                        }
                        scheduler.free_socket(sock);

                        // Process any queued commands now that a socket is free
                        if let Err(e) =
                            process_command_queue(transport, scheduler, event_tx, executor, true)
                                .await
                        {
                            error!("Error processing command queue after error: {}", e);
                        }
                    }
                } else {
                    // Broadcast error with no specific socket
                    // Try to route to the most recently sent command
                    // (VISCA cameras typically send broadcast errors for the last command)

                    // Get the most recent command ID from any socket
                    let recent_cmd_id = scheduler.most_recent_command();

                    if let Some(cmd_id) = recent_cmd_id {
                        debug!("Routing broadcast error to command {}", cmd_id);
                        let _ = event_tx
                            .send_async(RxEvent::Error {
                                code: error,
                                socket: None,
                                id: Some(cmd_id),
                            })
                            .await;

                        // Notify the waiting command
                        if let Some(response_tx) = scheduler.get_response_channel(cmd_id) {
                            let error_code = error.to_byte();
                            let _ = response_tx.send(Err(Error::from_code(error_code)));
                        }

                        // Free any socket that might be associated with this command
                        scheduler.free_command_socket(cmd_id);
                    } else {
                        // No pending commands, just broadcast the error
                        let _ = event_tx
                            .send_async(RxEvent::Error {
                                code: error,
                                socket: None,
                                id: None,
                            })
                            .await;
                    }
                }
            }
        }

        ViscaResponse::NetworkChange => {
            debug!("Network change notification received");
            // Could trigger a re-initialization or status check
        }

        ViscaResponse::Unknown { data } => {
            warn!("Unknown response received: {:02X?}", data);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::decode::{parse_response, ViscaResponse};
    use crate::protocol::encode::VISCA_TERMINATOR;

    #[test]
    fn test_socket_id() {
        assert_eq!(SocketId::Socket1.as_index(), 0);
        assert_eq!(SocketId::Socket2.as_index(), 1);
    }

    #[test]
    fn test_response_parsing() {
        // Test ACK parsing
        let ack_frame = vec![0x90, 0x41, VISCA_TERMINATOR];
        let response = parse_response(&ack_frame);
        assert!(matches!(
            response,
            ViscaResponse::Ack {
                socket: SocketId::Socket1
            }
        ));

        // Test Completion parsing
        let completion_frame = vec![0x90, 0x52, VISCA_TERMINATOR];
        let response = parse_response(&completion_frame);
        assert!(matches!(
            response,
            ViscaResponse::Completion {
                socket: SocketId::Socket2
            }
        ));

        // Test Data Reply parsing
        let data_frame = vec![0x90, 0x50, 0x02, VISCA_TERMINATOR];
        let response = parse_response(&data_frame);
        assert!(matches!(response, ViscaResponse::DataReply { data } if data == vec![0x02]));

        // Test Error parsing
        let error_frame = vec![0x90, 0x61, 0x03, VISCA_TERMINATOR];
        let response = parse_response(&error_frame);
        assert!(matches!(
            response,
            ViscaResponse::Error {
                socket: Some(SocketId::Socket1),
                error: ViscaError::BufferFull
            }
        ));
    }

    #[cfg(feature = "rt-tokio")]
    #[tokio::test]
    #[allow(clippy::expect_used)]
    async fn test_runtime_loop_shutdown() {
        use bytes::Bytes;

        // Mock transport that never returns data
        struct MockTransport;

        impl AsyncTransport for MockTransport {
            async fn send(&self, _bytes: &[u8]) -> Result<(), Error> {
                Ok(())
            }

            async fn recv(&self) -> Result<Bytes, Error> {
                // Never return, simulating waiting for data
                std::future::pending().await
            }
        }

        let (submit_tx, submit_rx) = flume::unbounded();
        let (event_tx, _event_rx) = flume::unbounded();
        let (_metrics_tx, metrics_rx) = flume::unbounded();

        // Start runtime loop
        let executor = Arc::new(
            crate::executor::TokioExecutor::from_current()
                .expect("Failed to create TokioExecutor from current runtime"),
        );
        let runtime_task = runtime_loop_with_config(
            MockTransport,
            submit_rx,
            event_tx,
            metrics_rx,
            None,
            executor,
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
}
