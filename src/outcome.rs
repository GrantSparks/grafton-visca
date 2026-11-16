//! Public terminal cancellation outcomes.

/// The conclusive result of observing a cancellation request.
///
/// Cancellation can lose a race with the original operation's successful
/// completion. That exact race is reported as [`Self::Completed`], never as a
/// synthetic cancellation success.
///
/// The enum is `#[non_exhaustive]` so later releases can report further
/// honest outcomes; match it with a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum CancellationOutcome {
    /// The original operation was conclusively cancelled.
    Cancelled,
    /// The original operation completed successfully before cancellation won.
    Completed,
}

/// Evidence establishing a targeted operation's settlement.
/// Polled stability does not establish arrival at the requested endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case", deny_unknown_fields)
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum Settlement {
    /// The profile declares this operation's protocol completion exact.
    #[non_exhaustive]
    ProfileCompletion {
        /// The operation's axes.
        axes: crate::AffectedAxes,
    },
    /// The selected positions were stable across this sampling window.
    #[non_exhaustive]
    ObservedStable {
        /// The sampled axes.
        axes: crate::AffectedAxes,
        /// Minimum time from the first snapshot's end to the second's start.
        #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
        window: std::time::Duration,
        /// Maximum stable position delta for each axis.
        tolerance: crate::camera::MovementTolerance,
    },
}

/// The result of one independently dispatched STOP in an owner halt.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum HaltOutcome {
    /// The profile has no typed STOP for this axis.
    Unsupported,
    /// This axis's STOP was applied; this is protocol evidence, not physical feedback.
    Applied,
    /// Preparation, admission, protocol execution, or observation failed.
    Failed(crate::Error),
}

/// Results of one owner-ordered halt under one end-to-end deadline.
/// Older declared queued motion is superseded; later submissions run normally.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[must_use = "inspect each supported axis result"]
pub struct HaltReport {
    /// Pan/tilt STOP result.
    pub pan_tilt: HaltOutcome,
    /// Zoom STOP result.
    pub zoom: HaltOutcome,
    /// Focus STOP result.
    pub focus: HaltOutcome,
}

impl Settlement {
    /// Constructs evidence from a profile-declared operation completion.
    #[must_use]
    pub const fn profile_completion(axes: crate::AffectedAxes) -> Self {
        Self::ProfileCompletion { axes }
    }
    /// Constructs a stable-sample report. This describes evidence, not an endpoint guarantee.
    #[must_use]
    pub const fn observed_stable(
        axes: crate::AffectedAxes,
        window: std::time::Duration,
        tolerance: crate::camera::MovementTolerance,
    ) -> Self {
        Self::ObservedStable {
            axes,
            window,
            tolerance,
        }
    }
}
impl HaltReport {
    /// Constructs a report from the three independent axis outcomes.
    pub const fn new(pan_tilt: HaltOutcome, zoom: HaltOutcome, focus: HaltOutcome) -> Self {
        Self {
            pan_tilt,
            zoom,
            focus,
        }
    }
    /// Explicitly collapses the report to its first supported-axis failure.
    /// Unsupported axes are skipped. Inspect the fields to retain every outcome.
    pub fn into_result(self) -> Result<(), crate::Error> {
        for result in [self.pan_tilt, self.zoom, self.focus] {
            match result {
                HaltOutcome::Failed(error) => return Err(error),
                HaltOutcome::Unsupported | HaltOutcome::Applied => {}
            }
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "serde"))]
#[allow(clippy::unwrap_used)]
mod serialization_tests {
    use super::CancellationOutcome;

    #[test]
    fn cancellation_outcomes_serialize_and_deserialize_snake_case() {
        for (outcome, name) in [
            (CancellationOutcome::Cancelled, "cancelled"),
            (CancellationOutcome::Completed, "completed"),
        ] {
            let json = serde_json::json!(name);
            assert_eq!(serde_json::to_value(outcome).unwrap(), json);
            assert_eq!(
                serde_json::from_value::<CancellationOutcome>(json).unwrap(),
                outcome
            );
        }
        assert!(
            serde_json::from_value::<CancellationOutcome>(serde_json::json!("Cancelled")).is_err()
        );
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn outcome_schema_and_typescript_match_serde_names() {
        let schema = schemars::schema_for!(CancellationOutcome);
        let values = schema
            .get("enum")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_else(|| {
                schema
                    .get("oneOf")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|variant| variant["const"].clone())
                    .collect()
            });
        assert_eq!(
            values,
            serde_json::json!(["cancelled", "completed"])
                .as_array()
                .unwrap()
                .to_vec()
        );
        assert_eq!(
            <CancellationOutcome as ts_rs::TS>::decl(&ts_rs::Config::default()),
            "type CancellationOutcome = \"cancelled\" | \"completed\";"
        );
    }
}

#[cfg(all(test, feature = "serde"))]
#[allow(clippy::expect_used)]
mod settlement_portable_tests {
    use super::Settlement;
    use crate::types::{AffectedAxes, MovementTolerance};

    #[test]
    fn settlement_round_trips_snake_case_evidence_with_portable_duration() {
        let completion = Settlement::profile_completion(AffectedAxes::ZOOM);
        let data = serde_json::to_value(completion).expect("serialize completion");
        assert_eq!(
            data,
            serde_json::json!({"profile_completion": {"axes": serde_json::to_value(AffectedAxes::ZOOM).expect("axes")}})
        );
        assert_eq!(
            serde_json::from_value::<Settlement>(data.clone()).expect("restore completion"),
            completion
        );
        let mut legacy_name = data;
        let record = legacy_name.as_object_mut().expect("evidence record");
        let payload = record
            .remove("profile_completion")
            .expect("completion payload");
        record.insert("ProfileCompletion".to_owned(), payload);
        assert!(serde_json::from_value::<Settlement>(legacy_name).is_err());
        let stable = Settlement::observed_stable(
            AffectedAxes::PAN_TILT,
            std::time::Duration::new(2, 3),
            MovementTolerance::default(),
        );
        let data = serde_json::to_value(stable).expect("serialize stability");
        assert_eq!(
            data["observed_stable"]["window"],
            serde_json::json!({"secs": 2, "nanos": 3})
        );
        assert_eq!(
            serde_json::from_value::<Settlement>(data.clone()).expect("restore stability"),
            stable
        );
        let mut unknown = data.clone();
        unknown["observed_stable"]["extra"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Settlement>(unknown).is_err());
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn settlement_schema_and_typescript_preserve_the_evidence_record_shape() {
        let schema = serde_json::to_value(schemars::schema_for!(Settlement)).expect("schema JSON");
        let rendered = serde_json::to_string(&schema).expect("schema text");
        for name in [
            "profile_completion",
            "observed_stable",
            "axes",
            "window",
            "tolerance",
        ] {
            assert!(rendered.contains(name), "missing {name}");
        }
        let declaration = <Settlement as ts_rs::TS>::decl(&ts_rs::Config::default());
        for name in [
            "profile_completion",
            "observed_stable",
            "secs: number",
            "nanos: number",
            "MovementTolerance",
        ] {
            assert!(declaration.contains(name), "{declaration}");
        }
    }
}
