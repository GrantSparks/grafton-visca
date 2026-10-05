//! Serial port configuration shared by the blocking and Tokio serial
//! transports.

use std::time::Duration;

use crate::transport::{buffer::BufferConfig, builder::TransportConfig};

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
/// When both are selected Address Set is sent first. See
/// [`crate::transport::serial`] for the timing and failure policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Startup {
    /// Broadcast Address Set and wait for its reply.
    pub address_set: bool,
    /// Broadcast I/F Clear and wait the settle delay.
    pub interface_clear: bool,
}

impl Startup {
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
/// [`Config::transport_config`] reports them back as the transport's
/// configuration. The default [`Startup`] writes nothing to the bus.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Config {
    /// Serial port path (e.g., "/dev/ttyUSB0" on Unix, "COM1" on Windows).
    pub port: String,
    /// Baud rate (typically 9600 or 38400 for VISCA).
    pub baud_rate: u32,
    /// Bus writes performed while opening the port.
    pub startup: Startup,
    /// Read timeout for serial operations.
    pub read_timeout: Duration,
    /// Write timeout for serial operations.
    pub write_timeout: Duration,
    /// Frame and retention limits for received data.
    pub buffer_config: BufferConfig,
}

impl Default for Config {
    fn default() -> Self {
        let transport = TransportConfig::for_serial();
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 9600,
            startup: Startup::default(),
            read_timeout: transport.read_timeout,
            write_timeout: transport.write_timeout,
            buffer_config: transport.buffer_config,
        }
    }
}

impl Config {
    /// Create a new serial configuration with default settings.
    pub fn new(port: impl Into<String>) -> Self {
        Self {
            port: port.into(),
            ..Default::default()
        }
    }

    /// Set the baud rate.
    pub fn baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    /// Select the bus writes performed while opening.
    pub fn startup(mut self, startup: Startup) -> Self {
        self.startup = startup;
        self
    }

    /// Set the read timeout.
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    /// Set the write timeout.
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = timeout;
        self
    }

    /// Set the buffer configuration.
    pub fn buffer_config(mut self, config: BufferConfig) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::AddressingMode;

    /// #828: the serial defaults used to request I/F Clear on every open.
    #[test]
    fn defaults_come_from_the_serial_transport_defaults_and_write_nothing() {
        let config = Config::new("/dev/test");
        assert_eq!(config.startup, Startup::default());
        assert!(!config.startup.address_set && !config.startup.interface_clear);
        assert_eq!(config.transport_config(), TransportConfig::for_serial());
    }

    #[test]
    fn reported_transport_config_is_serial_shaped() {
        let config = Config::new("/dev/test")
            .read_timeout(Duration::from_millis(37))
            .write_timeout(Duration::from_millis(41));
        let reported = config.transport_config();
        assert_eq!(reported.addressing, AddressingMode::Serial);
        assert_eq!(reported.tcp_nodelay, None);
        assert_eq!(reported.tcp_keepalive, None);
        assert_eq!(reported.ttl, None);
        assert_eq!(reported.read_timeout, Duration::from_millis(37));
        assert_eq!(reported.write_timeout, Duration::from_millis(41));
    }
}
