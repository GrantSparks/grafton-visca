//! Timeout configuration for VISCA commands.
//!
//! This module provides configurable timeout support for different categories of VISCA commands,
//! allowing fine-tuned control over command execution timeouts based on the expected duration
//! of each operation type.

use std::time::Duration;

/// Categories of VISCA commands with different timeout requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    /// Quick commands like inquiry, power status (1-2 seconds).
    Quick,
    /// Movement commands like pan/tilt/zoom (5-10 seconds).
    Movement,
    /// Preset operations like recall/save (30-60 seconds).
    Preset,
    /// Long operations like preset discovery (2-5 minutes).
    LongRunning,
    /// Network commands like multicast/NDI settings (1-2 seconds).
    Network,
    /// Custom timeout for specific commands.
    Custom,
}

impl CommandCategory {
    /// Returns the default timeout for this category.
    #[must_use]
    pub const fn default_timeout(&self) -> Duration {
        match self {
            Self::Quick => Duration::from_secs(5), // Increased from 2s for network delays
            Self::Movement => Duration::from_secs(30), // Increased from 10s for full-range movements
            Self::Preset => Duration::from_secs(90),   // Increased from 60s for complex presets
            Self::LongRunning => Duration::from_secs(300), // Keep at 5 minutes for discovery
            Self::Network => Duration::from_secs(5),   // Increased from 2s for network operations
            Self::Custom => Duration::from_secs(60),   // Increased from 30s as general fallback
        }
    }
}

/// Configuration for command timeouts.
#[derive(Debug, Copy, Clone)]
pub struct TimeoutConfig {
    /// Timeout for ACK responses from the camera (typically 50-75ms)
    pub ack_timeout: Duration,
    /// Timeout for quick commands (inquiry, power status)
    pub quick_timeout: Duration,
    /// Timeout for movement commands (pan/tilt/zoom)
    pub movement_timeout: Duration,
    /// Timeout for preset operations
    pub preset_timeout: Duration,
    /// Timeout for long-running operations
    pub long_timeout: Duration,
    /// Timeout for network commands (multicast, NDI)
    pub network_timeout: Duration,
    /// Default timeout for uncategorized commands
    pub default_timeout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            ack_timeout: Duration::from_millis(75), // Quick ACK response timeout
            quick_timeout: Duration::from_secs(5),  // Increased for network delays
            movement_timeout: Duration::from_secs(30), // Increased for full-range movements
            preset_timeout: Duration::from_secs(90), // Increased for complex presets
            long_timeout: Duration::from_secs(300), // Keep at 5 minutes for discovery
            network_timeout: Duration::from_secs(5), // Increased for network operations
            default_timeout: Duration::from_secs(60), // Increased as general fallback
        }
    }
}

impl TimeoutConfig {
    /// Creates a new timeout configuration with all timeouts set to the same value.
    #[must_use]
    pub const fn uniform(timeout: Duration) -> Self {
        Self {
            ack_timeout: timeout,
            quick_timeout: timeout,
            movement_timeout: timeout,
            preset_timeout: timeout,
            long_timeout: timeout,
            network_timeout: timeout,
            default_timeout: timeout,
        }
    }

    /// Gets the timeout for a specific command category.
    #[must_use]
    pub const fn get_timeout(&self, category: CommandCategory) -> Duration {
        match category {
            CommandCategory::Quick => self.quick_timeout,
            CommandCategory::Movement => self.movement_timeout,
            CommandCategory::Preset => self.preset_timeout,
            CommandCategory::LongRunning => self.long_timeout,
            CommandCategory::Network => self.network_timeout,
            CommandCategory::Custom => self.default_timeout,
        }
    }

    /// Creates a builder for timeout configuration.
    #[must_use]
    pub fn builder() -> TimeoutConfigBuilder {
        TimeoutConfigBuilder::default()
    }
}

/// Builder for creating custom timeout configurations.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeoutConfigBuilder {
    config: TimeoutConfig,
}

impl TimeoutConfigBuilder {
    /// Sets the timeout for ACK responses.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn ack_timeout(mut self, timeout: Duration) -> Self {
        self.config.ack_timeout = timeout;
        self
    }

    /// Sets the timeout for quick commands.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn quick_timeout(mut self, timeout: Duration) -> Self {
        self.config.quick_timeout = timeout;
        self
    }

    /// Sets the timeout for movement commands.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn movement_timeout(mut self, timeout: Duration) -> Self {
        self.config.movement_timeout = timeout;
        self
    }

    /// Sets the timeout for preset operations.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn preset_timeout(mut self, timeout: Duration) -> Self {
        self.config.preset_timeout = timeout;
        self
    }

    /// Sets the timeout for long-running operations.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn long_timeout(mut self, timeout: Duration) -> Self {
        self.config.long_timeout = timeout;
        self
    }

    /// Sets the timeout for network commands.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn network_timeout(mut self, timeout: Duration) -> Self {
        self.config.network_timeout = timeout;
        self
    }

    /// Sets the default timeout for uncategorized commands.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn default_timeout(mut self, timeout: Duration) -> Self {
        self.config.default_timeout = timeout;
        self
    }

    /// Builds the timeout configuration.
    #[must_use]
    pub const fn build(self) -> TimeoutConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_category_defaults() {
        assert_eq!(
            CommandCategory::Quick.default_timeout(),
            Duration::from_secs(5)
        );
        assert_eq!(
            CommandCategory::Movement.default_timeout(),
            Duration::from_secs(30)
        );
        assert_eq!(
            CommandCategory::Preset.default_timeout(),
            Duration::from_secs(90)
        );
        assert_eq!(
            CommandCategory::LongRunning.default_timeout(),
            Duration::from_secs(300)
        );
        assert_eq!(
            CommandCategory::Network.default_timeout(),
            Duration::from_secs(5)
        );
        assert_eq!(
            CommandCategory::Custom.default_timeout(),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn test_timeout_config_default() {
        let config = TimeoutConfig::default();
        assert_eq!(config.ack_timeout, Duration::from_millis(75));
        assert_eq!(config.quick_timeout, Duration::from_secs(5));
        assert_eq!(config.movement_timeout, Duration::from_secs(30));
        assert_eq!(config.preset_timeout, Duration::from_secs(90));
        assert_eq!(config.long_timeout, Duration::from_secs(300));
        assert_eq!(config.network_timeout, Duration::from_secs(5));
        assert_eq!(config.default_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_timeout_config_uniform() {
        let timeout = Duration::from_secs(5);
        let config = TimeoutConfig::uniform(timeout);
        assert_eq!(config.ack_timeout, timeout);
        assert_eq!(config.quick_timeout, timeout);
        assert_eq!(config.movement_timeout, timeout);
        assert_eq!(config.preset_timeout, timeout);
        assert_eq!(config.long_timeout, timeout);
        assert_eq!(config.network_timeout, timeout);
        assert_eq!(config.default_timeout, timeout);
    }

    #[test]
    fn test_get_timeout() {
        let config = TimeoutConfig::default();
        assert_eq!(
            config.get_timeout(CommandCategory::Quick),
            Duration::from_secs(5)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Movement),
            Duration::from_secs(30)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Preset),
            Duration::from_secs(90)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::LongRunning),
            Duration::from_secs(300)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Network),
            Duration::from_secs(5)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Custom),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn test_timeout_config_builder() {
        let config = TimeoutConfig::builder()
            .quick_timeout(Duration::from_secs(1))
            .movement_timeout(Duration::from_secs(5))
            .preset_timeout(Duration::from_secs(30))
            .long_timeout(Duration::from_secs(120))
            .default_timeout(Duration::from_secs(15))
            .build();

        assert_eq!(config.quick_timeout, Duration::from_secs(1));
        assert_eq!(config.movement_timeout, Duration::from_secs(5));
        assert_eq!(config.preset_timeout, Duration::from_secs(30));
        assert_eq!(config.long_timeout, Duration::from_secs(120));
        assert_eq!(config.default_timeout, Duration::from_secs(15));
    }
}
