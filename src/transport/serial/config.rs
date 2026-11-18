//! Serial port configuration shared by the blocking and Tokio serial
//! transports.

/// Bus writes a serial transport performs while opening, before any session
/// traffic.
///
/// The default performs **no** protocol write: opening a serial transport
/// then only opens and configures the port. VISCA serial startup commands are
/// broadcasts that affect every camera on the daisy chain, so they are an
/// explicit choice of the bus owner:
///
/// - [`address_set`](Self::address_set) broadcasts Address Set
///   (`88 30 01 FF`) and waits for the reply that reports how many cameras
///   were addressed. The VISCA serial guidance is to send it after the chain
///   powers up or changes; a bus whose cameras already hold their addresses
///   does not need it.
/// - [`interface_clear`](Self::interface_clear) broadcasts I/F Clear
///   (`88 01 00 01 FF`) and waits a 100 ms settle delay. It clears every
///   camera's command buffers and cancels pending commands, so it is for
///   setup or recovery, normally after Address Set.
///
/// When both are selected Address Set is sent first. Opening waits for each
/// selected bus operation before ordinary session traffic begins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub struct SerialStartup {
    /// Broadcast Address Set and wait for its reply.
    pub(crate) address_set: bool,
    /// Broadcast I/F Clear and wait the settle delay.
    pub(crate) interface_clear: bool,
}

impl SerialStartup {
    /// Returns whether opening broadcasts Address Set.
    #[must_use]
    pub const fn address_set(&self) -> bool {
        self.address_set
    }

    /// Returns whether opening broadcasts I/F Clear.
    #[must_use]
    pub const fn interface_clear(&self) -> bool {
        self.interface_clear
    }

    /// Select or deselect Address Set.
    #[must_use]
    pub const fn with_address_set(mut self, enabled: bool) -> Self {
        self.address_set = enabled;
        self
    }

    /// Select or deselect I/F Clear.
    #[must_use]
    pub const fn with_interface_clear(mut self, enabled: bool) -> Self {
        self.interface_clear = enabled;
        self
    }
}

/// Serial port configuration for VISCA communication.
///
/// Startup writes are controlled by [`SerialStartup`]. Timeouts and frame
/// limits are specified separately with [`crate::transport::IoBounds`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub struct SerialConfig {
    /// Serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows).
    pub(crate) port: String,
    /// Baud rate (typically 9600 or 38400 for VISCA).
    pub(crate) baud_rate: u32,
    /// Bus writes performed while opening the port.
    pub(crate) startup: SerialStartup,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            startup: SerialStartup::default(),
        }
    }
}

impl SerialConfig {
    /// Returns the serial device path.
    #[must_use]
    pub fn port(&self) -> &str {
        &self.port
    }

    /// Returns the baud rate.
    #[must_use]
    pub const fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    /// Returns the selected startup bus writes.
    #[must_use]
    pub const fn startup(&self) -> SerialStartup {
        self.startup
    }

    /// Selects the serial device path.
    #[must_use]
    pub fn with_port(mut self, port: impl Into<String>) -> Self {
        self.port = port.into();
        self
    }

    /// Create a new serial configuration with default settings.
    pub fn new(port: impl Into<String>) -> Self {
        Self {
            port: port.into(),
            ..Default::default()
        }
    }

    /// Set the baud rate.
    #[must_use]
    pub const fn with_baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    /// Select the bus writes performed while opening.
    #[must_use]
    pub const fn with_startup(mut self, startup: SerialStartup) -> Self {
        self.startup = startup;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_select_9600_baud_and_write_nothing() {
        let config = SerialConfig::new("/dev/test");
        assert_eq!(config.port(), "/dev/test");
        assert_eq!(config.baud_rate(), 9600);
        assert!(!config.startup().address_set() && !config.startup().interface_clear());
    }

    #[test]
    fn configured_device_preserves_the_requested_startup() {
        let config = SerialConfig::new("COM1")
            .with_baud_rate(38_400)
            .with_startup(SerialStartup::default().with_interface_clear(true));
        assert_eq!(config.port(), "COM1");
        assert_eq!(config.baud_rate(), 38_400);
        assert!(config.startup().interface_clear());
    }
}
