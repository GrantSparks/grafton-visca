//! Optional operational overrides with one setter and getter per setting.

use std::time::Duration;

use crate::{profiles::RetryBounds, timeout::CommandTimeoutClass};

/// A checked override for the base retry count, from zero to three.
///
/// Requests still derive their effective retry count from their class and
/// timeout category.
#[crate::value(min = 0, max = 3)]
pub struct RetryLimit(#[cfg_attr(feature = "ts-rs", ts(type = "0 | 1 | 2 | 3"))] u8);

/// Operational overrides that cannot change request or profile semantics.
///
/// Tuning contains no request class, route, decoder, cancellation capability,
/// or completion-kind fields. Fixed bounds are checked by their value types, and
/// profile-dependent bounds are validated against the profile before use.
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
    maximum_command_sockets: Option<crate::CommandSocketCount>,
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
    retry_limit: Option<RetryLimit>,
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
    pub const fn with_maximum_command_sockets(
        mut self,
        maximum: crate::CommandSocketCount,
    ) -> Self {
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
    /// The default base is 3, the largest value accepted by [`RetryLimit`].
    /// Each request's own
    /// budget is derived from this base by its timeout category: quick and
    /// inquiry work gets two more attempts, network work one fewer (at least
    /// one), and a long-running command exactly one, so the effective count is
    /// not always the number given here. A request whose retry class is
    /// [`crate::request::CommandRetryClass::Never`] or
    /// [`crate::request::InquiryRetryClass::Never`] is never replayed
    /// regardless of this value.
    #[must_use]
    pub const fn with_retry_limit(mut self, maximum: RetryLimit) -> Self {
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
    ///
    /// Retry bounds may only be lengthened. The profile validates each
    /// override before preparation, including monotonic-clock limits.
    #[must_use]
    pub const fn with_retry_budget(mut self, value: Duration) -> Self {
        self.retry_budget = Some(value);
        self
    }

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
    pub const fn maximum_command_sockets(self) -> Option<crate::CommandSocketCount> {
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
    pub const fn retry_limit(self) -> Option<RetryLimit> {
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
        self.retry_limit
            .map_or(default, |limit| u32::from(limit.value()).min(default))
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
            .with_maximum_command_sockets(crate::types::CommandSocketCount::ONE)
            .with_ack_timeout(duration)
            .with_quick_timeout(duration)
            .with_movement_timeout(duration)
            .with_preset_timeout(duration)
            .with_long_running_timeout(duration)
            .with_network_timeout(duration)
            .with_settlement_timeout(duration)
            .with_inquiry_timeout(duration)
            .with_retry_limit(RetryLimit::new(2).expect("retry limit"))
            .with_initial_retry_backoff(duration)
            .with_maximum_retry_backoff(duration)
            .with_retry_budget(duration);
        assert_eq!(
            tuning.maximum_command_sockets(),
            Some(crate::CommandSocketCount::ONE)
        );
        assert_eq!(tuning.retry_limit().map(RetryLimit::value), Some(2));
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

    #[test]
    fn retry_limit_checks_its_fixed_domain() {
        for limit in 0..=3 {
            let value = RetryLimit::new(limit).expect("in-range retry limit");
            assert_eq!(value.value(), limit);
        }
        for limit in [4, 99] {
            assert!(RetryLimit::new(limit).is_err());
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn tuning_persistence_has_fifteen_optional_settings_and_no_unchecked_fields() {
        let empty = OperationalTuning::new();
        for count in [0, 3] {
            assert!(
                serde_json::from_value::<OperationalTuning>(serde_json::json!({
                    "maximum_command_sockets": count
                }))
                .is_err()
            );
        }
        for limit in [-1, 4, 99] {
            assert!(
                serde_json::from_value::<OperationalTuning>(serde_json::json!({
                    "retry_limit": limit
                }))
                .is_err()
            );
        }
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
            .with_maximum_command_sockets(crate::types::CommandSocketCount::ONE)
            .with_quick_timeout(Duration::new(12, 345))
            .with_retry_limit(RetryLimit::new(2).expect("retry limit"))
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
        assert!(
            declaration.contains("retry_limit: RetryLimit | null"),
            "{declaration}"
        );
        let retry_limit = schemars::schema_for!(RetryLimit);
        assert_eq!(retry_limit.as_value()["minimum"], 0);
        assert_eq!(retry_limit.as_value()["maximum"], 3);
        let retry_limit = <RetryLimit as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(retry_limit.contains("0 | 1 | 2 | 3"), "{retry_limit}");
        assert!(declaration.contains("secs: number"), "{declaration}");
        assert!(declaration.contains("nanos: number"), "{declaration}");
    }
}
