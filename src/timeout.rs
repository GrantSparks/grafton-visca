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
    /// Custom timeout for specific commands.
    Custom,
}

impl CommandCategory {
    /// Returns the default timeout for this category.
    #[must_use]
    pub const fn default_timeout(&self) -> Duration {
        match self {
            Self::Quick => Duration::from_secs(2),
            Self::Movement => Duration::from_secs(10),
            Self::Preset => Duration::from_secs(60),
            Self::LongRunning => Duration::from_secs(300),
            Self::Custom => Duration::from_secs(30),
        }
    }
}

/// Configuration for command timeouts.
#[derive(Debug, Copy, Clone)]
pub struct TimeoutConfig {
    /// Timeout for quick commands (inquiry, power status)
    pub quick_timeout: Duration,
    /// Timeout for movement commands (pan/tilt/zoom)
    pub movement_timeout: Duration,
    /// Timeout for preset operations
    pub preset_timeout: Duration,
    /// Timeout for long-running operations
    pub long_timeout: Duration,
    /// Default timeout for uncategorized commands
    pub default_timeout: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            quick_timeout: Duration::from_secs(2),
            movement_timeout: Duration::from_secs(10),
            preset_timeout: Duration::from_secs(60),
            long_timeout: Duration::from_secs(300),
            default_timeout: Duration::from_secs(30),
        }
    }
}

impl TimeoutConfig {
    /// Creates a new timeout configuration with all timeouts set to the same value.
    #[must_use]
    pub const fn uniform(timeout: Duration) -> Self {
        Self {
            quick_timeout: timeout,
            movement_timeout: timeout,
            preset_timeout: timeout,
            long_timeout: timeout,
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
    /// Sets the timeout for quick commands.
    #[allow(clippy::missing_const_for_fn)] // Builder methods mutate self
    #[must_use]
    pub fn quick_timeout(mut self, timeout: Duration) -> Self {
        self.config.quick_timeout = timeout;
        self
    }

    /// Sets the timeout for movement commands.
    #[allow(clippy::missing_const_for_fn)] // Builder methods mutate self
    #[must_use]
    pub fn movement_timeout(mut self, timeout: Duration) -> Self {
        self.config.movement_timeout = timeout;
        self
    }

    /// Sets the timeout for preset operations.
    #[allow(clippy::missing_const_for_fn)] // Builder methods mutate self
    #[must_use]
    pub fn preset_timeout(mut self, timeout: Duration) -> Self {
        self.config.preset_timeout = timeout;
        self
    }

    /// Sets the timeout for long-running operations.
    #[allow(clippy::missing_const_for_fn)] // Builder methods mutate self
    #[must_use]
    pub fn long_timeout(mut self, timeout: Duration) -> Self {
        self.config.long_timeout = timeout;
        self
    }

    /// Sets the default timeout for uncategorized commands.
    #[allow(clippy::missing_const_for_fn)] // Builder methods mutate self
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_command_category_defaults() {
        assert_eq!(
            CommandCategory::Quick.default_timeout(),
            Duration::from_secs(2)
        );
        assert_eq!(
            CommandCategory::Movement.default_timeout(),
            Duration::from_secs(10)
        );
        assert_eq!(
            CommandCategory::Preset.default_timeout(),
            Duration::from_secs(60)
        );
        assert_eq!(
            CommandCategory::LongRunning.default_timeout(),
            Duration::from_secs(300)
        );
        assert_eq!(
            CommandCategory::Custom.default_timeout(),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn test_timeout_config_default() {
        let config = TimeoutConfig::default();
        assert_eq!(config.quick_timeout, Duration::from_secs(2));
        assert_eq!(config.movement_timeout, Duration::from_secs(10));
        assert_eq!(config.preset_timeout, Duration::from_secs(60));
        assert_eq!(config.long_timeout, Duration::from_secs(300));
        assert_eq!(config.default_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_timeout_config_uniform() {
        let timeout = Duration::from_secs(5);
        let config = TimeoutConfig::uniform(timeout);
        assert_eq!(config.quick_timeout, timeout);
        assert_eq!(config.movement_timeout, timeout);
        assert_eq!(config.preset_timeout, timeout);
        assert_eq!(config.long_timeout, timeout);
        assert_eq!(config.default_timeout, timeout);
    }

    #[test]
    fn test_get_timeout() {
        let config = TimeoutConfig::default();
        assert_eq!(
            config.get_timeout(CommandCategory::Quick),
            Duration::from_secs(2)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Movement),
            Duration::from_secs(10)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Preset),
            Duration::from_secs(60)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::LongRunning),
            Duration::from_secs(300)
        );
        assert_eq!(
            config.get_timeout(CommandCategory::Custom),
            Duration::from_secs(30)
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
