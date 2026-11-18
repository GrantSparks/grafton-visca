//! Serial port configuration shared by the blocking and Tokio serial
//! transports.

use std::time::Duration;

use crate::transport::{
    buffer::BufferConfig,
    builder::{TransportConfig, ValidatedTransportConfig},
};

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
/// Timeouts and buffer limits default to [`TransportConfig::for_serial`], the
/// single source of serial transport defaults, and
/// [`SerialConfig::transport_config`] reports them back as the transport's
/// configuration. The default [`SerialStartup`] writes nothing to the bus.
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
    /// Read timeout for serial operations.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    pub(crate) read_timeout: Duration,
    /// Write timeout for serial operations.
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    pub(crate) write_timeout: Duration,
    /// Frame and retention limits for received data.
    pub(crate) buffer_config: BufferConfig,
}

impl Default for SerialConfig {
    fn default() -> Self {
        let transport = TransportConfig::for_serial();
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            startup: SerialStartup::default(),
            read_timeout: transport.read_timeout(),
            write_timeout: transport.write_timeout(),
            buffer_config: transport.buffer_config(),
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

    /// Returns the receive timeout.
    #[must_use]
    pub const fn read_timeout(&self) -> Duration {
        self.read_timeout
    }

    /// Returns the send timeout.
    #[must_use]
    pub const fn write_timeout(&self) -> Duration {
        self.write_timeout
    }

    /// Returns the receive frame and retention limits.
    #[must_use]
    pub const fn buffer_config(&self) -> BufferConfig {
        self.buffer_config
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

    /// Set the read timeout.
    #[must_use]
    pub const fn with_read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Set the write timeout.
    #[must_use]
    pub const fn with_write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = timeout;
        self
    }

    /// Set the buffer configuration.
    #[must_use]
    pub const fn with_buffer_config(mut self, config: BufferConfig) -> Self {
        self.buffer_config = config;
        self
    }

    /// The transport configuration a serial transport opened from this
    /// configuration reports: [`TransportConfig::for_serial`] with this
    /// configuration's timeouts and buffer limits. It never carries IP
    /// addressing or TCP socket options.
    #[must_use]
    pub fn transport_config(&self) -> TransportConfig {
        let mut config = TransportConfig::for_serial();
        config.read_timeout = self.read_timeout;
        config.write_timeout = self.write_timeout;
        config.buffer_config = self.buffer_config;
        config
    }
}

/// Serial device settings and their transport limits, checked before startup I/O.
#[derive(Debug, Clone)]
pub struct ValidatedSerialConfig {
    config: SerialConfig,
    transport_config: ValidatedTransportConfig,
}

impl ValidatedSerialConfig {
    /// Returns the device settings.
    #[must_use]
    pub const fn as_config(&self) -> &SerialConfig {
        &self.config
    }

    /// Returns the validated bounds for a transport opened on this device.
    #[must_use]
    pub const fn transport_config(&self) -> &ValidatedTransportConfig {
        &self.transport_config
    }

    #[cfg(any(
        feature = "transport-serial",
        feature = "transport-serial-tokio",
        all(test, any(feature = "async", feature = "blocking"))
    ))]
    pub(crate) fn into_parts(self) -> (SerialConfig, ValidatedTransportConfig) {
        (self.config, self.transport_config)
    }
}

impl SerialConfig {
    /// Check the transport limits before opening a serial device.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidRequest`] if a timeout or buffer limit is invalid.
    pub fn validated(self) -> crate::Result<ValidatedSerialConfig> {
        Ok(ValidatedSerialConfig {
            transport_config: self.transport_config().validated()?,
            config: self,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #828: the serial defaults used to request I/F Clear on every open.
    #[test]
    fn defaults_come_from_the_serial_transport_defaults_and_write_nothing() {
        let config = SerialConfig::new("/dev/test");
        assert_eq!(config.startup(), SerialStartup::default());
        assert!(!config.startup().address_set() && !config.startup().interface_clear());
        assert_eq!(config.transport_config(), TransportConfig::for_serial());
    }

    #[test]
    fn reported_transport_config_is_serial_shaped() {
        let config = SerialConfig::new("/dev/test")
            .with_read_timeout(Duration::from_millis(37))
            .with_write_timeout(Duration::from_millis(41));
        let reported = config.transport_config();

        assert_eq!(reported.tcp_nodelay(), None);
        assert_eq!(reported.tcp_keepalive(), None);
        assert_eq!(reported.ttl(), None);
        assert_eq!(reported.read_timeout(), Duration::from_millis(37));
        assert_eq!(reported.write_timeout(), Duration::from_millis(41));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_preserves_opt_in_bus_writes_and_rejects_unknown_options(
    ) -> Result<(), serde_json::Error> {
        let config = SerialConfig::new("/dev/test")
            .with_baud_rate(38_400)
            .with_startup(SerialStartup::default().with_address_set(true))
            .with_read_timeout(Duration::from_millis(37));
        let value = serde_json::to_value(&config)?;
        let restored: SerialConfig = serde_json::from_value(value.clone())?;
        assert_eq!(restored, config);
        assert!(restored.startup().address_set());
        assert!(!restored.startup().interface_clear());
        for nested in [false, true] {
            let mut unknown = value.clone();
            let object = if nested {
                &mut unknown["startup"]
            } else {
                &mut unknown
            };
            object["misspelled_option"] = serde_json::json!(true);
            assert!(serde_json::from_value::<SerialConfig>(unknown).is_err());
        }
        Ok(())
    }
}
