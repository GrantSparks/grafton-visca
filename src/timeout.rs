//! Private time/deadline helpers and the canonical command timeout values.

use std::time::{Duration, Instant};

use crate::{Error, Result};

/// Exact completion values for the five command timeout classes.
///
/// The defaults are Quick 5 seconds, Movement 30 seconds, Preset 60 seconds,
/// LongRunning 300 seconds, and Network 5 seconds. Inquiry deadlines are
/// profile timing facts and are intentionally not represented here.
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

/// The instant `timeout` after `now`.
///
/// This is the single representability check for every transport timeout:
/// configuration validation and every runtime budget use it, so a timeout too
/// large for the monotonic clock is always the same
/// [`Error::InvalidParameter`] naming the configured timeout (for example
/// `"write_timeout"`), never a panic.
pub(crate) fn instant_after(
    now: Instant,
    timeout: Duration,
    parameter: &'static str,
) -> Result<Instant> {
    now.checked_add(timeout)
        .ok_or_else(|| Error::InvalidParameter {
            parameter,
            value: format!("{timeout:?}").into(),
            reason: "timeout is too large for the monotonic clock".into(),
        })
}

/// One fixed I/O budget expressed as a monotonic deadline.
///
/// Every transport operation that spans more than one syscall (a whole-frame
/// write, a datagram receive that skips empty packets, a multi-address
/// connect, a serial startup attempt) computes its remaining budget through
/// this type, so overflow and expiry are handled identically everywhere:
///
/// - a budget too large for the monotonic clock is rejected up front with
///   [`Error::InvalidParameter`] naming the configured timeout
///   ([`Deadline::after`]);
/// - a spent budget is reported with the caller's own expiry error
///   ([`Deadline::remaining_or`]), because only the caller knows whether it is
///   a connect, read, or write timeout.
///
/// The clock is always passed in, so executor-driven code can use its virtual
/// clock and stay deterministic under the test executor.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Deadline {
    deadline: Instant,
}

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
impl Deadline {
    /// Starts a `timeout` budget at `now`.
    ///
    /// `parameter` names the configured timeout (for example
    /// `"write_timeout"`) in the error returned when the deadline cannot be
    /// represented, instead of letting the `Instant` addition panic.
    pub(crate) fn after(now: Instant, timeout: Duration, parameter: &'static str) -> Result<Self> {
        Ok(Self {
            deadline: instant_after(now, timeout, parameter)?,
        })
    }

    /// Returns the remaining duration at a sampled instant.
    pub(crate) fn remaining_at(&self, now: Instant) -> Duration {
        self.deadline.saturating_duration_since(now)
    }

    /// Returns the unspent budget, or `on_expiry()` once it is spent.
    pub(crate) fn remaining_or(&self, now: Instant, on_expiry: fn() -> Error) -> Result<Duration> {
        let remaining = self.remaining_at(now);
        if remaining.is_zero() {
            Err(on_expiry())
        } else {
            Ok(remaining)
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn command_defaults_are_the_documented_category_deadlines() {
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
    fn deadline_rejects_unrepresentable_timeout_with_its_parameter_name() {
        assert!(matches!(
            Deadline::after(Instant::now(), Duration::MAX, "write_timeout"),
            Err(Error::InvalidParameter {
                parameter: "write_timeout",
                ..
            })
        ));
    }

    #[cfg(any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    ))]
    #[test]
    fn deadline_reports_the_callers_expiry_error_once_spent() {
        let now = Instant::now();
        let deadline =
            Deadline::after(now, Duration::from_millis(5), "read_timeout").expect("finite budget");
        assert_eq!(
            deadline
                .remaining_or(now, Error::io_timeout)
                .expect("unspent budget"),
            Duration::from_millis(5)
        );
        assert!(matches!(
            deadline.remaining_or(now + Duration::from_millis(5), Error::io_timeout),
            Err(Error::Timeout { .. })
        ));
    }
}
