//! A queued speed prerequisite is checked after older mode effects commit.

use super::*;
use crate::{
    command::{SetVariableSpeedMode, VariableSpeedMode},
    completion::AppliedOnly,
    prepared::{
        test_support::{prepare_command, prepare_operation},
        ClassSelection,
    },
    profiles::SonyFR7,
    request::builtin::PanTiltDrive,
    types::{PanSpeed, TiltSpeed},
    OperationalTuning, PanTiltDirection, ProfileSpec, TargetRegistration,
};

fn mode_request(registration: &TargetRegistration, mode: VariableSpeedMode) -> RuntimeRequest {
    prepare_command(
        &SetVariableSpeedMode::new(mode),
        registration,
        OperationalTuning::new(),
        ClassSelection::Request,
    )
    .unwrap()
    .admit_with(|request, _| request)
}

fn drive_request(registration: &TargetRegistration, tuning: OperationalTuning) -> RuntimeRequest {
    let fast = PanTiltDrive::new(
        PanTiltDirection::Right,
        PanSpeed::new(50).unwrap(),
        TiltSpeed::new(50).unwrap(),
    )
    .unwrap();
    prepare_operation::<AppliedOnly, _>(&fast, registration, tuning, ClassSelection::Request)
        .unwrap()
        .admit_with(|request, _, _, _| request)
}

fn queued_mode_refusal(invalidate: bool) {
    let registration = TargetRegistration::new(
        crate::CameraId::CAMERA_1,
        ProfileSpec::from_compile_time::<SonyFR7>().unwrap(),
    )
    .unwrap();
    let mut policy = owner_policy(EnvelopeKind::Sony);
    policy.targets[TargetSlot::CAMERA_1]
        .as_mut()
        .unwrap()
        .command_sockets = 1;
    let mut state = OwnerState::new(policy).unwrap();
    let initial = Instant::now();
    let _enabled = sent(
        &mut state,
        mode_request(&registration, VariableSpeedMode::Fine50),
        Some(1),
        initial,
    );
    apply(
        &mut state,
        frame(
            Some(1),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        initial,
    );
    apply(
        &mut state,
        frame(
            Some(1),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        initial,
    );
    let cache = crate::state_cache::StateCache::from_registry(
        state.state_cache_registry(),
        TargetSlot::CAMERA_1,
    );
    assert_eq!(cache.variable_speed_mode(), Some(VariableSpeedMode::Fine50));

    let now = initial + Duration::from_secs(1);
    let _changing = sent(
        &mut state,
        mode_request(&registration, VariableSpeedMode::Standard24),
        Some(2),
        now,
    );
    apply(
        &mut state,
        frame(
            Some(2),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        now,
    );
    let request = drive_request(&registration, OperationalTuning::new());
    let permit = state.permits().try_acquire(request.context()).unwrap();
    let (input, observer, _admitted) = super::super::stage_admission(&mut state, request, permit);
    for effect in state.input(input, now) {
        assert!(
            !matches!(state.apply_effect(effect), AppliedEffect::Transmit(_)),
            "the executing mode setter occupies the only socket"
        );
    }
    assert_eq!(
        cache.variable_speed_mode(),
        Some(VariableSpeedMode::Fine50),
        "preparation must not pretend a queued mode change has applied"
    );

    let now = now + Duration::from_millis(100);
    let response = if invalidate {
        DecodedResponse::Error {
            socket: Some(ViscaSocket::S1),
            code: 0x41,
        }
    } else {
        DecodedResponse::Completion {
            socket: Some(ViscaSocket::S1),
        }
    };
    let mut staged = None;
    for effect in state.input(frame(Some(2), response), now) {
        if let AppliedEffect::Transmit(write) = state.apply_effect(effect) {
            staged = Some(write);
        }
    }
    assert_eq!(
        cache.variable_speed_mode(),
        if invalidate {
            None
        } else {
            Some(VariableSpeedMode::Standard24)
        }
    );
    let staged = staged.expect("the queued drive becomes dispatchable after the setter ends");
    let error = state.prepare_write(&staged).unwrap_err();
    assert!(matches!(error, Error::InvalidRequest(_)));
    for effect in state.finish_write(&staged, WriteOutcome::NotSent(error), now) {
        state.apply_effect(effect);
    }
    let RuntimeOutcome::Failed(error) = observer.try_recv().unwrap() else {
        panic!("the drive must be refused");
    };
    assert_eq!(
        error.failure_context(),
        Some(crate::FailureContext::new(
            crate::FailureStage::Terminal,
            crate::Certainty::NotAccepted
        ))
    );
    assert_eq!(state.state(), SessionState::Running);
    assert_eq!(
        state.metrics().writes,
        2,
        "only the two mode setters reached the wire"
    );
    assert_eq!(state.metrics().write_failures, 0);
}

#[test]
fn a_queued_drive_observes_the_new_applied_normal_mode_at_dispatch() {
    queued_mode_refusal(false);
}

#[test]
fn a_queued_drive_observes_mode_invalidation_after_an_unconfirmed_setter() {
    queued_mode_refusal(true);
}

#[test]
fn a_retry_refused_after_mode_invalidation_preserves_the_first_attempts_uncertainty() {
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().unwrap();
    let spacing = profile.timing().minimum_command_spacing();
    let ack_timeout = profile.timing().ack_timeout();
    assert!(spacing < ack_timeout);
    let registration = TargetRegistration::new(crate::CameraId::CAMERA_1, profile).unwrap();
    let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
    let initial = Instant::now();
    let _enabled = sent(
        &mut state,
        mode_request(&registration, VariableSpeedMode::Fine50),
        Some(1),
        initial,
    );
    apply(
        &mut state,
        frame(
            Some(1),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S1),
            },
        ),
        initial,
    );
    apply(
        &mut state,
        frame(
            Some(1),
            DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        ),
        initial,
    );

    let now = initial + Duration::from_secs(1);
    let request = drive_request(&registration, OperationalTuning::new().retry_limit(1));
    let observer = sent(&mut state, request, Some(2), now);
    // The drive may have reached the camera, but it has not acknowledged.
    // A second Sony slot can process a mode change while that reply is lost.
    let mode_at = now + spacing;
    let _changing = sent(
        &mut state,
        mode_request(&registration, VariableSpeedMode::Standard24),
        Some(3),
        mode_at,
    );
    apply(
        &mut state,
        frame(
            Some(3),
            DecodedResponse::Ack {
                socket: Some(ViscaSocket::S2),
            },
        ),
        mode_at,
    );
    apply(
        &mut state,
        frame(
            Some(3),
            DecodedResponse::Error {
                socket: Some(ViscaSocket::S2),
                code: 0x41,
            },
        ),
        mode_at,
    );
    let cache = crate::state_cache::StateCache::from_registry(
        state.state_cache_registry(),
        TargetSlot::CAMERA_1,
    );
    assert_eq!(cache.variable_speed_mode(), None);

    let expiry = now + ack_timeout;
    for effect in state.advance(expiry) {
        state.apply_effect(effect);
    }
    let retry_at = state
        .next_wake()
        .expect("the retry has an engine-owned ready instant");
    let mut retry = None;
    for effect in state.advance(retry_at) {
        if let AppliedEffect::Transmit(write) = state.apply_effect(effect) {
            retry = Some(write);
        }
    }
    let retry = retry.expect("the lost Sony ACK schedules a same-sequence retry");
    let error = state.prepare_write(&retry).unwrap_err();
    for effect in state.finish_write(&retry, WriteOutcome::NotSent(error), retry_at) {
        state.apply_effect(effect);
    }
    let RuntimeOutcome::Failed(error) = observer.try_recv().unwrap() else {
        panic!("retry prerequisite failed");
    };
    assert_eq!(
        error.failure_context(),
        Some(crate::FailureContext::new(
            crate::FailureStage::Terminal,
            crate::Certainty::Unconfirmed
        ))
    );
    assert_eq!(
        state.metrics().writes,
        3,
        "the refused retry did not reach the wire"
    );
    assert_eq!(state.metrics().write_failures, 0);
    assert_eq!(state.state(), SessionState::Running);
}
