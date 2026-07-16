//! Main runtime event loop for VISCA communication.

use flume::Receiver;
use futures_lite::future;
use tracing::{debug, error, instrument, trace, warn};

use std::sync::Arc;

use crate::{
    capabilities::Profile,
    command::{encode::ViscaCommand, system::CommandCancelCommand, CommandKind},
    error::{Error, Result},
    protocol::framer::ProtocolFramer,
    runtime::{
        async_adapter::{AsyncAdapter, ControlRequest, SubmitRequest, UrgentControlRequest},
        core::CancelOutcome,
        driver::send_one,
    },
    timeout::TimeoutConfig,
    transport::{
        buffer::BufferManager, envelope::Envelope, AsyncTransport, RetryConfig, SendSemantics,
    },
};

/// Event variants representing all possible wake sources for the runtime loop.
///
/// This enum centralizes all wake sources in a type-safe manner, preventing
/// future regressions where a new control path might be added but not included
/// in the wake set.
enum LoopEvent {
    /// A command or inquiry was submitted.
    Submit(SubmitRequest),
    /// An urgent control-plane request was received.
    UrgentControl(UrgentControlRequest),
    /// A normal control-plane request was received.
    Control(ControlRequest),
    /// Data was received from the transport.
    TransportRecv(usize),
    /// Transport receive returned an error.
    TransportErr(Error),
    /// A timer tick occurred (deadline elapsed or idle sleep).
    Tick,
    /// All control senders were dropped without a request/reply shutdown.
    ImplicitShutdown,
}

/// Runtime trace logging controlled by RUNTIME_TRACE environment variable
macro_rules! runtime_trace {
    ($($arg:tt)*) => {
        if std::env::var("RUNTIME_TRACE").is_ok() {
            eprintln!("[RUNTIME_TRACE] {}", format!($($arg)*));
        }
    };
}

/// Handle a send failure based on transport semantics.
///
/// For stream transports (TCP, Serial), a send failure can leave the byte stream
/// in an unknown state (partial write). This macro poisons the transport and
/// returns an error from the enclosing function.
///
/// For datagram transports (UDP), the individual command fails but the transport
/// can continue operating.
///
/// # Usage
/// ```ignore
/// handle_send_failure!(
///     transport,
///     adapter,
///     boundary_receivers,
///     error,
///     "context message"
/// );
/// ```
macro_rules! handle_send_failure {
    (
        $transport:expr,
        $adapter:expr,
        $boundary_receivers:expr,
        $error:expr,
        $context:expr
    ) => {
        if $transport.send_semantics() == SendSemantics::Stream {
            let reason = format!("{}: {}", $context, $error);
            error!(
                send_semantics = ?$transport.send_semantics(),
                reason = %reason,
                "Stream transport poisoned - failing all pending commands and exiting"
            );
            let failed_count = $adapter.poison_transport(reason.clone());
            debug!(failed_count, "Poisoned transport and failed pending commands");
            let runtime_error = Error::StreamPoisoned {
                reason: reason.into(),
            };
            $boundary_receivers.drain(runtime_error.clone());
            return Err(runtime_error);
        } else {
            debug!(
                send_semantics = ?$transport.send_semantics(),
                error = %$error,
                "Datagram send failed - continuing with next command"
            );
        }
    };
}

#[derive(Clone, Copy)]
struct BoundaryReceivers<'a> {
    submit: &'a Receiver<SubmitRequest>,
    urgent_control: &'a Receiver<UrgentControlRequest>,
    control: &'a Receiver<ControlRequest>,
}

impl<'a> BoundaryReceivers<'a> {
    fn new(
        submit: &'a Receiver<SubmitRequest>,
        urgent_control: &'a Receiver<UrgentControlRequest>,
        control: &'a Receiver<ControlRequest>,
    ) -> Self {
        Self {
            submit,
            urgent_control,
            control,
        }
    }

    fn drain(self, error: Error) {
        while let Ok(request) = self.submit.try_recv() {
            request.fail(error.clone());
        }

        while let Ok(request) = self.urgent_control.try_recv() {
            request.fail(error.clone());
        }

        while let Ok(request) = self.control.try_recv() {
            request.fail(error.clone());
        }
    }
}

async fn send_cancel_frame<T, Env, Ex>(
    transport: &mut T,
    executor: &Ex,
    envelope: &Env,
    send_buf: &mut bytes::BytesMut,
    camera_id: crate::camera_id::CameraId,
    socket: crate::ViscaSocket,
    write_timeout: std::time::Duration,
) -> Result<()>
where
    T: AsyncTransport,
    Env: Envelope,
    Ex: crate::executor::Executor,
{
    let cancel_cmd = CommandCancelCommand::new(socket);
    let mut temp_buf = [0u8; CommandCancelCommand::MAX_SIZE];
    let len = cancel_cmd
        .write_into(camera_id, &mut temp_buf)
        .map_err(|e| {
            error!("Failed to encode cancel command: {e}");
            e
        })?;

    envelope.frame_into(&temp_buf[..len], CommandKind::Command, send_buf);
    future::race(
        async { transport.send(&send_buf[..]).await.map(|_| ()) },
        async {
            executor.sleep(write_timeout).await;
            Err(Error::Timeout)
        },
    )
    .await?;
    debug!("Sent cancel for socket {socket:?} with camera_id {camera_id:?}");
    Ok(())
}

fn reply_or_exit_on_cancel_result<P, T, Ex>(
    transport: &mut T,
    adapter: &mut AsyncAdapter<P, Ex>,
    boundary_receivers: BoundaryReceivers<'_>,
    reply_tx: flume::Sender<Result<()>>,
    result: Result<()>,
    context: &str,
) -> Result<()>
where
    P: Profile,
    T: AsyncTransport,
    Ex: crate::executor::Executor,
{
    match result {
        Ok(()) => {
            let _ = reply_tx.send(Ok(()));
            Ok(())
        }
        Err(error) if transport.send_semantics() == SendSemantics::Stream => {
            let reason = format!("{context}: {error}");
            error!(
                send_semantics = ?transport.send_semantics(),
                reason = %reason,
                "Stream transport poisoned by cancel send failure"
            );
            let failed_count = adapter.poison_transport(reason.clone());
            debug!(
                failed_count,
                "Poisoned transport and failed pending commands"
            );
            let _ = reply_tx.send(Err(error));
            let runtime_error = Error::StreamPoisoned {
                reason: reason.into(),
            };
            boundary_receivers.drain(runtime_error.clone());
            Err(runtime_error)
        }
        Err(error) => {
            debug!(
                send_semantics = ?transport.send_semantics(),
                error = %error,
                "Datagram cancel send failed - continuing runtime loop"
            );
            let _ = reply_tx.send(Err(error));
            Ok(())
        }
    }
}

fn cleanup_shutdown<P, Ex>(
    adapter: &mut AsyncAdapter<P, Ex>,
    boundary_receivers: BoundaryReceivers<'_>,
    reply_tx: Option<flume::Sender<Result<()>>>,
) where
    P: Profile,
    Ex: crate::executor::Executor,
{
    let error = Error::RuntimeShutdown;

    let failed_count = adapter.shutdown(error.clone());
    debug!(
        failed_count,
        "Runtime shutdown failed pending command waiters"
    );

    boundary_receivers.drain(error);

    if let Some(reply_tx) = reply_tx {
        let _ = reply_tx.send(Ok(()));
    }
}

fn cleanup_runtime_termination<P, Ex>(
    adapter: &mut AsyncAdapter<P, Ex>,
    boundary_receivers: BoundaryReceivers<'_>,
    error: Error,
) where
    P: Profile,
    Ex: crate::executor::Executor,
{
    let failed_count = adapter.fail_runtime_terminated(error.clone());
    debug!(
        failed_count,
        "Runtime termination failed pending command waiters"
    );
    boundary_receivers.drain(error);
}

/// Configuration for the runtime loop.
pub struct RuntimeLoopConfig<E: Envelope> {
    pub envelope: E,
    pub buffer_manager: BufferManager,
    pub timeout_config: TimeoutConfig,
    pub retry_config: RetryConfig,
    /// Write timeout from transport config
    pub write_timeout: std::time::Duration,
    /// Maximum concurrent inquiries.
    ///
    /// For envelopes without sequence correlation (e.g., Raw VISCA), this should
    /// be set to 1 to ensure responses can be reliably matched to requests.
    /// For envelopes with sequence correlation (e.g., Sony Encapsulated), higher
    /// values enable concurrent inquiry execution.
    pub max_concurrent_inquiries: usize,
    /// Minimum time spacing between consecutive inquiry sends.
    ///
    /// Some cameras (e.g., PTZOptics) cannot process inquiries faster than
    /// ~125-150ms apart. Setting this enforces a minimum delay between sends.
    /// Default: Duration::ZERO (no artificial spacing)
    pub min_inquiry_spacing: std::time::Duration,
    /// Minimum time spacing between consecutive sends of any kind.
    ///
    /// Consumer PTZ cameras have small internal command buffers that overflow
    /// when commands are sent back-to-back at wire speed, causing spurious
    /// syntax errors and dropped completions. This interval is enforced at
    /// the transport layer for all sends (commands and inquiries).
    /// Default: Duration::ZERO (no artificial spacing)
    pub min_command_spacing: std::time::Duration,
    /// Maximum pending queue depth for admission control.
    ///
    /// This bounds the number of commands/inquiries that can be queued
    /// in the scheduler. When at capacity, new submissions are rejected
    /// with a retryable `RuntimeQueueFull` error.
    pub max_pending_queue_depth: usize,
}

/// Main runtime loop with configurable tick interval.
#[instrument(
    level = "debug",
    name = "visca_runtime_loop",
    skip(transport, submit_rx, urgent_control_rx, control_rx, executor, config)
)]
pub async fn runtime_loop_with_config<
    P: Profile + 'static,
    T: AsyncTransport + Send + 'static,
    Ex: crate::executor::Executor + Send + Sync + 'static,
>(
    mut transport: T,
    submit_rx: Receiver<SubmitRequest>,
    urgent_control_rx: Receiver<UrgentControlRequest>,
    control_rx: Receiver<ControlRequest>,
    executor: Arc<Ex>,
    config: RuntimeLoopConfig<P::Envelope>,
) -> Result<()> {
    let mut adapter = AsyncAdapter::<P, Ex>::new(
        config.timeout_config,
        config.retry_config,
        executor.clone(),
        config.max_pending_queue_depth,
    );
    adapter.set_max_inquiries_inflight(config.max_concurrent_inquiries);
    adapter.set_min_inquiry_spacing(config.min_inquiry_spacing);
    adapter.set_min_command_spacing(config.min_command_spacing);
    let mut protocol_framer = ProtocolFramer::new_with_config(config.buffer_manager.config());

    // Allocate a single reusable buffer for receiving data
    let mut read_buf = vec![0u8; config.buffer_manager.config().recv_buffer_size];

    // Allocate a single reusable buffer for sending data (zero allocation per send)
    let mut send_buf =
        bytes::BytesMut::with_capacity(config.buffer_manager.config().send_buffer_size);

    debug!("VISCA runtime started");

    let boundary_receivers = BoundaryReceivers::new(&submit_rx, &urgent_control_rx, &control_rx);

    // Default idle sleep duration when no deadlines are pending
    const DEFAULT_IDLE_SLEEP: std::time::Duration = std::time::Duration::from_millis(100);

    loop {
        runtime_trace!(
            "Loop iteration start - pending_ack: {}, can_send: {}, now: {:?}",
            adapter.pending_ack_count(),
            adapter.can_send_command(),
            executor.now()
        );

        // Compute the next deadline for time-based operations
        let now = executor.now();
        let until = adapter.next_deadline().unwrap_or(now + DEFAULT_IDLE_SLEEP);
        let sleep_dur = until.saturating_duration_since(now);

        // Await ALL wake sources in a single select point:
        // - urgent_control_rx: shutdown and cancellation
        // - submit_rx: new commands/inquiries
        // - control_rx: metrics/subscription requests
        // - transport recv or deadline tick
        //
        // This ensures new submissions/control requests wake the loop immediately,
        // regardless of how far away the next deadline is.
        let event = {
            // Build transport recv or sleep future
            let recv_or_tick = future::race(
                async {
                    match transport.recv_into(&mut read_buf).await {
                        Ok(n) => LoopEvent::TransportRecv(n),
                        // If transport emits timeout, treat as idle tick
                        Err(Error::Timeout) => LoopEvent::Tick,
                        Err(err) => LoopEvent::TransportErr(err),
                    }
                },
                async {
                    executor.sleep(sleep_dur).await;
                    LoopEvent::Tick
                },
            );

            // Race all control channels + transport
            // Use nested races since futures_lite::future::race only takes two futures
            let urgent_control_future = async {
                match urgent_control_rx.recv_async().await {
                    Ok(request) => LoopEvent::UrgentControl(request),
                    Err(_) => LoopEvent::ImplicitShutdown,
                }
            };
            let submit_future = async {
                match submit_rx.recv_async().await {
                    Ok(request) => LoopEvent::Submit(request),
                    // Channel closed - usually means all handles were dropped.
                    Err(_) => LoopEvent::ImplicitShutdown,
                }
            };
            let control_future = async {
                match control_rx.recv_async().await {
                    Ok(request) => LoopEvent::Control(request),
                    // Channel closed - treat as tick (non-fatal)
                    Err(_) => LoopEvent::Tick,
                }
            };

            // Combine all futures using nested races
            // Priority order matters for ties: urgent control > submit > normal control > recv/tick
            let event_future = future::race(
                urgent_control_future,
                future::race(submit_future, future::race(control_future, recv_or_tick)),
            );

            event_future.await
        };

        runtime_trace!(
            "Event received: {}",
            match &event {
                LoopEvent::Submit(request) => format!("Submit({:?})", request),
                LoopEvent::UrgentControl(_) => "UrgentControl".to_string(),
                LoopEvent::Control(_) => "Control".to_string(),
                LoopEvent::TransportRecv(n) => format!("TransportRecv({n})"),
                LoopEvent::TransportErr(_) => "TransportErr".to_string(),
                LoopEvent::Tick => "Tick".to_string(),
                LoopEvent::ImplicitShutdown => "ImplicitShutdown".to_string(),
            }
        );

        // Handle the event - note: NO early `continue` statements here
        // All paths fall through to the housekeeping section at the end
        match event {
            LoopEvent::ImplicitShutdown => {
                debug!("Runtime handles dropped; shutting down runtime loop");
                runtime_trace!("Implicit shutdown received; cleaning up runtime loop");
                cleanup_shutdown(&mut adapter, boundary_receivers, None);
                return Ok(());
            }

            LoopEvent::Submit(request) => {
                adapter.admit_submit(request);

                // Try to send immediately if possible
                if let Some(cmd) = adapter.next_item_to_send() {
                    if let Err(e) = send_one(
                        &mut transport,
                        &executor,
                        &mut adapter,
                        cmd,
                        &config.envelope,
                        &mut send_buf,
                        config.write_timeout,
                    )
                    .await
                    {
                        handle_send_failure!(
                            transport,
                            adapter,
                            boundary_receivers,
                            e,
                            "Send failed during submit"
                        );
                    }
                }
                // Fall through to housekeeping
            }

            LoopEvent::UrgentControl(request) => match request {
                UrgentControlRequest::Shutdown { reply_tx } => {
                    debug!("Shutdown request received; cleaning up runtime loop");
                    runtime_trace!("Shutdown request received; cleaning up runtime loop");
                    cleanup_shutdown(&mut adapter, boundary_receivers, Some(reply_tx));
                    return Ok(());
                }
                UrgentControlRequest::CancelSocket {
                    camera_id,
                    socket,
                    reply_tx,
                } => {
                    let result = send_cancel_frame(
                        &mut transport,
                        &executor,
                        &config.envelope,
                        &mut send_buf,
                        camera_id,
                        socket,
                        config.write_timeout,
                    )
                    .await;
                    reply_or_exit_on_cancel_result(
                        &mut transport,
                        &mut adapter,
                        boundary_receivers,
                        reply_tx,
                        result,
                        "Failed to send cancel for socket",
                    )?;
                }
                UrgentControlRequest::CancelById {
                    camera_id,
                    id,
                    reply_tx,
                } => match adapter.request_cancel_by_id(id) {
                    outcome @ (CancelOutcome::QueuedRemoved
                    | CancelOutcome::MarkedCancelOnAck
                    | CancelOutcome::NoOp) => {
                        trace!(
                            ?camera_id,
                            %id,
                            ?outcome,
                            "Processed cancel-by-id without immediate socket send"
                        );
                        let _ = reply_tx.send(Ok(()));
                    }
                    CancelOutcome::SendCancel { camera_id, socket } => {
                        let result = send_cancel_frame(
                            &mut transport,
                            &executor,
                            &config.envelope,
                            &mut send_buf,
                            camera_id,
                            socket,
                            config.write_timeout,
                        )
                        .await;
                        reply_or_exit_on_cancel_result(
                            &mut transport,
                            &mut adapter,
                            boundary_receivers,
                            reply_tx,
                            result,
                            "Failed to send cancel for command",
                        )?;
                    }
                },
            },

            LoopEvent::Control(request) => {
                match request {
                    #[cfg(feature = "test-utils")]
                    ControlRequest::Metrics { reply_tx } => {
                        let summary = adapter.metrics_summary();
                        let _ = reply_tx.send(Ok(summary));
                    }
                    ControlRequest::SubscribeCompletions { reply_tx } => {
                        let completion_rx = adapter.subscribe_completions();
                        let _ = reply_tx.send(Ok(completion_rx));
                    }
                }
                // Fall through to housekeeping
            }

            LoopEvent::TransportRecv(n) => {
                if n == 0 {
                    error!("Connection closed by peer; exiting runtime loop");
                    let error = Error::ConnectionClosed {
                        reason: Some(std::borrow::Cow::Borrowed("peer closed connection")),
                    };
                    cleanup_runtime_termination(&mut adapter, boundary_receivers, error.clone());
                    return Err(error);
                }

                trace!("Received {n} bytes from transport");

                // Wire-level RX logging for diagnostics
                trace!(
                    target: "grafton_visca::wire",
                    len = n,
                    hex = %crate::runtime::driver::hex_bytes(&read_buf[..n]),
                    "RX",
                );

                // Use push_slice_with_resync to handle buffer overflow gracefully.
                // This clears the buffer and retries if overflow occurs, preventing
                // permanent runtime stalls from un-framable data accumulation.
                match protocol_framer.push_slice_with_resync(&read_buf[..n]) {
                    Ok(normal_push) => {
                        if !normal_push {
                            debug!("Framer resynced after buffer overflow");
                        }
                    }
                    Err(e) => {
                        // Resync failed - chunk alone exceeds max_buffer_size.
                        // This indicates a configuration issue but we continue
                        // to allow housekeeping and future data processing.
                        warn!("Framer resync failed (chunk exceeds max_buffer_size): {e}");
                    }
                }

                // Always drain frames after push attempt (even after resync)
                for frame_result in protocol_framer.drain_frames() {
                    let frame = match frame_result {
                        Ok(frame) => frame,
                        Err(e) => {
                            warn!("Failed to extract frame: {e}");
                            continue;
                        }
                    };
                    let (payload, meta) = match config.envelope.extract_with_meta(frame) {
                        Ok(result) => result,
                        Err(e) => {
                            warn!("Failed to extract response from frame: {e}");
                            continue;
                        }
                    };

                    if let Err(e) = adapter.process_response(&payload, meta.sequence).await {
                        error!("Error processing response: {e}");
                    }
                }

                // Try to send more items after processing responses
                while let Some(cmd) = adapter.next_item_to_send() {
                    if let Err(e) = send_one(
                        &mut transport,
                        &executor,
                        &mut adapter,
                        cmd,
                        &config.envelope,
                        &mut send_buf,
                        config.write_timeout,
                    )
                    .await
                    {
                        handle_send_failure!(
                            transport,
                            adapter,
                            boundary_receivers,
                            e,
                            "Send failed while draining pending"
                        );
                    }
                }

                // Drain cancel outbox: send cancels that were queued via cancel-on-ACK
                // (when a cancel was requested before the socket was assigned, and an
                // ACK subsequently assigned the socket)
                for (camera_id, socket) in adapter.drain_cancel_outbox() {
                    if let Err(e) = send_cancel_frame(
                        &mut transport,
                        &executor,
                        &config.envelope,
                        &mut send_buf,
                        camera_id,
                        socket,
                        config.write_timeout,
                    )
                    .await
                    {
                        handle_send_failure!(
                            transport,
                            adapter,
                            boundary_receivers,
                            e,
                            "Failed to send cancel-on-ACK for socket"
                        );
                    }
                }
                // Fall through to housekeeping
            }

            LoopEvent::TransportErr(e) => {
                if matches!(e, Error::ConnectionClosed { .. }) {
                    error!("Connection closed by peer; exiting runtime loop");
                    cleanup_runtime_termination(&mut adapter, boundary_receivers, e.clone());
                    return Err(e);
                }

                error!("Error receiving from transport: {e}");
                adapter.on_network_error(e).await?;

                // Avoid hot-looping on immediate errors
                executor.sleep(std::time::Duration::from_millis(10)).await;
                // Fall through to housekeeping
            }

            LoopEvent::Tick => {
                runtime_trace!("Deadline tick with no data");
                // Fall through to housekeeping
            }
        }

        // ============================================================
        // HOUSEKEEPING: Always runs after every event (no early returns)
        // This ensures timeout/retry logic is never bypassed.
        // ============================================================

        runtime_trace!(
            "Before check_timeouts: pending_ack={}",
            adapter.pending_ack_count()
        );
        adapter.check_timeouts().await?;
        runtime_trace!(
            "After check_timeouts: pending_ack={}",
            adapter.pending_ack_count()
        );

        // Promote any retries whose backoff has elapsed back onto the send
        // queues so the drain below dispatches them through the same pacing and
        // capacity gates as fresh sends, rather than bypassing them.
        adapter.promote_ready_retries();

        // Try to send any queued items (fresh sends and promoted retries alike).
        while let Some(cmd) = adapter.next_item_to_send() {
            if let Err(e) = send_one(
                &mut transport,
                &executor,
                &mut adapter,
                cmd,
                &config.envelope,
                &mut send_buf,
                config.write_timeout,
            )
            .await
            {
                handle_send_failure!(
                    transport,
                    adapter,
                    boundary_receivers,
                    e,
                    "Send failed while draining pending after tick"
                );
            }
        }
    }
}
