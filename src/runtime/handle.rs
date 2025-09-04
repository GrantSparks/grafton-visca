//! RuntimeHandle implementation for VISCA communication.

use flume::Sender;
use tracing::{debug, instrument};

use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

use super::{
    loop_task::runtime_loop_with_config,
    scheduler::{MetricsSummary, Priority, TxItem},
};
use crate::{
    capabilities::ProtocolStyle,
    command::response::ViscaResponse,
    error::{Error, Result},
    transport::{
        buffer::{BufferConfig, BufferManager},
        envelope::TransportEnvelope,
        AsyncTransport,
    },
    ViscaSocket,
};

/// Helper function to spawn runtime tasks properly for different executor types.
#[instrument(level = "debug", skip(executor, runtime_task))]
fn spawn_runtime_task_properly<E: crate::executor::Executor>(
    executor: &E,
    runtime_task: impl std::future::Future<Output = Result<(), Error>> + Send + 'static,
) {
    // Spawn the runtime loop as a background task
    debug!("Spawning runtime task using spawn_bg");
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

impl RuntimeHandle {
    /// Create a new camera runtime with the given transport using raw VISCA protocol.
    ///
    /// This spawns a background task to handle communication with the camera.
    /// For compatibility, this defaults to raw VISCA protocol.
    pub async fn new<T: AsyncTransport + Send + 'static, E: crate::executor::Executor>(
        transport: T,
        executor: Arc<E>,
    ) -> Result<Self> {
        Self::new_with_style(transport, executor, ProtocolStyle::RawVisca).await
    }

    /// Create a new runtime handle with a transport and executor.
    ///
    /// This is an alias for `new` to match the expected API used by AsyncCamera.
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

        spawn_runtime_task_properly(executor.as_ref(), runtime_task);

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
    #[cfg(all(feature = "async", feature = "rt-tokio"))]
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
        // Encode the command into a stack buffer
        let mut stack_buffer = vec![0u8; C::MAX_SIZE];
        let len = cmd.encode_into(camera_id, &mut stack_buffer)?;
        let bytes = bytes::Bytes::copy_from_slice(&stack_buffer[..len]);

        // Generate command ID
        let command_id = self.next_command_id.fetch_add(1, Ordering::Relaxed);

        // Create response channel
        let (response_tx, response_rx) = flume::bounded(1);

        // Create the TxItem
        let item = TxItem::Command {
            id: command_id,
            bytes,
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
        // Encode the inquiry into a stack buffer
        let mut stack_buffer = vec![0u8; I::MAX_SIZE];
        let len = inquiry.encode_into(camera_id, &mut stack_buffer)?;
        let bytes = bytes::Bytes::copy_from_slice(&stack_buffer[..len]);

        // Create response channel
        let (response_tx, response_rx) = flume::bounded(1);

        // Get the expected response type from the inquiry
        let response_type = inquiry.response_type();

        // Create the TxItem
        let item = TxItem::Inquiry {
            id: 0, // Will be assigned by scheduler
            bytes,
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
