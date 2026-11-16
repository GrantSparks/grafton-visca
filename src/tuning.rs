//! Optional operational overrides with one setter and getter per setting.

use std::time::Duration;

use crate::{profiles::RetryBounds, timeout::CommandTimeoutClass};

/// Operational overrides that cannot change request or profile semantics.
///
/// Tuning contains no request class, route, decoder, cancellation capability,
/// or completion-kind fields. Every value is validated against a profile before
/// it is lowered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct OperationalTuning {
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    command_spacing: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    inquiry_spacing: Option<Duration>,
    maximum_command_sockets: Option<u8>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    ack_timeout: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    quick_timeout: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    movement_timeout: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    preset_timeout: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    long_running_timeout: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    network_timeout: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    settlement_timeout: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    inquiry_timeout: Option<Duration>,
    retry_limit: Option<u32>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    initial_retry_backoff: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    maximum_retry_backoff: Option<Duration>,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number } | null"))]
    retry_budget: Option<Duration>,
}

impl OperationalTuning {
    /// Creates tuning with no overrides.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            command_spacing: None,
            inquiry_spacing: None,
            maximum_command_sockets: None,
            ack_timeout: None,
            quick_timeout: None,
            movement_timeout: None,
            preset_timeout: None,
            long_running_timeout: None,
            network_timeout: None,
            settlement_timeout: None,
            inquiry_timeout: None,
            retry_limit: None,
            initial_retry_backoff: None,
            maximum_retry_backoff: None,
            retry_budget: None,
        }
    }

    /// Overrides command pacing, subject to the profile minimum.
    #[must_use]
    pub const fn with_command_spacing(mut self, spacing: Duration) -> Self {
        self.command_spacing = Some(spacing);
        self
    }

    /// Overrides inquiry pacing, subject to the profile minimum.
    #[must_use]
    pub const fn with_inquiry_spacing(mut self, spacing: Duration) -> Self {
        self.inquiry_spacing = Some(spacing);
        self
    }

    /// Lowers owner socket capacity without exceeding the profile limit.
    #[must_use]
    pub const fn with_maximum_command_sockets(mut self, maximum: u8) -> Self {
        self.maximum_command_sockets = Some(maximum);
        self
    }

    /// Overrides the non-zero acknowledgement deadline.
    #[must_use]
    pub const fn with_ack_timeout(mut self, timeout: Duration) -> Self {
        self.ack_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero quick-command completion deadline.
    ///
    /// Category overrides are independent; set each category that needs a
    /// different deadline.
    #[must_use]
    pub const fn with_quick_timeout(mut self, timeout: Duration) -> Self {
        self.quick_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero movement-command completion deadline.
    ///
    #[must_use]
    pub const fn with_movement_timeout(mut self, timeout: Duration) -> Self {
        self.movement_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero preset-command completion deadline.
    ///
    #[must_use]
    pub const fn with_preset_timeout(mut self, timeout: Duration) -> Self {
        self.preset_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero long-running-command completion deadline.
    ///
    #[must_use]
    pub const fn with_long_running_timeout(mut self, timeout: Duration) -> Self {
        self.long_running_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero network-command completion deadline.
    ///
    #[must_use]
    pub const fn with_network_timeout(mut self, timeout: Duration) -> Self {
        self.network_timeout = Some(timeout);
        self
    }

    /// Lengthens the default protocol-settlement budget for targeted operations.
    ///
    /// The effective budget is the larger of this override and the request's
    /// retry budget. Values below every built-in targeted operation's retry
    /// budget are refused. Use `settled_with_timeout` for an exact shorter bound.
    #[must_use]
    pub const fn with_settlement_timeout(mut self, timeout: Duration) -> Self {
        self.settlement_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero inquiry deadline.
    #[must_use]
    pub const fn with_inquiry_timeout(mut self, timeout: Duration) -> Self {
        self.inquiry_timeout = Some(timeout);
        self
    }

    /// Lowers the base bounded retry count.
    ///
    /// The default base is 3, and tuning may only reduce it:
    /// [`crate::profiles::ProfileSpec::validate_tuning`] rejects a larger value, so tuning can
    /// never make retries more aggressive than the default. Each request's own
    /// budget is derived from this base by its timeout category: quick and
    /// inquiry work gets two more attempts, network work one fewer (at least
    /// one), and a long-running command exactly one, so the effective count is
    /// not always the number given here. A request whose retry class is
    /// [`crate::request::RetryClass::Never`](crate::request::RetryClass::Never) is never replayed
    /// regardless of this value.
    #[must_use]
    pub const fn with_retry_limit(mut self, maximum: u32) -> Self {
        self.retry_limit = Some(maximum);
        self
    }

    /// Lengthens the initial retry backoff, subject to the profile floor.
    #[must_use]
    pub const fn with_initial_retry_backoff(mut self, value: Duration) -> Self {
        self.initial_retry_backoff = Some(value);
        self
    }

    /// Lengthens the retry-backoff ceiling, subject to the profile floor.
    #[must_use]
    pub const fn with_maximum_retry_backoff(mut self, value: Duration) -> Self {
        self.maximum_retry_backoff = Some(value);
        self
    }

    /// Lengthens the total retry budget, subject to the profile floor.
    #[must_use]
    pub const fn with_retry_budget(mut self, value: Duration) -> Self {
        self.retry_budget = Some(value);
        self
    }

    /// Retry bounds may only be lengthened. The profile validates each
    /// override before preparation, including monotonic-clock limits.
    /// Returns the optional command spacing override.
    #[must_use]
    pub const fn command_spacing(self) -> Option<Duration> {
        self.command_spacing
    }

    /// Returns the optional inquiry spacing override.
    #[must_use]
    pub const fn inquiry_spacing(self) -> Option<Duration> {
        self.inquiry_spacing
    }

    /// Returns the optional maximum command sockets override.
    #[must_use]
    pub const fn maximum_command_sockets(self) -> Option<u8> {
        self.maximum_command_sockets
    }

    /// Returns the optional ack timeout override.
    #[must_use]
    pub const fn ack_timeout(self) -> Option<Duration> {
        self.ack_timeout
    }

    /// Returns the optional quick timeout override.
    #[must_use]
    pub const fn quick_timeout(self) -> Option<Duration> {
        self.quick_timeout
    }

    /// Returns the optional movement timeout override.
    #[must_use]
    pub const fn movement_timeout(self) -> Option<Duration> {
        self.movement_timeout
    }

    /// Returns the optional preset timeout override.
    #[must_use]
    pub const fn preset_timeout(self) -> Option<Duration> {
        self.preset_timeout
    }

    /// Returns the optional long running timeout override.
    #[must_use]
    pub const fn long_running_timeout(self) -> Option<Duration> {
        self.long_running_timeout
    }

    /// Returns the optional network timeout override.
    #[must_use]
    pub const fn network_timeout(self) -> Option<Duration> {
        self.network_timeout
    }

    /// Returns the optional settlement timeout override.
    #[must_use]
    pub const fn settlement_timeout(self) -> Option<Duration> {
        self.settlement_timeout
    }

    /// Returns the optional inquiry timeout override.
    #[must_use]
    pub const fn inquiry_timeout(self) -> Option<Duration> {
        self.inquiry_timeout
    }

    /// Returns the optional retry limit override.
    #[must_use]
    pub const fn retry_limit(self) -> Option<u32> {
        self.retry_limit
    }

    /// Returns the optional initial retry backoff override.
    #[must_use]
    pub const fn initial_retry_backoff(self) -> Option<Duration> {
        self.initial_retry_backoff
    }

    /// Returns the optional maximum retry backoff override.
    #[must_use]
    pub const fn maximum_retry_backoff(self) -> Option<Duration> {
        self.maximum_retry_backoff
    }

    /// Returns the optional retry budget override.
    #[must_use]
    pub const fn retry_budget(self) -> Option<Duration> {
        self.retry_budget
    }

    /// The completion-deadline override for one command category.
    pub(crate) const fn command_override(self, category: CommandTimeoutClass) -> Option<Duration> {
        match category {
            CommandTimeoutClass::Quick => self.quick_timeout,
            CommandTimeoutClass::Movement => self.movement_timeout,
            CommandTimeoutClass::Preset => self.preset_timeout,
            CommandTimeoutClass::LongRunning => self.long_running_timeout,
            CommandTimeoutClass::Network => self.network_timeout,
        }
    }

    /// Applies the retry-limit override to the default base retry count. The
    /// override may only reduce it.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn reduce_retry_base(self, default: u32) -> u32 {
        self.retry_limit.map_or(default, |limit| limit.min(default))
    }
}

/// Owner-side reads of the remaining overrides; only preparation and the
/// owner policy consume them.
#[cfg(any(feature = "blocking", feature = "async"))]
impl OperationalTuning {
    pub(crate) const fn command_spacing_override(self) -> Option<Duration> {
        self.command_spacing
    }

    pub(crate) const fn inquiry_spacing_override(self) -> Option<Duration> {
        self.inquiry_spacing
    }

    pub(crate) const fn maximum_command_sockets_override(self) -> Option<u8> {
        self.maximum_command_sockets
    }

    pub(crate) const fn ack_timeout_override(self) -> Option<Duration> {
        self.ack_timeout
    }

    pub(crate) const fn settlement_timeout_override(self) -> Option<Duration> {
        self.settlement_timeout
    }

    pub(crate) const fn inquiry_timeout_override(self) -> Option<Duration> {
        self.inquiry_timeout
    }
}

impl OperationalTuning {
    /// Applies the retry-timing overrides to a request's profile retry
    /// bounds. Each override may only lengthen its bound, so a value below the
    /// request's own bound leaves that bound in force.
    pub(crate) fn lengthen_retry_bounds(self, profile: RetryBounds) -> RetryBounds {
        let lengthen = |value: Option<Duration>, bound: Duration| {
            value.map_or(bound, |value| value.max(bound))
        };
        RetryBounds {
            initial_backoff: lengthen(self.initial_retry_backoff, profile.initial_backoff),
            maximum_backoff: lengthen(self.maximum_retry_backoff, profile.maximum_backoff),
            total_budget: lengthen(self.retry_budget, profile.total_budget),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn tuning_reports_all_overrides_without_changing_their_units() {
        let duration = Duration::new(12, 345);
        let tuning = OperationalTuning::new()
            .with_command_spacing(duration)
            .with_inquiry_spacing(duration)
            .with_maximum_command_sockets(1)
            .with_ack_timeout(duration)
            .with_quick_timeout(duration)
            .with_movement_timeout(duration)
            .with_preset_timeout(duration)
            .with_long_running_timeout(duration)
            .with_network_timeout(duration)
            .with_settlement_timeout(duration)
            .with_inquiry_timeout(duration)
            .with_retry_limit(2)
            .with_initial_retry_backoff(duration)
            .with_maximum_retry_backoff(duration)
            .with_retry_budget(duration);
        assert_eq!(tuning.maximum_command_sockets(), Some(1));
        assert_eq!(tuning.retry_limit(), Some(2));
        for value in [
            tuning.command_spacing(),
            tuning.inquiry_spacing(),
            tuning.ack_timeout(),
            tuning.quick_timeout(),
            tuning.movement_timeout(),
            tuning.preset_timeout(),
            tuning.long_running_timeout(),
            tuning.network_timeout(),
            tuning.settlement_timeout(),
            tuning.inquiry_timeout(),
            tuning.initial_retry_backoff(),
            tuning.maximum_retry_backoff(),
            tuning.retry_budget(),
        ] {
            assert_eq!(value, Some(duration));
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn tuning_persistence_has_fifteen_optional_settings_and_no_unchecked_fields() {
        let empty = OperationalTuning::new();
        assert_eq!(empty, OperationalTuning::default());
        assert_eq!(
            serde_json::from_str::<OperationalTuning>("{}").expect("omitted overrides"),
            empty
        );
        let value = serde_json::to_value(empty).expect("serialize tuning");
        let object = value.as_object().expect("tuning object");
        assert_eq!(object.len(), 15);
        assert!(object.values().all(serde_json::Value::is_null));
        assert!(serde_json::from_str::<OperationalTuning>("{\"future_override\":1}").is_err());
        let expected = serde_json::json!({
            "command_spacing": null, "inquiry_spacing": null,
            "maximum_command_sockets": 1, "ack_timeout": null,
            "quick_timeout": {"secs": 12, "nanos": 345},
            "movement_timeout": null, "preset_timeout": null,
            "long_running_timeout": null, "network_timeout": null,
            "settlement_timeout": null, "inquiry_timeout": null,
            "retry_limit": 2, "initial_retry_backoff": null,
            "maximum_retry_backoff": null, "retry_budget": {"secs": 30, "nanos": 0}
        });
        let saved = OperationalTuning::new()
            .with_maximum_command_sockets(1)
            .with_quick_timeout(Duration::new(12, 345))
            .with_retry_limit(2)
            .with_retry_budget(Duration::from_secs(30));
        assert_eq!(
            serde_json::to_value(saved).expect("serialize populated tuning"),
            expected
        );
        assert_eq!(
            serde_json::from_value::<OperationalTuning>(expected).expect("restore tuning"),
            saved
        );
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn schema_and_typescript_cover_the_same_fifteen_json_keys() {
        let json = serde_json::to_value(OperationalTuning::new()).expect("serialize tuning");
        let schema = schemars::schema_for!(OperationalTuning);
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("schema properties");
        let declaration = <OperationalTuning as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert_eq!(properties.len(), 15);
        for key in json.as_object().expect("tuning object").keys() {
            assert!(properties.contains_key(key), "schema omits {key}");
            assert!(
                declaration.contains(&format!("{key}:")),
                "TypeScript omits {key}: {declaration}"
            );
        }
        assert!(declaration.contains("secs: number"), "{declaration}");
        assert!(declaration.contains("nanos: number"), "{declaration}");
    }
}
