//! Private time/deadline helpers and the canonical command timeout values.

use std::time::Duration;
#[cfg(any(feature = "async", feature = "blocking", test))]
use std::time::Instant;

use crate::TimeoutClass;
use crate::{Error, Result};

/// Bounds an operation with a timer on any executor. A completed operation
/// always wins a tie: the operation is polled before the timer on every poll.
#[cfg(feature = "async")]
pub(crate) async fn within<T>(
    operation: impl std::future::Future<Output = T>,
    timer: impl std::future::Future<Output = ()>,
) -> Option<T> {
    futures_lite::future::or(async { Some(operation.await) }, async {
        timer.await;
        None
    })
    .await
}

/// Validates an interval against an already sampled owner clock. Zero remains
/// valid for pacing and immediate retry delays; positive timeouts use the
/// engine's separate nonzero proof.
#[cfg(any(feature = "async", feature = "blocking", test))]
pub(crate) fn validate_duration_at(at: Instant, duration: Duration) -> Result<()> {
    if at.checked_add(duration).is_none() {
        return Err(Error::InvalidRequest {
            reason: "engine deadline interval is not representable by the owner clock".into(),
        });
    }
    Ok(())
}

/// Adds an interval without inverting overflow into an immediate deadline.
/// The common path is exact. On overflow, retain the latest representable
/// instant; the search reads no clock and cannot wrap duration arithmetic.
#[cfg(any(feature = "async", feature = "blocking", test))]
pub(crate) fn saturating_deadline(at: Instant, duration: Duration) -> Instant {
    if let Some(deadline) = at.checked_add(duration) {
        return deadline;
    }
    const NANOS_PER_SECOND: u128 = 1_000_000_000;
    let mut lower = 0;
    let mut upper = duration.as_nanos();
    let mut latest = at;
    while upper - lower > 1 {
        let middle = lower + (upper - lower) / 2;
        // middle never exceeds a Duration's own total nanoseconds, so the
        // quotient fits u64 and the remainder is strictly below one billion.
        let candidate = Duration::new(
            (middle / NANOS_PER_SECOND) as u64,
            (middle % NANOS_PER_SECOND) as u32,
        );
        if let Some(deadline) = at.checked_add(candidate) {
            lower = middle;
            latest = deadline;
        } else {
            upper = middle;
        }
    }
    latest
}

/// A strictly positive interval. Absolute representability is checked against
/// the owner's sampled clock when a policy or request crosses the engine boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NonZeroDuration(Duration);

impl NonZeroDuration {
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn new(duration: Duration) -> Result<Self> {
        if duration.is_zero() {
            return Err(Error::InvalidRequest {
                reason: "timeout durations must be non-zero".into(),
            });
        }
        Ok(Self(duration))
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) const HALF_SECOND: Self = Self(Duration::from_millis(500));

    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) const fn from_secs<const SECONDS: u64>() -> Self {
        // This constructor is used only by fixed, positive engine defaults.
        const {
            assert!(SECONDS > 0);
        }
        Self(Duration::from_secs(SECONDS))
    }

    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) const fn from_millis(milliseconds: std::num::NonZeroU64) -> Self {
        Self(Duration::from_millis(milliseconds.get()))
    }

    pub(crate) const fn get(self) -> Duration {
        self.0
    }
}

/// One profile's checked operational deadlines and pacing. Public persisted
/// values remain `Duration`; only this internal lowering carries the proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedTiming {
    pub(crate) ack: NonZeroDuration,
    pub(crate) inquiry: NonZeroDuration,
    pub(crate) cancellation: NonZeroDuration,
    pub(crate) ambiguity: NonZeroDuration,
    command: [NonZeroDuration; CommandTimeoutClass::ALL.len()],
    // Inquiries have always retained the untuned Quick value in their unused
    // completion field. Keeping that separate preserves the inert context.
    inquiry_completion: NonZeroDuration,
    pub(crate) command_spacing: Duration,
    pub(crate) inquiry_spacing: Duration,
    settlement: Option<NonZeroDuration>,
    busy: Duration,
    tuning: crate::config::OperationalTuning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedRetryBounds {
    pub(crate) initial_backoff: Duration,
    pub(crate) maximum_backoff: Duration,
    pub(crate) total_budget: NonZeroDuration,
}

impl ResolvedTiming {
    /// Establish the override proof once, during tuning validation. The
    /// immutable profile already proves its own deadlines are positive.
    pub(crate) fn validate(
        tuning: crate::config::OperationalTuning,
        timing: &crate::profiles::ProfileTiming,
    ) -> Result<Self> {
        if [
            tuning.ack_timeout(),
            tuning.inquiry_timeout(),
            tuning.settlement_timeout(),
        ]
        .into_iter()
        .chain(CommandTimeoutClass::ALL.map(|category| tuning.command_override(category)))
        .flatten()
        .any(|duration| duration.is_zero())
        {
            return Err(Error::InvalidRequest {
                reason: "operational timeout overrides must be non-zero".into(),
            });
        }
        Ok(Self::resolve(tuning, timing))
    }

    /// Reuse the completed non-zero proof instead of testing the same
    /// deadlines on every prepared request.
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn from_validated(
        tuning: crate::session_config::ValidatedTuning,
        timing: &crate::profiles::ProfileTiming,
    ) -> Self {
        Self::resolve(tuning.get(), timing)
    }

    /// The only override-or-profile resolution. This private constructor is
    /// reached after `validate` or through the completed `ValidatedTuning`
    /// proof, so each wrapped interval is positive without another check.
    fn resolve(
        tuning: crate::config::OperationalTuning,
        timing: &crate::profiles::ProfileTiming,
    ) -> Self {
        Self {
            ack: NonZeroDuration(tuning.ack_timeout().unwrap_or(timing.ack_timeout())),
            inquiry: NonZeroDuration(tuning.inquiry_timeout().unwrap_or(timing.inquiry_timeout())),
            cancellation: NonZeroDuration(timing.cancellation_timeout()),
            ambiguity: NonZeroDuration(timing.ambiguity_timeout()),
            command: CommandTimeoutClass::ALL.map(|category| {
                NonZeroDuration(
                    tuning
                        .command_override(category)
                        .unwrap_or(timing.command_timeouts().get(category)),
                )
            }),
            inquiry_completion: NonZeroDuration(timing.command_timeouts().quick_timeout()),
            command_spacing: tuning.command_spacing().unwrap_or(timing.command_spacing()),
            inquiry_spacing: tuning.inquiry_spacing().unwrap_or(timing.inquiry_spacing()),
            settlement: tuning.settlement_timeout().map(NonZeroDuration),
            busy: timing.busy_timeout(),
            tuning,
        }
    }

    pub(crate) fn command(self, category: CommandTimeoutClass) -> NonZeroDuration {
        self.command[category as usize]
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn completion(self, class: TimeoutClass) -> NonZeroDuration {
        CommandTimeoutClass::of(class)
            .map_or(self.inquiry_completion, |category| self.command(category))
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn settlement_budget(self, class: TimeoutClass) -> Duration {
        self.settlement
            .unwrap_or_else(|| self.completion(class))
            .get()
    }

    /// A command's conservative ACK-bearing budget, also the validation floor
    /// for a targeted operation's explicit settlement budget.
    pub(crate) fn retry_budget(self, category: CommandTimeoutClass) -> NonZeroDuration {
        self.retry_bounds(Some(category), true).total_budget
    }

    /// The single request-budget formula. Inquiries and completion-only
    /// commands have no ACK window; ACK-bearing commands must fit both windows.
    pub(crate) fn retry_bounds(
        self,
        category: Option<CommandTimeoutClass>,
        ack_bearing: bool,
    ) -> ResolvedRetryBounds {
        let deadline = category
            .map_or(self.inquiry, |category| self.command(category))
            .get();
        let attempt = if ack_bearing {
            self.ack.get().saturating_add(deadline)
        } else {
            deadline
        };
        let bounds = self
            .tuning
            .lengthen_retry_bounds(crate::profiles::RetryBounds::for_request(
                self.busy, deadline, attempt,
            ));
        ResolvedRetryBounds {
            initial_backoff: bounds.initial_backoff,
            maximum_backoff: bounds.maximum_backoff,
            // RetryBounds has a positive ten-second floor, and overrides only
            // lengthen it. This proof does not require another runtime check.
            total_budget: NonZeroDuration(bounds.total_budget),
        }
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn maximum_command_sockets(
        self,
        baseline: crate::CommandSocketCount,
    ) -> crate::CommandSocketCount {
        self.tuning.maximum_command_sockets().unwrap_or(baseline)
    }
}

/// Declares [`crate::profiles::CommandTimeoutClass`] and its [`crate::profiles::CommandTimeoutClass::ALL`] inventory
/// from one variant list, so a category added to the enum is always visited by
/// every walk over `ALL` (validation, override checks and table projection).
macro_rules! command_timeout_classes {
    ($(#[doc = $doc:literal] $variant:ident),+ $(,)?) => {
        /// The five command completion categories: every [`crate::request::TimeoutClass`]
        /// except [`crate::request::TimeoutClass::Inquiry`], whose deadline is the profile's
        /// inquiry timing fact.
        ///
        /// Every per-category table ([`crate::profiles::CommandTimeouts`], the operational
        /// overrides, and the validation that relates them) is reached through
        /// this one vocabulary, so a new category cannot be added without
        /// supplying each of its values.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        pub enum CommandTimeoutClass {
            $(#[doc = $doc] $variant,)+
        }

        impl CommandTimeoutClass {
            /// Every command category, in declaration order. Generated from
            /// the same list as the enum, so it cannot omit a category.
            pub(crate) const ALL: [Self; [$(stringify!($variant)),+].len()] =
                [$(Self::$variant),+];
        }
    };
}

command_timeout_classes!(
    /// Fast configuration and ordinary control commands.
    Quick,
    /// Physical movement commands.
    Movement,
    /// Preset recall commands.
    Preset,
    /// Operations requiring an extended completion deadline.
    LongRunning,
    /// Network configuration commands.
    Network,
);

impl CommandTimeoutClass {
    /// Returns the request timeout class for this command category.
    #[must_use]
    pub const fn timeout_class(self) -> TimeoutClass {
        match self {
            Self::Quick => TimeoutClass::Quick,
            Self::Movement => TimeoutClass::Movement,
            Self::Preset => TimeoutClass::Preset,
            Self::LongRunning => TimeoutClass::LongRunning,
            Self::Network => TimeoutClass::Network,
        }
    }

    /// The command category a request's timeout class selects, or `None` for
    /// an inquiry.
    #[cfg(any(feature = "blocking", feature = "async", test))]
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

impl From<CommandTimeoutClass> for TimeoutClass {
    fn from(category: CommandTimeoutClass) -> Self {
        category.timeout_class()
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
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct CommandTimeouts {
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    quick_timeout: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    movement_timeout: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    preset_timeout: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    long_running_timeout: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number; nanos: number }"))]
    network_timeout: Duration,
}

impl CommandTimeouts {
    /// The standard category deadlines: Quick 5 s, Movement 30 s, Preset
    /// 60 s, LongRunning 300 s, and Network 5 s.
    pub const DEFAULT: Self = Self {
        quick_timeout: Duration::from_secs(5),
        movement_timeout: Duration::from_secs(30),
        preset_timeout: Duration::from_secs(60),
        long_running_timeout: Duration::from_secs(300),
        network_timeout: Duration::from_secs(5),
    };

    /// Selects the quick command completion deadline.
    /// Configuration admission requires every command deadline to be nonzero.
    #[must_use]
    pub const fn with_quick_timeout(mut self, timeout: Duration) -> Self {
        self.quick_timeout = timeout;
        self
    }

    /// Selects the movement command completion deadline.
    /// Configuration admission requires every command deadline to be nonzero.
    #[must_use]
    pub const fn with_movement_timeout(mut self, timeout: Duration) -> Self {
        self.movement_timeout = timeout;
        self
    }

    /// Selects the preset command completion deadline.
    /// Configuration admission requires every command deadline to be nonzero.
    #[must_use]
    pub const fn with_preset_timeout(mut self, timeout: Duration) -> Self {
        self.preset_timeout = timeout;
        self
    }

    /// Selects the long running command completion deadline.
    /// Configuration admission requires every command deadline to be nonzero.
    #[must_use]
    pub const fn with_long_running_timeout(mut self, timeout: Duration) -> Self {
        self.long_running_timeout = timeout;
        self
    }

    /// Selects the network command completion deadline.
    /// Configuration admission requires every command deadline to be nonzero.
    #[must_use]
    pub const fn with_network_timeout(mut self, timeout: Duration) -> Self {
        self.network_timeout = timeout;
        self
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
    pub(crate) const fn get(self, category: CommandTimeoutClass) -> Duration {
        match category {
            CommandTimeoutClass::Quick => self.quick_timeout,
            CommandTimeoutClass::Movement => self.movement_timeout,
            CommandTimeoutClass::Preset => self.preset_timeout,
            CommandTimeoutClass::LongRunning => self.long_running_timeout,
            CommandTimeoutClass::Network => self.network_timeout,
        }
    }

    /// Validates that every category has a non-zero deadline.
    pub(crate) fn validate(self) -> Result<()> {
        if CommandTimeoutClass::ALL
            .into_iter()
            .any(|category| self.get(category).is_zero())
        {
            return Err(Error::InvalidRequest {
                reason: "all command category timeouts must be non-zero".into(),
            });
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
    fn overflowing_deadlines_saturate_at_the_latest_representable_instant() {
        let at = Instant::now();
        for duration in [
            Duration::ZERO,
            Duration::from_nanos(1),
            Duration::from_secs(2 * 60 * 60),
        ] {
            assert_eq!(
                Some(saturating_deadline(at, duration)),
                at.checked_add(duration)
            );
        }
        let latest = saturating_deadline(at, Duration::MAX);
        assert!(latest > at);
        assert!(latest.checked_add(Duration::from_nanos(1)).is_none());
        assert_eq!(saturating_deadline(latest, Duration::from_secs(1)), latest);
        assert!(validate_duration_at(at, Duration::MAX).is_err());
        assert!(validate_duration_at(at, Duration::ZERO).is_ok());
    }

    #[cfg(feature = "async")]
    #[test]
    fn operation_wins_an_already_elapsed_timer() {
        use std::{future::ready, task::Poll};

        let timer_polls = std::cell::Cell::new(0);
        pollster::block_on(async {
            for _ in 0..10_000 {
                let timer = std::future::poll_fn(|_| {
                    timer_polls.set(timer_polls.get() + 1);
                    Poll::Ready(())
                });
                assert_eq!(within(ready(42), timer).await, Some(42));
            }
            assert_eq!(within(std::future::pending::<()>(), ready(())).await, None);
        });
        assert_eq!(
            timer_polls.get(),
            0,
            "a ready operation needs no timer poll"
        );
    }

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
        let timeouts = CommandTimeouts::DEFAULT
            .with_quick_timeout(Duration::from_secs(1))
            .with_movement_timeout(Duration::from_secs(2))
            .with_preset_timeout(Duration::from_secs(3))
            .with_long_running_timeout(Duration::from_secs(4))
            .with_network_timeout(Duration::from_secs(5));
        assert_eq!(
            CommandTimeoutClass::ALL.map(|category| timeouts.get(category)),
            [
                timeouts.quick_timeout(),
                timeouts.movement_timeout(),
                timeouts.preset_timeout(),
                timeouts.long_running_timeout(),
                timeouts.network_timeout(),
            ]
        );
        assert_eq!(CommandTimeoutClass::of(TimeoutClass::Inquiry), None);
        for (class, category) in [
            (TimeoutClass::Quick, CommandTimeoutClass::Quick),
            (TimeoutClass::Movement, CommandTimeoutClass::Movement),
            (TimeoutClass::Preset, CommandTimeoutClass::Preset),
            (TimeoutClass::LongRunning, CommandTimeoutClass::LongRunning),
            (TimeoutClass::Network, CommandTimeoutClass::Network),
        ] {
            assert_eq!(CommandTimeoutClass::of(class), Some(category));
        }
        assert_eq!(CommandTimeouts::default(), CommandTimeouts::DEFAULT);
    }

    #[test]
    fn command_validation_rejects_zero_values() {
        for invalid in [
            CommandTimeouts::DEFAULT.with_quick_timeout(Duration::ZERO),
            CommandTimeouts::DEFAULT.with_movement_timeout(Duration::ZERO),
            CommandTimeouts::DEFAULT.with_preset_timeout(Duration::ZERO),
            CommandTimeouts::DEFAULT.with_long_running_timeout(Duration::ZERO),
            CommandTimeouts::DEFAULT.with_network_timeout(Duration::ZERO),
        ] {
            assert!(invalid.validate().is_err());
        }
        assert!(CommandTimeouts::DEFAULT
            .with_quick_timeout(Duration::from_secs(4))
            .validate()
            .is_ok());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_pins_timeout_class_names_and_rejects_unknown_settings() -> Result<(), serde_json::Error>
    {
        for (class, tag) in CommandTimeoutClass::ALL.into_iter().zip([
            "quick",
            "movement",
            "preset",
            "long_running",
            "network",
        ]) {
            assert_eq!(serde_json::to_value(class)?, tag);
            assert_eq!(
                serde_json::from_value::<CommandTimeoutClass>(serde_json::json!(tag))?,
                class
            );
        }
        let config = CommandTimeouts::DEFAULT
            .with_quick_timeout(Duration::from_millis(37))
            .with_network_timeout(Duration::from_secs(9));
        let value = serde_json::to_value(config)?;
        assert_eq!(
            value["quick_timeout"],
            serde_json::json!({"secs": 0, "nanos": 37_000_000})
        );
        assert_eq!(
            serde_json::from_value::<CommandTimeouts>(value.clone())?,
            config
        );
        let mut unknown = value;
        unknown["quick"] = serde_json::json!({"secs": 1, "nanos": 0});
        assert!(serde_json::from_value::<CommandTimeouts>(unknown).is_err());
        Ok(())
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn timeout_bindings_keep_duration_objects_and_snake_case_classes() {
        let config_schema = schemars::schema_for!(CommandTimeouts);
        assert_eq!(
            config_schema.get("additionalProperties"),
            Some(&serde_json::json!(false))
        );
        let config = <CommandTimeouts as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(config.contains("secs: number"));
        assert!(config.contains("nanos: number"));
        let class = <CommandTimeoutClass as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(class.contains("\"long_running\""));
        assert!(!class.contains("LongRunning"));
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
