//! Private time/deadline helpers and the canonical command timeout values.

use std::time::Duration;
#[cfg(any(feature = "async", feature = "blocking", test))]
use std::time::Instant;

#[cfg(any(feature = "async", feature = "blocking", test))]
use crate::TimeoutClass;
use crate::{Error, Result};

/// Declares [`CommandCategory`] and its [`CommandCategory::ALL`] inventory
/// from one variant list, so a category added to the enum is always visited by
/// every walk over `ALL` (validation, override checks and table projection).
macro_rules! command_categories {
    ($($variant:ident),+ $(,)?) => {
        /// The five command completion categories: every [`TimeoutClass`]
        /// except [`TimeoutClass::Inquiry`], whose deadline is the profile's
        /// inquiry timing fact.
        ///
        /// Every per-category table ([`CommandTimeouts`], the operational
        /// overrides, and the validation that relates them) is reached through
        /// this one vocabulary, so a new category cannot be added without
        /// supplying each of its values.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum CommandCategory {
            $($variant,)+
        }

        impl CommandCategory {
            /// Every command category, in declaration order. Generated from
            /// the same list as the enum, so it cannot omit a category.
            pub(crate) const ALL: [Self; [$(stringify!($variant)),+].len()] =
                [$(Self::$variant),+];
        }
    };
}

command_categories!(Quick, Movement, Preset, LongRunning, Network);

impl CommandCategory {
    /// The command category a request's timeout class selects, or `None` for
    /// an inquiry.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) const fn of(class: TimeoutClass) -> Option<Self> {
        match class {
            TimeoutClass::Quick => Some(Self::Quick),
            TimeoutClass::Movement => Some(Self::Movement),
            TimeoutClass::Preset => Some(Self::Preset),
            TimeoutClass::LongRunning => Some(Self::LongRunning),
            TimeoutClass::Network => Some(Self::Network),
            TimeoutClass::Inquiry => None,
        }
    }
}

/// Exact completion values for the five command timeout classes.
///
/// [`Self::DEFAULT`] is Quick 5 seconds, Movement 30 seconds, Preset 60
/// seconds, LongRunning 300 seconds, and Network 5 seconds. Inquiry deadlines
/// are profile timing facts and are intentionally not represented here.
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
    /// The standard category deadlines: Quick 5 s, Movement 30 s, Preset
    /// 60 s, LongRunning 300 s, and Network 5 s.
    pub const DEFAULT: Self = Self::new(
        Duration::from_secs(5),
        Duration::from_secs(30),
        Duration::from_secs(60),
        Duration::from_secs(300),
        Duration::from_secs(5),
    );

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

    /// Returns one category's completion deadline.
    pub(crate) const fn get(self, category: CommandCategory) -> Duration {
        match category {
            CommandCategory::Quick => self.quick_timeout,
            CommandCategory::Movement => self.movement_timeout,
            CommandCategory::Preset => self.preset_timeout,
            CommandCategory::LongRunning => self.long_running_timeout,
            CommandCategory::Network => self.network_timeout,
        }
    }

    /// Validates that every category has a non-zero deadline.
    pub(crate) fn validate(self) -> Result<()> {
        if CommandCategory::ALL
            .into_iter()
            .any(|category| self.get(category).is_zero())
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
        Self::DEFAULT
    }
}

/// The instant `timeout` after `now`.
///
/// This is the single representability check for every transport timeout:
/// configuration validation and every runtime budget use it, so a timeout too
/// large for the monotonic clock is always the same
/// [`Error::InvalidParameter`] naming the configured timeout (for example
/// `"write_timeout"`), never a panic.
#[cfg(any(feature = "async", feature = "blocking", test))]
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
    fn category_accessor_reads_each_named_deadline() {
        let timeouts = CommandTimeouts::new(
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(4),
            Duration::from_secs(5),
        );
        assert_eq!(
            CommandCategory::ALL.map(|category| timeouts.get(category)),
            [
                timeouts.quick_timeout(),
                timeouts.movement_timeout(),
                timeouts.preset_timeout(),
                timeouts.long_running_timeout(),
                timeouts.network_timeout(),
            ]
        );
        assert_eq!(CommandCategory::of(TimeoutClass::Inquiry), None);
        for (class, category) in [
            (TimeoutClass::Quick, CommandCategory::Quick),
            (TimeoutClass::Movement, CommandCategory::Movement),
            (TimeoutClass::Preset, CommandCategory::Preset),
            (TimeoutClass::LongRunning, CommandCategory::LongRunning),
            (TimeoutClass::Network, CommandCategory::Network),
        ] {
            assert_eq!(CommandCategory::of(class), Some(category));
        }
        assert_eq!(CommandTimeouts::default(), CommandTimeouts::DEFAULT);
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
