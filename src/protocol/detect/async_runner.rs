//! Async runner for the protocol detection state machine.

use std::time::Duration;

use crate::{
    capabilities::ProtocolStyle,
    executor::Executor,
    protocol::detect::core::{Action, DetectorCore},
    protocol::detect::DetectionResult,
    transport::{buffer::BufferConfig, AsyncTransport, RetryConfig},
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

    // Scratch buffer for receiving - honor the configured buffer size
    let mut scratch = vec![0u8; buffer_config.recv_buffer_size];

    loop {
        let now = executor.now();
        match core.next_action(now) {
            Action::SendInquiry => {
                // Build with a fresh Sony sequence each try
                let inquiry = core.build_inquiry_frame();
                debug!(
                    "Sending {} bytes for {:?} detection",
                    inquiry.len(),
                    protocol_style
                );

                // Send the inquiry
                if let Err(e) = transport.send(&inquiry).await {
                    debug!("Failed to send detection command: {}", e);
                    // Move to next try
                    core.on_deadline(executor.now());
                }
            }
            Action::RecvUntil(deadline) => {
                let remaining = deadline.saturating_duration_since(executor.now());
                if remaining.is_zero() {
                    // Already past deadline
                    match core.on_deadline(executor.now()) {
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

                // Use executor's timeout abstraction
                match executor
                    .timeout(remaining, transport.recv_into(&mut scratch))
                    .await
                {
                    Ok(Ok(n)) if n > 0 => {
                        // Received data successfully
                        if let Some(next) = core.on_recv(&scratch[..n], executor.now()) {
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
                    Ok(Ok(_)) | Err(Error::Timeout) => {
                        // Zero bytes received or timeout
                        match core.on_deadline(executor.now()) {
                            Action::Pause(d) if !d.is_zero() => {
                                executor.sleep(d).await;
                                core.resume_after_pause();
                            }
                            Action::Done(res) => return Ok(res),
                            Action::NoResponse => return Ok(DetectionResult::NoResponse),
                            _ => {}
                        }
                    }
                    Ok(Err(e)) => {
                        // Transport error (non-timeout)
                        debug!("Transport error during detection: {}", e);
                        match core.on_deadline(executor.now()) {
                            Action::Pause(d) if !d.is_zero() => {
                                executor.sleep(d).await;
                                core.resume_after_pause();
                            }
                            Action::Done(res) => return Ok(res),
                            Action::NoResponse => return Ok(DetectionResult::NoResponse),
                            _ => {}
                        }
                    }
                    Err(e) if !matches!(e, Error::Timeout) => {
                        // Unexpected error from executor.timeout itself
                        debug!("Executor timeout error: {}", e);
                        match core.on_deadline(executor.now()) {
                            Action::Pause(d) if !d.is_zero() => {
                                executor.sleep(d).await;
                                core.resume_after_pause();
                            }
                            Action::Done(res) => return Ok(res),
                            Action::NoResponse => return Ok(DetectionResult::NoResponse),
                            _ => {}
                        }
                    }
                    _ => unreachable!("Handled all timeout error cases"),
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
