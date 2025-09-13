//! Blocking runner for the protocol detection state machine.

use std::time::{Duration, Instant};

use crate::{
    capabilities::ProtocolStyle,
    command::CommandKind,
    protocol::detect::core::{Action, DetectorCore},
    transport::{
        buffer::BufferConfig, protocol_detection::DetectionResult, RetryConfig, SyncTransport,
    },
    Error,
};

/// Blocking protocol detection using the DetectorCore FSM.
///
/// This runner interprets Actions from the core FSM and performs
/// the actual blocking I/O operations.
pub(crate) fn detect_protocol_blocking<T>(
    transport: &mut T,
    protocol_style: ProtocolStyle,
    buffer_config: BufferConfig,
    retry_config: RetryConfig,
    timeout: Duration,
) -> Result<DetectionResult, Error>
where
    T: SyncTransport + ?Sized,
{
    use tracing::debug;

    // Create the detector core
    let mut core = DetectorCore::new(protocol_style, buffer_config, retry_config, timeout);

    // Build the inquiry frame once
    let inquiry = core.build_inquiry_frame();

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
                if let Err(e) = transport.send_with_kind(&inquiry, CommandKind::Inquiry) {
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
                            std::thread::sleep(d);
                            core.resume_after_pause();
                        }
                        Action::Done(res) => return Ok(res),
                        Action::NoResponse => return Ok(DetectionResult::NoResponse),
                        _ => {}
                    }
                    continue;
                }

                // Try to receive with timeout
                match transport.recv_with_timeout(remaining) {
                    Ok(recv_buffer) if !recv_buffer.is_empty() => {
                        // Received data
                        if let Some(next) = core.on_recv(&recv_buffer, Instant::now()) {
                            match next {
                                Action::Done(res) => return Ok(res),
                                Action::NoResponse => return Ok(DetectionResult::NoResponse),
                                Action::Pause(d) if !d.is_zero() => {
                                    std::thread::sleep(d);
                                    core.resume_after_pause();
                                }
                                _ => {}
                            }
                        }
                        // Continue receiving if no action returned
                    }
                    Ok(_) | Err(Error::Timeout) => {
                        // Timeout or empty response
                        match core.on_deadline(Instant::now()) {
                            Action::Pause(d) if !d.is_zero() => {
                                std::thread::sleep(d);
                                core.resume_after_pause();
                            }
                            Action::Done(res) => return Ok(res),
                            Action::NoResponse => return Ok(DetectionResult::NoResponse),
                            _ => {}
                        }
                    }
                    Err(e) => {
                        debug!("Transport error during detection: {}", e);
                        match core.on_deadline(Instant::now()) {
                            Action::Pause(d) if !d.is_zero() => {
                                std::thread::sleep(d);
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
                    std::thread::sleep(d);
                }
                core.resume_after_pause();
            }
            Action::Done(res) => return Ok(res),
            Action::NoResponse => return Ok(DetectionResult::NoResponse),
        }
    }
}
