//! Independent characterization of the timing lowering before #1001.
#![allow(clippy::expect_used)]
use super::*;

type TuningSetter = fn(OperationalTuning, Duration) -> OperationalTuning;

struct TimingRequest(TimeoutClass, RetryClass, crate::raw::RawReplyShape);

impl Request for TimingRequest {
    type Class = crate::request::PlainClass;
    const MAX_SIZE: usize = 5;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
    const RETRY_CLASS: RetryClass = RetryClass::Standard;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn timeout_class(&self) -> TimeoutClass {
        self.0
    }
    fn retry_class(&self) -> RetryClass {
        self.1
    }
    fn reply_shape(&self) -> crate::raw::RawReplyShape {
        self.2
    }
    fn write_into(&self, _: CameraId, _: &mut [u8]) -> Result<usize> {
        unreachable!("timing characterization never encodes")
    }
}

#[test]
fn every_profile_class_and_override_preserves_the_characterized_context() {
    use crate::raw::RawReplyShape;
    for id in crate::profiles::ProfileId::all() {
        let profile = id.spec().expect("built-in profile");
        let timing = profile.timing();
        let registration =
            crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile.clone());
        let command_setters: [TuningSetter; 5] = [
            OperationalTuning::with_quick_timeout,
            OperationalTuning::with_movement_timeout,
            OperationalTuning::with_preset_timeout,
            OperationalTuning::with_long_running_timeout,
            OperationalTuning::with_network_timeout,
        ];
        let mut tunings = vec![OperationalTuning::new()];
        let mut all = OperationalTuning::new();
        for (setter, category) in command_setters.into_iter().zip(CommandTimeoutClass::ALL) {
            let floor = timing.command_timeouts().get(category);
            tunings.push(setter(OperationalTuning::new(), floor));
            tunings.push(setter(
                OperationalTuning::new(),
                floor + Duration::from_secs(1),
            ));
            all = setter(all, floor + Duration::from_secs(1));
        }
        let other_setters: [(TuningSetter, Duration); 4] = [
            (OperationalTuning::with_ack_timeout, timing.ack_timeout()),
            (
                OperationalTuning::with_inquiry_timeout,
                timing.inquiry_timeout(),
            ),
            (
                OperationalTuning::with_command_spacing,
                timing.command_spacing(),
            ),
            (
                OperationalTuning::with_inquiry_spacing,
                timing.inquiry_spacing(),
            ),
        ];
        for (setter, floor) in other_setters {
            tunings.push(setter(OperationalTuning::new(), floor));
            tunings.push(setter(
                OperationalTuning::new(),
                floor + Duration::from_secs(1),
            ));
            all = setter(all, floor + Duration::from_secs(1));
        }
        // Deliberately make ACK longer than twice Quick, so the two reply
        // shapes exercise different total-budget floors.
        tunings.push(OperationalTuning::new().with_ack_timeout(Duration::from_secs(900)));
        tunings.push(OperationalTuning::new().with_retry_limit(1));
        tunings.push(
            OperationalTuning::new()
                .with_initial_retry_backoff(Duration::from_secs(1))
                .with_maximum_retry_backoff(Duration::from_secs(2))
                .with_retry_budget(Duration::from_secs(900)),
        );
        tunings.push(all.with_settlement_timeout(Duration::from_secs(900)));
        for tuning in tunings {
            let validated = ValidatedTuning::validate(tuning, [&profile])
                .expect("valid characterization tuning");
            for class in CommandTimeoutClass::ALL
                .into_iter()
                .map(CommandTimeoutClass::timeout_class)
                .chain([TimeoutClass::Inquiry])
            {
                for shape in [
                    RawReplyShape::AckThenCompletion,
                    RawReplyShape::CompletionOnly,
                    RawReplyShape::NoReply,
                ] {
                    for retry in [
                        RetryClass::Never,
                        RetryClass::Standard,
                        RetryClass::Movement,
                        RetryClass::Preset,
                        RetryClass::Inquiry,
                    ] {
                        let inquiry = class == TimeoutClass::Inquiry;
                        let actual = request_context(
                            &TimingRequest(class, retry, shape),
                            &registration,
                            validated,
                            ClassSelection::Request,
                            inquiry,
                        )
                        .expect("context");
                        let ack = tuning.ack_timeout().unwrap_or_else(|| timing.ack_timeout());
                        let completion = CommandTimeoutClass::of(class).map_or(
                            timing.command_timeouts().quick_timeout(),
                            |category| {
                                tuning
                                    .command_override(category)
                                    .unwrap_or_else(|| timing.command_timeouts().get(category))
                            },
                        );
                        let inquiry_timeout = tuning
                            .inquiry_timeout()
                            .unwrap_or_else(|| timing.inquiry_timeout());
                        let deadline = if inquiry { inquiry_timeout } else { completion };
                        let attempt = if inquiry || shape != RawReplyShape::AckThenCompletion {
                            deadline
                        } else {
                            ack + completion
                        };
                        let lengthen = |override_value: Option<Duration>, floor| {
                            override_value.map_or(floor, |value| value.max(floor))
                        };
                        let total_budget = lengthen(
                            tuning.retry_budget(),
                            Duration::from_secs(10)
                                .max(deadline.saturating_mul(2))
                                .max(attempt)
                                .max(timing.busy_timeout()),
                        );
                        let base = tuning.retry_limit().map_or(3, |limit| limit.min(3));
                        let count = match class {
                            TimeoutClass::Quick | TimeoutClass::Inquiry => base + 2,
                            TimeoutClass::Movement | TimeoutClass::Preset => base,
                            TimeoutClass::Network => base.saturating_sub(1).max(1),
                            TimeoutClass::LongRunning => 1,
                        };
                        assert_eq!(actual.timeout.ack.get(), ack, "{id:?} {class:?}");
                        assert_eq!(actual.timeout.completion.get(), completion);
                        assert_eq!(actual.timeout.inquiry.get(), inquiry_timeout);
                        assert_eq!(
                            actual.timeout.cancellation.get(),
                            timing.cancellation_timeout()
                        );
                        assert_eq!(actual.timeout.ambiguity.get(), timing.ambiguity_timeout());
                        assert_eq!(
                            actual.control.minimum_spacing,
                            if inquiry {
                                tuning
                                    .inquiry_spacing()
                                    .unwrap_or_else(|| timing.inquiry_spacing())
                            } else {
                                tuning
                                    .command_spacing()
                                    .unwrap_or_else(|| timing.command_spacing())
                            }
                        );
                        assert_eq!(
                            actual.retry,
                            RetryPolicy {
                                max_retries: if retry == RetryClass::Never { 0 } else { count },
                                initial_backoff: lengthen(
                                    tuning.initial_retry_backoff(),
                                    Duration::from_millis(50)
                                ),
                                maximum_backoff: lengthen(
                                    tuning.maximum_retry_backoff(),
                                    Duration::from_millis(500).max(timing.busy_timeout())
                                ),
                                total_budget: Some(
                                    crate::runtime::engine::NonZeroDuration::new(total_budget)
                                        .expect("positive characterized budget")
                                ),
                                ack_timeout: retry != RetryClass::Never,
                                completion_timeout: retry != RetryClass::Never,
                                inquiry_timeout: retry == RetryClass::Inquiry,
                                buffer_full: retry != RetryClass::Never,
                                movement_not_executable: matches!(
                                    retry,
                                    RetryClass::Movement | RetryClass::Preset
                                ),
                                builtin_inquiry_syntax: false,
                            },
                            "{id:?} {class:?} {shape:?} {tuning:?}"
                        );
                        // The field-level independent oracle above checks all
                        // timing bytes; compare the complete inert context too.
                        assert_eq!(
                            actual,
                            RequestContext {
                                requires_extended_pan_tilt_speed: false,
                                motion: None,
                                submission_order: 0,
                                dispatch_deadline: None,
                                target: TargetSlot::CAMERA_1,
                                timeout: actual.timeout,
                                retry: actual.retry,
                                control: ControlPolicy {
                                    class: crate::runtime::engine::ControlClass::Normal,
                                    minimum_spacing: actual.control.minimum_spacing,
                                },
                                cancellation: if profile.supports_command_cancel() {
                                    CancellationPolicy::Supported
                                } else {
                                    CancellationPolicy::Unsupported
                                },
                            }
                        );
                        assert_eq!(
                            settlement_budget(class, &profile, validated),
                            tuning.settlement_timeout().unwrap_or(completion)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn resolved_retry_budgets_are_the_settlement_validation_floor() {
    for id in crate::profiles::ProfileId::all() {
        let profile = id.spec().expect("built-in profile");
        for tuning in [
            OperationalTuning::new(),
            OperationalTuning::new().with_ack_timeout(Duration::from_secs(900)),
        ] {
            let validated = ValidatedTuning::validate(tuning, [&profile]).expect("valid tuning");
            let resolved = validated.resolve(&profile.timing());
            let registration = crate::TargetRegistration::new(
                crate::IndividualCameraId::CAMERA_1,
                profile.clone(),
            );
            for category in CommandTimeoutClass::ALL {
                let request = TimingRequest(
                    category.timeout_class(),
                    RetryClass::Standard,
                    crate::raw::RawReplyShape::AckThenCompletion,
                );
                let context = request_context(
                    &request,
                    &registration,
                    validated,
                    ClassSelection::Request,
                    false,
                )
                .expect("context");
                assert_eq!(
                    context.retry.total_budget,
                    Some(resolved.retry_budget(category)),
                    "{id:?} {category:?}"
                );
            }
            let floor = resolved
                .retry_budget(CommandTimeoutClass::Movement)
                .min(resolved.retry_budget(CommandTimeoutClass::Preset))
                .get();
            profile
                .validate_tuning(tuning.with_settlement_timeout(floor))
                .expect("resolved floor is accepted");
            assert!(profile
                .validate_tuning(tuning.with_settlement_timeout(floor - Duration::from_nanos(1)))
                .is_err());
        }
    }
}
