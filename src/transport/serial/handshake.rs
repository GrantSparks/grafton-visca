//! Runtime-agnostic serial handshake module for VISCA communication.
//!
//! This module provides unified handshake logic for both async and blocking
//! serial transports, including I/F Clear and Address Set operations.
//!
//! The handshake process is protocol-aware and uses the existing ProtocolFramer
//! for robust frame handling instead of manual buffer scanning.
//!
//! Address-set discovery retains only the final camera count reported by the
//! bus. The protocol framer bounds incomplete wire data by the serial
//! `BufferConfig` limit (8 KiB); no second response-history buffer is kept by
//! either loop.

#[cfg(any(
    feature = "transport-serial-tokio",
    all(feature = "blocking", feature = "transport-serial")
))]
use tracing::warn;
#[cfg(any(
    feature = "transport-serial-tokio",
    all(feature = "blocking", feature = "transport-serial"),
    test
))]
use tracing::{debug, trace};

#[cfg(feature = "transport-serial-tokio")]
use std::time::Duration;

#[cfg(any(
    feature = "transport-serial-tokio",
    all(feature = "blocking", feature = "transport-serial"),
    test
))]
use crate::command::bytes::VISCA_TERMINATOR;
#[cfg(any(
    feature = "transport-serial-tokio",
    all(feature = "blocking", feature = "transport-serial")
))]
use crate::{
    camera_id::CameraId,
    command::{
        encode::WireEncode,
        system::{AddressSetCommand, InterfaceClearCommand},
    },
    error::{Error, Result},
    protocol::framer::{FramingMode, ProtocolFramer},
    transport::buffer::BufferConfig,
};

/// Result of parsing address set response bytes.
#[cfg(any(
    feature = "transport-serial-tokio",
    all(feature = "blocking", feature = "transport-serial"),
    test
))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseOutcome {
    /// Partial response received, more data needed.
    Partial {
        /// No final camera count has been observed yet.
        camera_count: u8,
    },
    /// Address set complete.
    Complete {
        /// Total number of cameras discovered.
        camera_count: u8,
    },
}

/// Parse address set response bytes from the buffer.
///
/// This function processes VISCA address set responses:
/// - Final address-set reply: `88 30 0p FF`, where `p` is the final assigned
///   device address plus one.
/// - Network Change: `z0 38 FF`, which is a separate notification and is not
///   an address-set completion.
///
/// Returns `ParseOutcome::Partial` if more data is needed,
/// or `ParseOutcome::Complete` when the address set is finished.
#[cfg(any(
    feature = "transport-serial-tokio",
    all(feature = "blocking", feature = "transport-serial"),
    test
))]
pub fn parse_address_set_bytes(buf: &[u8]) -> ParseOutcome {
    let mut i = 0;

    while i < buf.len() {
        // Address Set returns the broadcast header 0x88. `p` is not a marker:
        // it is the final assigned address plus one, giving the camera count
        // directly as `p - 1`.
        if i + 3 < buf.len()
            && buf[i] == 0x88
            && buf[i + 1] == 0x30
            && buf[i + 3] == VISCA_TERMINATOR
        {
            let next_address = buf[i + 2];
            if (0x02..=0x08).contains(&next_address) {
                let camera_count = next_address - 1;
                debug!("Address Set complete, {camera_count} cameras found");
                return ParseOutcome::Complete { camera_count };
            }
        }

        i += 1;
    }

    ParseOutcome::Partial { camera_count: 0 }
}

const SERIAL_HANDSHAKE_READ_SIZE: usize = 64;

/// Scalar state carried across framed address-set responses.
///
/// An address-set reply contains the final count rather than one response per
/// camera. Keeping only that count makes discovery state constant-sized even
/// when a serial device sends arbitrary amounts of noise or repeated frames.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AddressSetState {
    camera_count: u8,
}

impl AddressSetState {
    fn observe(&mut self, frame: &[u8]) -> Option<u8> {
        match parse_address_set_bytes(frame) {
            ParseOutcome::Complete { camera_count } => {
                self.camera_count = camera_count;
                Some(camera_count)
            }
            ParseOutcome::Partial { .. } => None,
        }
    }

    fn camera_count(self) -> u8 {
        self.camera_count
    }
}

/// Feed one bounded read chunk through the shared serial framing/discovery
/// path. `ProtocolFramer` resynchronizes on overflow, while `AddressSetState`
/// retains only a scalar count.
fn process_address_set_chunk(
    framer: &mut ProtocolFramer,
    state: &mut AddressSetState,
    chunk: &[u8],
) -> Result<Option<u8>> {
    framer.push_slice_with_resync(chunk)?;

    for frame_result in framer.drain_frames() {
        let frame = frame_result?;
        if let Some(camera_count) = state.observe(&frame) {
            return Ok(Some(camera_count));
        }
    }

    Ok(None)
}

// Async handshake functions (feature-gated for tokio-serial)
#[cfg(feature = "transport-serial-tokio")]
pub mod async_handshake {
    use std::time::Instant;

    use super::*;
    use crate::{
        executor::Executor,
        transport::async_io::{AsyncReadExt, AsyncWriteExt},
    };

    // Address Set has always used a two-second attempt budget. I/F Clear has
    // no caller-supplied budget, so give its complete write/flush/settle
    // operation the same finite startup bound.
    const IF_CLEAR_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);
    const IF_CLEAR_SETTLE_DELAY: Duration = Duration::from_millis(100);
    const ADDRESS_SET_RETRY_DELAY: Duration = Duration::from_millis(100);
    const ADDRESS_SET_IDLE_PAUSE: Duration = Duration::from_millis(10);

    /// Return the unspent portion of one fixed attempt budget.
    ///
    /// The start instant is sampled once, before the attempt's write. Every
    /// later write, flush, read, and pacing pause uses the same clock origin so
    /// partial or noisy input cannot buy another full timeout.
    fn remaining_attempt_budget<E>(
        exec: &E,
        attempt_started: Instant,
        attempt_budget: Duration,
    ) -> Result<Duration>
    where
        E: Executor,
    {
        let elapsed = exec.now().saturating_duration_since(attempt_started);
        let remaining = attempt_budget.saturating_sub(elapsed);

        if remaining.is_zero() {
            Err(Error::Timeout)
        } else {
            Ok(remaining)
        }
    }

    /// Bound both halves of a serial write by the remaining attempt budget.
    async fn write_all_and_flush_within_attempt<E, S>(
        exec: &E,
        io: &mut S,
        bytes: &[u8],
        attempt_started: Instant,
        attempt_budget: Duration,
    ) -> Result<()>
    where
        E: Executor,
        S: AsyncWriteExt + Send + ?Sized,
    {
        let remaining = remaining_attempt_budget(exec, attempt_started, attempt_budget)?;
        exec.timeout(remaining, io.write_all(bytes)).await??;

        let remaining = remaining_attempt_budget(exec, attempt_started, attempt_budget)?;
        exec.timeout(remaining, io.flush()).await??;
        Ok(())
    }

    /// Yield between unsuccessful reads without extending the attempt.
    ///
    /// This is deliberately used after partial/noisy chunks as well as empty
    /// reads. Besides avoiding a hot loop, it lets a deterministic executor
    /// advance to the fixed attempt deadline when a synthetic stream is always
    /// immediately readable but never yields a valid Address Set reply.
    async fn pause_before_next_read<E>(
        exec: &E,
        attempt_started: Instant,
        attempt_budget: Duration,
    ) -> Result<()>
    where
        E: Executor,
    {
        let remaining = remaining_attempt_budget(exec, attempt_started, attempt_budget)?;
        exec.sleep(ADDRESS_SET_IDLE_PAUSE.min(remaining)).await;
        Ok(())
    }

    /// Whether a transport read means it simply had no bytes to offer.
    fn read_reported_no_data(error: &Error) -> bool {
        matches!(error, Error::Timeout)
            || matches!(
                error,
                Error::Io(io_error)
                    if matches!(
                        io_error.kind(),
                        std::io::ErrorKind::TimedOut
                            | std::io::ErrorKind::WouldBlock
                            | std::io::ErrorKind::Interrupted
                    )
            )
    }

    /// Send I/F Clear command to reset all devices on the bus.
    ///
    /// This is executor-driven and runtime-agnostic.
    pub async fn if_clear_async<E, S>(exec: &E, io: &mut S) -> Result<()>
    where
        E: Executor,
        S: AsyncWriteExt + Send + ?Sized,
    {
        debug!("Sending I/F Clear command");
        let cmd = InterfaceClearCommand::new();
        let mut buffer = [0u8; 16];

        // InterfaceClearCommand is const-constructed and guaranteed to encode
        let len = cmd
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .map_err(|e| Error::TransportError(format!("Failed to encode IF Clear: {e}").into()))?;

        let attempt_started = exec.now();
        write_all_and_flush_within_attempt(
            exec,
            io,
            &buffer[..len],
            attempt_started,
            IF_CLEAR_OPERATION_TIMEOUT,
        )
        .await?;

        // Keep the required settle delay inside the same bounded startup
        // operation rather than allowing a stalled write to consume it all.
        let remaining =
            remaining_attempt_budget(exec, attempt_started, IF_CLEAR_OPERATION_TIMEOUT)?;
        if remaining < IF_CLEAR_SETTLE_DELAY {
            return Err(Error::Timeout);
        }
        exec.timeout(remaining, exec.sleep(IF_CLEAR_SETTLE_DELAY))
            .await?;
        Ok(())
    }

    /// Send Address Set command to assign addresses to devices.
    ///
    /// Returns the number of cameras detected.
    /// This is executor-driven and runtime-agnostic.
    pub async fn address_set_async<E, S>(exec: &E, io: &mut S, timeout: Duration) -> Result<u8>
    where
        E: Executor,
        S: AsyncReadExt + AsyncWriteExt + Send + ?Sized,
    {
        let max_attempts = 3;

        for attempt in 0..max_attempts {
            debug!("Address Set attempt {}", attempt + 1);
            let attempt_started = exec.now();
            let cmd = AddressSetCommand::new();
            let mut buffer = [0u8; 16];

            // AddressSetCommand is const-constructed and guaranteed to encode
            let len = cmd
                .write_into(CameraId::CAMERA_1, &mut buffer)
                .map_err(|e| {
                    Error::TransportError(format!("Failed to encode Address Set: {e}").into())
                })?;

            // A timed-out write is a send failure, not an absent reply: after
            // cancellation its stream position may be unknowable, so surface
            // it immediately rather than retrying on the same serial stream.
            write_all_and_flush_within_attempt(exec, io, &buffer[..len], attempt_started, timeout)
                .await?;

            match recv_address_set_response_async(exec, io, attempt_started, timeout).await {
                Ok(camera_count) => {
                    debug!("Address Set successful, found {camera_count} cameras");
                    return Ok(camera_count);
                }
                Err(Error::Timeout) if attempt < max_attempts - 1 => {
                    warn!("Address Set timeout, retrying...");
                    exec.sleep(ADDRESS_SET_RETRY_DELAY).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::MaxRetriesExceeded)
    }

    /// Receive and parse Address Set response using executor-driven timeout.
    ///
    /// # Clock-Agnostic Design
    ///
    /// This function uses `exec.now()` for all time measurements, ensuring
    /// compatibility with deterministic executors that use virtual time.
    async fn recv_address_set_response_async<E, S>(
        exec: &E,
        stream: &mut S,
        attempt_started: Instant,
        timeout_duration: Duration,
    ) -> Result<u8>
    where
        E: Executor,
        S: AsyncReadExt + Send + ?Sized,
    {
        // Use ProtocolFramer for robust frame handling
        // Serial VISCA has no Sony envelope; raw framing remains authoritative
        // even when noise begins with Sony payload-type bytes.
        let mut framer = ProtocolFramer::new_with_config_and_mode(
            BufferConfig::for_serial(),
            FramingMode::RawVisca,
        );
        let mut state = AddressSetState::default();

        loop {
            let remaining = match remaining_attempt_budget(exec, attempt_started, timeout_duration)
            {
                Ok(remaining) => remaining,
                Err(Error::Timeout) => break,
                Err(error) => return Err(error),
            };
            let mut temp_buf = [0u8; SERIAL_HANDSHAKE_READ_SIZE];
            match exec.timeout(remaining, stream.read(&mut temp_buf)).await {
                Ok(Ok(n)) if n > 0 => {
                    trace!("Address Set response: {:02X?}", &temp_buf[..n]);

                    if let Some(camera_count) =
                        process_address_set_chunk(&mut framer, &mut state, &temp_buf[..n])?
                    {
                        return Ok(camera_count);
                    }

                    match pause_before_next_read(exec, attempt_started, timeout_duration).await {
                        Ok(()) => {}
                        Err(Error::Timeout) => break,
                        Err(error) => return Err(error),
                    }
                }
                Ok(Ok(_)) => {
                    match pause_before_next_read(exec, attempt_started, timeout_duration).await {
                        Ok(()) => {}
                        Err(Error::Timeout) => break,
                        Err(error) => return Err(error),
                    }
                }
                Ok(Err(error)) if read_reported_no_data(&error) => {
                    match pause_before_next_read(exec, attempt_started, timeout_duration).await {
                        Ok(()) => {}
                        Err(Error::Timeout) => break,
                        Err(error) => return Err(error),
                    }
                }
                Ok(Err(error)) | Err(error) => return Err(error),
            }
        }

        // Total timeout reached - check if we got any cameras
        match state.camera_count() {
            camera_count if camera_count > 0 => {
                debug!(
                    "Address Set timeout reached, but {} cameras were found",
                    camera_count
                );
                Ok(camera_count)
            }
            _ => {
                debug!("Address Set timeout - no cameras found");
                Err(Error::Timeout)
            }
        }
    }

    #[cfg(all(test, feature = "test-utils"))]
    mod tests {
        use std::{collections::VecDeque, future::Future, sync::Arc, time::Duration};

        use super::*;
        use crate::{testing::testkit::DeterministicExecutor, transport::async_io::AsyncReadExt};

        /// A serial reader whose next read never resolves.
        struct PendingRead;

        impl AsyncReadExt for PendingRead {
            fn read(&mut self, _buf: &mut [u8]) -> impl Future<Output = Result<usize>> + Send {
                std::future::pending()
            }
        }

        /// A reader that provides selected chunks after executor-clock delays,
        /// then remains pending. It lets the test prove that a partial chunk
        /// does not restart the overall attempt deadline.
        struct DelayedRead {
            executor: Arc<DeterministicExecutor>,
            chunks: VecDeque<(Duration, Vec<u8>)>,
        }

        impl DelayedRead {
            fn new(
                executor: Arc<DeterministicExecutor>,
                chunks: impl IntoIterator<Item = (Duration, Vec<u8>)>,
            ) -> Self {
                Self {
                    executor,
                    chunks: chunks.into_iter().collect(),
                }
            }
        }

        impl AsyncReadExt for DelayedRead {
            async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
                let Some((delay, chunk)) = self.chunks.pop_front() else {
                    return std::future::pending().await;
                };

                self.executor.sleep(delay).await;
                buf[..chunk.len()].copy_from_slice(&chunk);
                Ok(chunk.len())
            }
        }

        #[test]
        fn pending_read_expires_at_the_deterministic_attempt_deadline() {
            let (executor, clock) = DeterministicExecutor::new();
            let timeout = Duration::from_secs(2);
            let attempt_started = executor.now();
            let mut stream = PendingRead;

            let result = executor.run_until(recv_address_set_response_async(
                executor.as_ref(),
                &mut stream,
                attempt_started,
                timeout,
            ));

            assert!(matches!(result, Err(Error::Timeout)));
            assert_eq!(
                clock.now().saturating_duration_since(attempt_started),
                timeout
            );
        }

        #[test]
        fn partial_input_does_not_restart_the_attempt_deadline() {
            let (executor, clock) = DeterministicExecutor::new();
            let timeout = Duration::from_millis(100);
            let attempt_started = executor.now();
            let mut stream = DelayedRead::new(
                executor.clone(),
                [
                    // This is an incomplete Address Set reply at t=60ms.
                    (Duration::from_millis(60), vec![0x88, 0x30]),
                    // After the t=60ms partial chunk (and its pacing pause),
                    // this would complete after t=100ms. It must lose to the
                    // original deadline rather than receiving a fresh budget.
                    (Duration::from_millis(60), vec![0x08, VISCA_TERMINATOR]),
                ],
            );

            let result = executor.run_until(recv_address_set_response_async(
                executor.as_ref(),
                &mut stream,
                attempt_started,
                timeout,
            ));

            assert!(matches!(result, Err(Error::Timeout)));
            assert_eq!(
                clock.now().saturating_duration_since(attempt_started),
                timeout
            );
        }
    }
}

// Blocking handshake functions
#[cfg(all(feature = "blocking", feature = "transport-serial"))]
pub mod blocking_handshake {
    use std::{
        io::ErrorKind,
        time::{Duration, Instant},
    };

    use super::*;

    // Keep the blocking path aligned with the async handshake: Address Set has
    // one fixed budget per attempt, while I/F Clear has a bounded startup
    // operation including its required settle delay.
    const IF_CLEAR_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);
    const IF_CLEAR_SETTLE_DELAY: Duration = Duration::from_millis(100);
    const ADDRESS_SET_RETRY_DELAY: Duration = Duration::from_millis(100);
    const ADDRESS_SET_IDLE_PAUSE: Duration = Duration::from_millis(10);

    /// A scoped serial-port timeout change for one blocking handshake I/O
    /// operation.
    ///
    /// `serialport` exposes one timeout setting for both reads and writes. A
    /// guard lets Address Set temporarily cap a read to its remaining attempt
    /// budget, or apply the configured write timeout, without leaving either
    /// setting behind for the transport's normal operation.
    struct HandshakeTimeoutGuard<'a> {
        port: &'a mut dyn serialport::SerialPort,
        original_timeout: Duration,
        restored: bool,
    }

    impl<'a> HandshakeTimeoutGuard<'a> {
        fn new(port: &'a mut dyn serialport::SerialPort, timeout: Duration) -> Result<Self> {
            let original_timeout = port.timeout();
            port.set_timeout(timeout).map_err(|error| {
                Error::TransportError(
                    format!("Failed to set serial handshake timeout: {error}").into(),
                )
            })?;

            Ok(Self {
                port,
                original_timeout,
                restored: false,
            })
        }

        fn port_mut(&mut self) -> &mut dyn serialport::SerialPort {
            self.port
        }

        /// Restore the caller's timeout and report a failure to do so.
        ///
        /// A normal, completed operation must not silently leave the port in
        /// its temporary handshake configuration. Drop remains a fallback for
        /// unwinding and error paths that cannot return a second error.
        fn restore(&mut self) -> Result<()> {
            if self.restored {
                return Ok(());
            }

            self.port
                .set_timeout(self.original_timeout)
                .map_err(|error| {
                    Error::TransportError(
                        format!("Failed to restore serial handshake timeout: {error}").into(),
                    )
                })?;
            self.restored = true;
            Ok(())
        }
    }

    impl Drop for HandshakeTimeoutGuard<'_> {
        fn drop(&mut self) {
            // Successful operations restore explicitly. This is only the
            // fallback for unwinding or another error path, where a second
            // restoration failure cannot replace the primary error.
            if !self.restored {
                if let Err(error) = self.port.set_timeout(self.original_timeout) {
                    trace!("Failed to restore serial handshake timeout: {error}");
                }
            }
        }
    }

    /// Run one serial I/O operation under a scoped timeout.
    ///
    /// The result deliberately remains an `io::Result` inside the crate
    /// result: a read timeout still needs its normal no-data classification,
    /// while a successful restoration failure is surfaced as a transport
    /// error. If the operation itself returns an error, restoration is still
    /// explicitly attempted before that raw result is returned.
    fn with_scoped_timeout<T>(
        port: &mut dyn serialport::SerialPort,
        timeout: Duration,
        operation: impl FnOnce(&mut dyn serialport::SerialPort) -> std::io::Result<T>,
    ) -> Result<std::io::Result<T>> {
        let mut guard = HandshakeTimeoutGuard::new(port, timeout)?;
        let operation_result = operation(guard.port_mut());
        guard.restore()?;
        Ok(operation_result)
    }

    /// Return the unspent portion of one fixed attempt budget.
    fn remaining_attempt_budget(
        attempt_started: Instant,
        attempt_budget: Duration,
    ) -> Result<Duration> {
        let remaining = attempt_budget.saturating_sub(attempt_started.elapsed());

        if remaining.is_zero() {
            Err(Error::Timeout)
        } else {
            Ok(remaining)
        }
    }

    /// Queue a complete serial write without allowing a low-level write to
    /// start after the current handshake attempt has expired.
    ///
    /// `serialport::SerialPort::flush` maps to `tcdrain` on POSIX and
    /// `FlushFileBuffers` on Windows, neither of which can be reliably
    /// bounded after it has begun. The handshake does not need to wait for the
    /// transmit queue to drain: Address Set naturally waits for the resulting
    /// reply, and I/F Clear has its protocol settle interval after submission.
    /// Avoiding that drain keeps the supplied attempt deadline authoritative.
    fn write_within_attempt(
        io: &mut dyn serialport::SerialPort,
        bytes: &[u8],
        attempt_started: Instant,
        attempt_budget: Duration,
        configured_write_timeout: Duration,
    ) -> Result<()> {
        let mut written = 0;

        while written < bytes.len() {
            // A `Write::write` may make partial progress. Re-sample before
            // every follow-up syscall so one slow partial write cannot grant a
            // fresh configured timeout to the next one.
            let remaining = remaining_attempt_budget(attempt_started, attempt_budget)?;
            let write_result =
                with_scoped_timeout(io, configured_write_timeout.min(remaining), |port| {
                    port.write(&bytes[written..])
                })?;

            match write_result {
                Ok(0) => {
                    return Err(Error::TransportError(
                        "Serial write made no progress".into(),
                    ));
                }
                Ok(count) if count <= bytes.len() - written => {
                    written += count;
                }
                Ok(count) => {
                    return Err(Error::TransportError(
                        format!(
                            "Serial write reported {count} bytes for a {}-byte buffer",
                            bytes.len() - written
                        )
                        .into(),
                    ));
                }
                Err(error) => {
                    return Err(Error::TransportError(
                        format!("Serial write error: {error}").into(),
                    ));
                }
            }
        }

        // A final low-level write may have consumed the whole budget. Do not
        // let a later receive begin with a fresh deadline.
        remaining_attempt_budget(attempt_started, attempt_budget)?;
        Ok(())
    }

    /// Pause between no-data reads without granting the attempt extra time.
    fn pause_before_next_read(attempt_started: Instant, attempt_budget: Duration) -> Result<()> {
        let remaining = remaining_attempt_budget(attempt_started, attempt_budget)?;
        std::thread::sleep(ADDRESS_SET_IDLE_PAUSE.min(remaining));
        Ok(())
    }

    /// Whether a serial read simply had no bytes to offer.
    fn read_reported_no_data(error: &std::io::Error) -> bool {
        matches!(
            error.kind(),
            ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted
        )
    }

    /// Send I/F Clear command to reset all devices on the bus (blocking).
    pub fn if_clear_blocking(
        io: &mut dyn serialport::SerialPort,
        configured_write_timeout: Duration,
    ) -> Result<()> {
        debug!("Sending I/F Clear command");
        let cmd = InterfaceClearCommand::new();
        let mut buffer = [0u8; 16];

        // InterfaceClearCommand is const-constructed and guaranteed to encode
        let len = cmd
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .map_err(|e| Error::TransportError(format!("Failed to encode IF Clear: {e}").into()))?;

        let attempt_started = Instant::now();
        write_within_attempt(
            io,
            &buffer[..len],
            attempt_started,
            IF_CLEAR_OPERATION_TIMEOUT,
            configured_write_timeout,
        )?;

        // Keep the required settle delay within the same bounded startup
        // operation rather than allowing a stalled write to consume it all.
        if remaining_attempt_budget(attempt_started, IF_CLEAR_OPERATION_TIMEOUT)?
            < IF_CLEAR_SETTLE_DELAY
        {
            return Err(Error::Timeout);
        }
        std::thread::sleep(IF_CLEAR_SETTLE_DELAY);
        Ok(())
    }

    /// Send Address Set command to assign addresses to devices (blocking).
    ///
    /// Returns the number of cameras detected.
    pub fn address_set_blocking(
        io: &mut dyn serialport::SerialPort,
        timeout: Duration,
        configured_write_timeout: Duration,
    ) -> Result<u8> {
        let max_attempts = 3;

        for attempt in 0..max_attempts {
            debug!("Address Set attempt {}", attempt + 1);
            let attempt_started = Instant::now();
            let cmd = AddressSetCommand::new();
            let mut buffer = [0u8; 16];

            // AddressSetCommand is const-constructed and guaranteed to encode
            let len = cmd
                .write_into(CameraId::CAMERA_1, &mut buffer)
                .map_err(|e| {
                    Error::TransportError(format!("Failed to encode Address Set: {e}").into())
                })?;

            // A timed-out write is a send failure, not an absent reply: the
            // stream may have advanced, so preserve the error rather than
            // retrying it as an Address Set receive timeout.
            write_within_attempt(
                io,
                &buffer[..len],
                attempt_started,
                timeout,
                configured_write_timeout,
            )?;

            match recv_address_set_response_blocking(io, attempt_started, timeout) {
                Ok(camera_count) => {
                    debug!("Address Set successful, found {camera_count} cameras");
                    return Ok(camera_count);
                }
                Err(Error::Timeout) if attempt < max_attempts - 1 => {
                    warn!("Address Set timeout, retrying...");
                    std::thread::sleep(ADDRESS_SET_RETRY_DELAY);
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::MaxRetriesExceeded)
    }

    /// Receive and parse Address Set response (blocking).
    fn recv_address_set_response_blocking(
        stream: &mut dyn serialport::SerialPort,
        attempt_started: Instant,
        timeout: Duration,
    ) -> Result<u8> {
        // Use ProtocolFramer for robust frame handling
        // Serial VISCA has no Sony envelope; raw framing remains authoritative
        // even when noise begins with Sony payload-type bytes.
        let mut framer = ProtocolFramer::new_with_config_and_mode(
            BufferConfig::for_serial(),
            FramingMode::RawVisca,
        );
        let mut state = AddressSetState::default();
        let mut temp_buf = [0u8; SERIAL_HANDSHAKE_READ_SIZE];

        loop {
            let remaining = match remaining_attempt_budget(attempt_started, timeout) {
                Ok(remaining) => remaining,
                Err(Error::Timeout) => break,
                Err(error) => return Err(error),
            };

            // The port's normal timeout can be shorter than an attempt (in
            // which case its early timeout is just idle no-data), or longer
            // than the remaining budget. Cap each read to both bounds and
            // restore the user's setting before processing the result.
            let read_timeout = stream.timeout().min(remaining);
            let read_result =
                with_scoped_timeout(stream, read_timeout, |port| port.read(&mut temp_buf))?;

            match read_result {
                Ok(n) if n > 0 => {
                    trace!("Address Set response: {:02X?}", &temp_buf[..n]);

                    if let Some(camera_count) =
                        process_address_set_chunk(&mut framer, &mut state, &temp_buf[..n])?
                    {
                        return Ok(camera_count);
                    }

                    match pause_before_next_read(attempt_started, timeout) {
                        Ok(()) => {}
                        Err(Error::Timeout) => break,
                        Err(error) => return Err(error),
                    }
                }
                Ok(_) => match pause_before_next_read(attempt_started, timeout) {
                    Ok(()) => {}
                    Err(Error::Timeout) => break,
                    Err(error) => return Err(error),
                },
                Err(error) if read_reported_no_data(&error) => {
                    // A per-read timeout means no frame was consumed. Keep
                    // the one attempt deadline active and try again.
                    match pause_before_next_read(attempt_started, timeout) {
                        Ok(()) => {}
                        Err(Error::Timeout) => break,
                        Err(error) => return Err(error),
                    }
                }
                Err(error) => {
                    return Err(Error::TransportError(
                        format!("Error reading Address Set response: {error}").into(),
                    ));
                }
            }
        }

        // Total timeout reached
        match state.camera_count() {
            camera_count if camera_count > 0 => {
                debug!(
                    "Address Set timeout reached, but {} cameras were found",
                    camera_count
                );
                Ok(camera_count)
            }
            _ => {
                debug!("Address Set timeout - no cameras found");
                Err(Error::Timeout)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_address_set_single_camera() {
        // Final assigned address is 1, so p is 2.
        let data = [0x88, 0x30, 0x02, VISCA_TERMINATOR];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 1 });
    }

    #[test]
    fn test_parse_address_set_multiple_cameras() {
        // Three cameras: the final assigned address is 3, so p is 4.
        let data = [0x88, 0x30, 0x04, VISCA_TERMINATOR];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 3 });
    }

    #[test]
    fn test_parse_address_set_seven_cameras() {
        // Seven cameras: the final assigned address is 7, so p is 8.
        let data = [0x88, 0x30, 0x08, VISCA_TERMINATOR];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 7 });
    }

    #[test]
    fn test_parse_address_set_ignores_network_change_and_noise() {
        // Network Change is z0 38 FF, not an Address Set completion. A z0
        // header must not be accepted even when the remaining bytes resemble
        // an Address Set reply.
        let data = [
            0x00,
            0x11, // Noise
            0x90,
            0x38,
            VISCA_TERMINATOR, // Network Change
            0x90,
            0x30,
            0x02,
            VISCA_TERMINATOR, // Not an Address Set reply: source is z0, not 88
            0xAA,
            0xBB, // More noise
            0x88,
            0x30,
            0x04,
            VISCA_TERMINATOR, // Three-camera Address Set reply
        ];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 3 });

        assert_eq!(
            parse_address_set_bytes(&[0x90, 0x38, VISCA_TERMINATOR]),
            ParseOutcome::Partial { camera_count: 0 }
        );
        assert_eq!(
            parse_address_set_bytes(&[0x90, 0x30, 0x02, VISCA_TERMINATOR]),
            ParseOutcome::Partial { camera_count: 0 }
        );
    }

    #[test]
    fn test_parse_address_set_partial_frame() {
        // Incomplete frame
        let data = [0x88, 0x30, 0x08]; // Missing terminator
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Partial { camera_count: 0 });
    }

    #[test]
    fn test_parse_address_set_empty() {
        let data = [];
        let result = parse_address_set_bytes(&data);
        assert_eq!(result, ParseOutcome::Partial { camera_count: 0 });
    }

    #[test]
    fn test_parse_address_set_fragmented() {
        // Test parsing in fragments
        let mut accumulated = Vec::new();

        // Fragment 1
        accumulated.extend_from_slice(&[0x88, 0x30]);
        let result = parse_address_set_bytes(&accumulated);
        assert_eq!(result, ParseOutcome::Partial { camera_count: 0 });

        // Fragment 2
        accumulated.extend_from_slice(&[0x08, VISCA_TERMINATOR]);
        let result = parse_address_set_bytes(&accumulated);
        assert_eq!(result, ParseOutcome::Complete { camera_count: 7 });
    }

    #[test]
    fn address_set_state_is_scalar_under_repeated_and_noisy_input() {
        use std::mem::size_of;

        assert_eq!(size_of::<AddressSetState>(), size_of::<u8>());

        let mut state = AddressSetState::default();
        let unrelated = [0x55u8; 4096];
        for _ in 0..128 {
            assert_eq!(state.observe(&unrelated), None);
        }
        assert_eq!(state.camera_count(), 0);

        let one_camera = [0x88, 0x30, 0x02, VISCA_TERMINATOR];
        assert_eq!(state.observe(&one_camera), Some(1));
        assert_eq!(state.camera_count(), 1);

        let network_change = [0x90, 0x38, VISCA_TERMINATOR];
        assert_eq!(state.observe(&network_change), None);
        assert_eq!(state.camera_count(), 1);

        let seven_cameras = [0x88, 0x30, 0x08, VISCA_TERMINATOR];
        assert_eq!(state.observe(&seven_cameras), Some(7));
        assert_eq!(state.camera_count(), 7);
    }

    #[test]
    fn address_set_chunk_processing_bounds_noise_and_preserves_fragments() {
        let mut framer = ProtocolFramer::new_with_limits_and_mode(8, 64, 64, FramingMode::RawVisca);
        let mut state = AddressSetState::default();

        assert!(matches!(
            process_address_set_chunk(&mut framer, &mut state, &[0x88, 0x30]),
            Ok(None)
        ));
        assert!(matches!(
            process_address_set_chunk(&mut framer, &mut state, &[0x08, VISCA_TERMINATOR]),
            Ok(Some(7))
        ));
        assert_eq!(state.camera_count(), 7);

        // Repeated full chunks of noise force the framer's resynchronization
        // path but never let its retained input exceed the configured bound.
        let noise = [0x55u8; 64];
        for _ in 0..1024 {
            assert!(matches!(
                process_address_set_chunk(&mut framer, &mut state, &noise),
                Ok(None)
            ));
            assert!(framer.buffered_len() <= 64);
        }

        framer.clear();
        assert!(matches!(
            process_address_set_chunk(&mut framer, &mut state, &[0x90, 0x38]),
            Ok(None)
        ));
        assert!(matches!(
            process_address_set_chunk(&mut framer, &mut state, &[VISCA_TERMINATOR]),
            Ok(None)
        ));
        assert!(matches!(
            process_address_set_chunk(
                &mut framer,
                &mut state,
                &[0x88, 0x30, 0x04, VISCA_TERMINATOR]
            ),
            Ok(Some(3))
        ));
        assert_eq!(state.camera_count(), 3);
    }
}
