//! Async runner for the protocol detection state machine.

use std::time::{Duration, Instant};

use crate::{
    capabilities::ProtocolStyle,
    executor::Executor,
    protocol::detect::core::{Action, DetectorCore},
    transport::{
        buffer::BufferConfig, protocol_detection::DetectionResult, AsyncTransport, RetryConfig,
    },
    Error,
};

/// Async protocol detection using the DetectorCore FSM.
///
/// This runner interprets Actions from the core FSM and performs
/// the actual async I/O operations.
pub(crate) async fn detect_protocol_async<T, E>(
    transport: &mut T,
    executor: &E,
    protocol_style: ProtocolStyle,
    buffer_config: BufferConfig,
    retry_config: RetryConfig,
    timeout: Duration,
) -> Result<DetectionResult, Error>
where
    T: AsyncTransport,
    E: Executor,
{
    use tracing::debug;

    // Create the detector core
    let mut core = DetectorCore::new(protocol_style, buffer_config, retry_config, timeout);

    // Build the inquiry frame once
    let inquiry = core.build_inquiry_frame();

    // Scratch buffer for receiving
    let mut scratch = vec![0u8; 1024];

    loop {
        let now = Instant::now();
        match core.next_action(now) {
            Action::SendInquiry => {
                debug!(
                    "Sending {} bytes for {:?} detection",
                    inquiry.len(),
                    protocol_style
                );

                // Send the pre-built inquiry
                if let Err(e) = transport.send(&inquiry).await {
                    debug!("Failed to send detection command: {}", e);
                    // Move to next try
                    core.on_deadline(now);
                }
            }
            Action::RecvUntil(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    // Already past deadline
                    match core.on_deadline(Instant::now()) {
                        Action::Pause(d) if !d.is_zero() => {
                            executor.sleep(d).await;
                            core.resume_after_pause();
                        }
                        Action::Done(res) => return Ok(res),
                        Action::NoResponse => return Ok(DetectionResult::NoResponse),
                        _ => {}
                    }
                    continue;
                }

                // Race recv vs sleep using futures_lite
                use futures_lite::future;

                let outcome = future::or(
                    async { Ok::<_, ()>(transport.recv_into(&mut scratch).await) },
                    async {
                        executor.sleep(remaining).await;
                        Err::<_, ()>(())
                    },
                )
                .await;

                match outcome {
                    Ok(Ok(n)) if n > 0 => {
                        // Received data
                        if let Some(next) = core.on_recv(&scratch[..n], Instant::now()) {
                            match next {
                                Action::Done(res) => return Ok(res),
                                Action::NoResponse => return Ok(DetectionResult::NoResponse),
                                Action::Pause(d) if !d.is_zero() => {
                                    executor.sleep(d).await;
                                    core.resume_after_pause();
                                }
                                _ => {}
                            }
                        }
                        // Continue receiving if no action returned
                    }
                    Ok(Ok(_)) | Ok(Err(_)) | Err(()) => {
                        // Timeout or error
                        match core.on_deadline(Instant::now()) {
                            Action::Pause(d) if !d.is_zero() => {
                                executor.sleep(d).await;
                                core.resume_after_pause();
                            }
                            Action::Done(res) => return Ok(res),
                            Action::NoResponse => return Ok(DetectionResult::NoResponse),
                            _ => {}
                        }
                    }
                }
            }
            Action::Pause(d) => {
                if !d.is_zero() {
                    executor.sleep(d).await;
                }
                core.resume_after_pause();
            }
            Action::Done(res) => return Ok(res),
            Action::NoResponse => return Ok(DetectionResult::NoResponse),
        }
    }
}
