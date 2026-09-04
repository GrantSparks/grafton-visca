//! Private time/deadline helpers and the canonical command timeout values.

use std::time::Duration;

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
use std::time::Instant;

use crate::{Error, Result};

/// Exact completion values for the five command timeout classes.
///
/// The defaults are the battle-tested 1.x values: Quick 5 seconds, Movement
/// 30 seconds, Preset 60 seconds, LongRunning 300 seconds, and Network 5
/// seconds. Inquiry deadlines are profile timing facts and are intentionally
/// not represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CommandTimeouts {
    quick_timeout: Duration,
    movement_timeout: Duration,
    preset_timeout: Duration,
    long_running_timeout: Duration,
    network_timeout: Duration,
}

impl CommandTimeouts {
    /// Creates exact category values.
    #[must_use]
    pub const fn new(
        quick: Duration,
        movement: Duration,
        preset: Duration,
        long_running: Duration,
        network: Duration,
    ) -> Self {
        Self {
            quick_timeout: quick,
            movement_timeout: movement,
            preset_timeout: preset,
            long_running_timeout: long_running,
            network_timeout: network,
        }
    }

    /// Returns the Quick command completion deadline.
    #[must_use]
    pub const fn quick_timeout(self) -> Duration {
        self.quick_timeout
    }

    /// Returns the Movement command completion deadline.
    #[must_use]
    pub const fn movement_timeout(self) -> Duration {
        self.movement_timeout
    }

    /// Returns the Preset command completion deadline.
    #[must_use]
    pub const fn preset_timeout(self) -> Duration {
        self.preset_timeout
    }

    /// Returns the LongRunning command completion deadline.
    #[must_use]
    pub const fn long_running_timeout(self) -> Duration {
        self.long_running_timeout
    }

    /// Returns the Network command completion deadline.
    #[must_use]
    pub const fn network_timeout(self) -> Duration {
        self.network_timeout
    }

    /// Validates that every category has a non-zero deadline.
    pub(crate) fn validate(self) -> Result<()> {
        if [
            self.quick_timeout,
            self.movement_timeout,
            self.preset_timeout,
            self.long_running_timeout,
            self.network_timeout,
        ]
        .into_iter()
        .any(|timeout| timeout.is_zero())
        {
            return Err(Error::InvalidRequest(
                "all command category timeouts must be non-zero".into(),
            ));
        }
        Ok(())
    }
}

impl Default for CommandTimeouts {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(5),
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(300),
            Duration::from_secs(5),
        )
    }
}

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Deadline {
    deadline: Instant,
}

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
impl Deadline {
    /// Creates a monotonic deadline from the current clock.
    ///
    /// An invalidly large duration is rejected instead of allowing the
    /// `Instant` addition to panic.
    pub(crate) fn from_timeout(timeout: Duration) -> Result<Self> {
        let deadline =
            Instant::now()
                .checked_add(timeout)
                .ok_or_else(|| Error::InvalidParameter {
                    parameter: "connect_timeout",
                    value: format!("{timeout:?}").into(),
                    reason: "timeout is too large for the monotonic clock".into(),
                })?;
        Ok(Self { deadline })
    }

    /// Returns the remaining duration at a sampled instant.
    pub(crate) fn remaining_at(&self, now: Instant) -> Duration {
        self.deadline.saturating_duration_since(now)
    }
}
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn command_defaults_match_the_legacy_oracle() {
        let defaults = CommandTimeouts::default();
        assert_eq!(defaults.quick_timeout(), Duration::from_secs(5));
        assert_eq!(defaults.movement_timeout(), Duration::from_secs(30));
        assert_eq!(defaults.preset_timeout(), Duration::from_secs(60));
        assert_eq!(defaults.long_running_timeout(), Duration::from_secs(300));
        assert_eq!(defaults.network_timeout(), Duration::from_secs(5));
    }

    #[test]
    fn command_validation_rejects_zero_values() {
        assert!(CommandTimeouts::new(
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .validate()
        .is_err());
        assert!(CommandTimeouts::new(
            Duration::from_secs(4),
            Duration::from_secs(30),
            Duration::from_secs(60),
            Duration::from_secs(300),
            Duration::from_secs(5),
        )
        .validate()
        .is_ok());
    }

    #[cfg(any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    ))]
    #[test]
    fn deadline_rejects_unrepresentable_timeout() {
        assert!(Deadline::from_timeout(Duration::MAX).is_err());
    }
}
