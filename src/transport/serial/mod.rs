//! Serial transport module for unified configuration and transport implementations.

mod config;
pub(crate) mod handshake;

pub use config::Config;

/// The shortest timeout that is safe to pass to a serial-device backend.
///
/// Windows interprets a zero-millisecond serial timeout as *no timeout*.
/// Derived owner budgets can legitimately be shorter than a millisecond, so
/// clamp only at this device boundary; the owner still checks its precise
/// deadline after every I/O operation.
pub(crate) const MIN_DEVICE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1);

/// Round a serial-device timeout up to the backend's smallest safe value.
pub(crate) fn device_timeout(timeout: std::time::Duration) -> std::time::Duration {
    timeout.max(MIN_DEVICE_TIMEOUT)
}

/// Write a complete frame before one absolute deadline.
///
/// Both regular blocking commands and startup handshakes use this loop.  A
/// partial write retains the same whole-frame deadline, transient interrupts
/// retry, and serial poll expiry is normalized to the public timeout error.
#[cfg(all(feature = "blocking", feature = "transport-serial"))]
pub(crate) fn write_bounded(
    port: &mut dyn serialport::SerialPort,
    bytes: &[u8],
    deadline: std::time::Instant,
    configured_write_timeout: std::time::Duration,
) -> crate::Result<()> {
    use std::io::ErrorKind;

    let mut written = 0;

    while written < bytes.len() {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(crate::Error::Timeout);
        }

        port.set_timeout(device_timeout(configured_write_timeout.min(remaining)))
            .map_err(|error| {
                crate::Error::TransportError(
                    format!("Failed to set serial write timeout: {error}").into(),
                )
            })?;

        match port.write(&bytes[written..]) {
            Ok(0) => {
                return Err(crate::Error::TransportError(
                    "Serial write made no progress".into(),
                ));
            }
            Ok(count) if count <= bytes.len() - written => written += count,
            Ok(count) => {
                return Err(crate::Error::TransportError(
                    format!(
                        "Serial write reported {count} bytes for a {}-byte buffer",
                        bytes.len() - written
                    )
                    .into(),
                ));
            }
            Err(error) if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                return Err(crate::Error::Timeout);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(crate::Error::TransportError(
                    format!("Serial write error: {error}").into(),
                ));
            }
        }
    }

    Ok(())
}

/// One optional operation performed while bringing up a serial VISCA bus.
///
/// Address assignment must precede I/F Clear when both are requested: the
/// clear resets the command interface after the bus has been addressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupOperation {
    AddressSet,
    InterfaceClear,
}

/// Return the serial startup operations in their protocol-required order.
///
/// Keeping this small plan shared makes blocking and Tokio connection paths
/// execute the same sequence while preserving the single-operation cases.
pub(crate) fn startup_plan(config: &Config) -> [Option<StartupOperation>; 2] {
    [
        config
            .address_set_on_connect
            .then_some(StartupOperation::AddressSet),
        config
            .if_clear_on_connect
            .then_some(StartupOperation::InterfaceClear),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_plan_preserves_requested_operations_in_protocol_order() {
        let neither = Config::default()
            .address_set_on_connect(false)
            .if_clear_on_connect(false);
        assert_eq!(startup_plan(&neither), [None, None]);

        let address_only = neither.clone().address_set_on_connect(true);
        assert_eq!(
            startup_plan(&address_only),
            [Some(StartupOperation::AddressSet), None]
        );

        let clear_only = neither.clone().if_clear_on_connect(true);
        assert_eq!(
            startup_plan(&clear_only),
            [None, Some(StartupOperation::InterfaceClear)]
        );

        let both = address_only.if_clear_on_connect(true);
        assert_eq!(
            startup_plan(&both),
            [
                Some(StartupOperation::AddressSet),
                Some(StartupOperation::InterfaceClear),
            ]
        );
    }
}
