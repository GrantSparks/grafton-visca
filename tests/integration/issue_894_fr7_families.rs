//! R7 families through the real Sony-envelope owners and public nouns.
//!
//! The command and reply literals come from R7 v4.00 pp. 14-16 and 19-22.
//! Iris has no position inquiry, high-resolution zoom has no speed inquiry,
//! and Clear Image Zoom has no direct setter. Their source-backed companions
//! are exercised without treating mode or position readback as settling proof.

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    ),
    all(any(feature = "async", feature = "blocking"), feature = "runtime-smol")
))]
#![allow(clippy::expect_used)]

use grafton_visca::Error;

fn rejected_before_io<T>(result: Result<T, Error>) {
    let error = result
        .err()
        .expect("another profile must refuse the FR7 surface");
    assert!(
        matches!(error, Error::FeatureNotSupported { .. }),
        "{error:?}"
    );
}

#[cfg(any(feature = "async", feature = "blocking"))]
fn other_builtin_profiles() -> [grafton_visca::profiles::ProfileSpec; 8] {
    use grafton_visca::profiles::ProfileSpec;
    use grafton_visca::profiles::{
        GenericVisca, NearusBrc300, PtzOptics30x, PtzOpticsG2, PtzOpticsG3, SonyBrc300,
        SonyBrcH900, SonyEviH100,
    };
    [
        ProfileSpec::from_compile_time::<GenericVisca>().expect("Generic VISCA"),
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
        ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3"),
        ProfileSpec::from_compile_time::<PtzOptics30x>().expect("30X"),
        ProfileSpec::from_compile_time::<SonyBrc300>().expect("BRC300"),
        ProfileSpec::from_compile_time::<NearusBrc300>().expect("Nearus BRC300"),
        ProfileSpec::from_compile_time::<SonyBrcH900>().expect("BRC H900"),
        ProfileSpec::from_compile_time::<SonyEviH100>().expect("EVI H100"),
    ]
}

#[cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
mod paused {
    use super::rejected_before_io;
    use std::{
        collections::{HashSet, VecDeque},
        time::Duration,
    };

    use grafton_visca::{
        config::{OperationalTuning, SessionConfig},
        error::{Certainty, FailureStage},
        operation::{AppliedOnly, HaltOutcome},
        profiles::{PtzOpticsG2, SonyFr7},
        request::{ControlClass, OperationCommand, Request},
        types::{
            AeLevel, AffectedAxes, BlackLevel, ClearImageZoomPosition, HighResolutionZoomSpeed,
            IrisStep, Kelvin, KneeMode, KneeParameter, KneePermission, KneePoint, KneeSlope,
            MotionQuery, PresetNumber, PresetSeparateMode, PresetSpeed, PresetSpeedSelection,
            WhiteBalanceGain, WhiteBalanceMode, WhiteBalanceOffset,
        },
        Error,
    };

    use grafton_visca_test_support::{
        facade_matrix,
        fake_camera::{frames, FakeCamera},
    };

    #[derive(Clone, Copy)]
    enum Reply {
        Applied,
        Data(&'static [u8]),
    }

    #[derive(Clone, Copy)]
    struct Exchange {
        request: &'static [u8],
        reply: Reply,
    }

    const fn command(request: &'static [u8]) -> Exchange {
        Exchange {
            request,
            reply: Reply::Applied,
        }
    }

    const fn inquiry(request: &'static [u8], reply: &'static [u8]) -> Exchange {
        Exchange {
            request,
            reply: Reply::Data(reply),
        }
    }

    fn scripted_camera(exchanges: Vec<Exchange>) -> FakeCamera {
        let mut remaining: VecDeque<_> = exchanges.into();
        FakeCamera::visca(move |request, answer| {
            let exchange = remaining.pop_front().expect("no extra or retried packet");
            assert_eq!(request, exchange.request, "R7 command/inquiry packet");
            match exchange.reply {
                Reply::Applied => {
                    answer.reply([0x90, 0x41, 0xff]).reply([0x90, 0x51, 0xff]);
                }
                Reply::Data(reply) => {
                    answer.reply(reply);
                }
            }
        })
    }

    fn assert_sony_writes(fake: &FakeCamera, expected: usize) {
        let writes = fake.writes();
        assert_eq!(
            writes.len(),
            expected,
            "one transmission per scripted packet"
        );
        let mut sequences = HashSet::new();
        for write in writes {
            let (sequence, payload) = frames::sony_split(&write).expect("FR7 uses a Sony envelope");
            assert!(
                sequences.insert(sequence),
                "independent packets have independent sequences"
            );
            assert_eq!(payload.first(), Some(&0x81));
            assert_eq!(payload.last(), Some(&0xff));
        }
    }

    fn assert_applied_only<T: OperationCommand<AppliedOnly>>(request: &T, axes: AffectedAxes) {
        assert_eq!(request.affected_axes(), axes);
    }

    facade_matrix! {
        paused:

        fn fr7_exposure_and_white_balance_families_use_r7_packets() {
            let script = vec![
                command(&[0x81, 1, 4, 0x0e, 2, 0xff]),
                inquiry(&[0x81, 9, 4, 0x4e, 0xff], &[0x90, 0x50, 0, 0, 1, 8, 0xff]),
                command(&[0x81, 1, 0x7e, 1, 0x75, 2, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 1, 0x75, 0xff], &[0x90, 0x50, 2, 0xff]),
                command(&[0x81, 1, 5, 0x35, 3, 0xff]),
                inquiry(&[0x81, 9, 5, 0x35, 0xff], &[0x90, 0x50, 3, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x4b, 2, 0xf, 0xf, 0xff]),
                command(&[0x81, 1, 5, 0x34, 2, 0xff]),
                inquiry(&[0x81, 9, 5, 0x34, 0xff], &[0x90, 0x50, 2, 0xff]),
                command(&[0x81, 1, 4, 0x35, 0x0a, 0xff]),
                inquiry(&[0x81, 9, 4, 0x35, 0xff], &[0x90, 0x50, 0x0a, 0xff]),
                command(&[0x81, 1, 4, 0x35, 5, 0xff]),
                inquiry(&[0x81, 9, 4, 0x35, 0xff], &[0x90, 0x50, 5, 0xff]),
                command(&[0x81, 1, 4, 0x10, 5, 0xff]),
                inquiry(&[0x81, 9, 4, 0x35, 0xff], &[0x90, 0x50, 5, 0xff]),
                command(&[0x81, 1, 5, 0x43, 1, 0, 7, 0xd, 0, 0xff]),
                inquiry(&[0x81, 9, 5, 0x43, 1, 0xff], &[0x90, 0x50, 0, 7, 0xd, 0, 0xff]),
                command(&[0x81, 1, 5, 0x43, 2, 3, 0xa, 9, 8, 0xff]),
                inquiry(&[0x81, 9, 5, 0x43, 2, 0xff], &[0x90, 0x50, 3, 0xa, 9, 8, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x46, 2, 0, 0, 0, 0, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x46, 2, 0xff], &[0x90, 0x50, 0, 0, 0, 0, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x56, 2, 0, 7, 0xb, 0xc, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x56, 2, 0xff], &[0x90, 0x50, 0, 7, 0xb, 0xc, 0xff]),
                command(&[0x81, 1, 5, 0x44, 2, 0, 0, 0xc, 6, 0xff]),
                inquiry(&[0x81, 9, 5, 0x44, 2, 0xff], &[0x90, 0x50, 0, 0, 0xc, 6, 0xff]),
                command(&[0x81, 1, 5, 0x45, 0, 0, 0, 6, 3, 0xff]),
                inquiry(&[0x81, 9, 5, 0x45, 0, 0xff], &[0x90, 0x50, 0, 0, 6, 3, 0xff]),
                command(&[0x81, 1, 5, 0x46, 0, 0, 0, 0, 0, 0xff]),
                inquiry(&[0x81, 9, 5, 0x46, 0, 0xff], &[0x90, 0x50, 0, 0, 0, 0, 0xff]),
            ];
            let expected = script.len();
            let fake = scripted_camera(script);
            let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config")).expect("FR7 session");
            let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("FR7 camera");
            let profile = view.profile();
            assert!(profile.supports_axes(AffectedAxes::IRIS), "relative iris is a physical axis");
            assert!(!profile.position_inquiries().is_some_and(|axes| axes.contains(AffectedAxes::IRIS)), "auto-iris state is not position feedback");
            assert!(profile.capabilities().exposure.as_ref().expect("exposure domain").iris_range.is_none(), "no shared absolute iris range is invented");
            let exposure = view.exposure();
            wait!(exposure.ae_level_up()).expect("AE up");
            assert_eq!(wait!(exposure.ae_level()).expect("AE level"), AeLevel::MAX);
            wait!(exposure.set_auto_gain(true)).expect("AGC on");
            assert!(wait!(exposure.auto_gain()).expect("AGC state"));
            wait!(exposure.set_auto_shutter(false)).expect("auto shutter off");
            assert!(!wait!(exposure.auto_shutter()).expect("auto shutter state"));
            assert_applied_only(&grafton_visca::command::exposure::IrisUpBy::new(IrisStep::MAX), AffectedAxes::IRIS);
            let mut iris = wait!(exposure.iris_up_by(IrisStep::MAX)).expect("relative iris admission");
            wait!(iris.applied()).expect("relative iris application");
            // Auto-iris readback reports a mode, not the iris step's endpoint.
            wait!(exposure.set_auto_iris(true)).expect("auto iris on");
            assert!(wait!(exposure.auto_iris()).expect("auto iris state"));
            let white = view.white_balance();
            wait!(white.set_mode(WhiteBalanceMode::Preset)).expect("preset white mode");
            assert_eq!(wait!(white.mode()).expect("WB mode"), WhiteBalanceMode::Preset);
            wait!(white.set_mode(WhiteBalanceMode::Manual)).expect("select Memory A");
            assert_eq!(wait!(white.mode()).expect("Memory A before WB SET"), WhiteBalanceMode::Manual);
            wait!(white.one_push()).expect("WB SET");
            assert_eq!(wait!(white.mode()).expect("WB SET preserves Memory A mode"), WhiteBalanceMode::Manual);
            wait!(white.set_preset_white(Kelvin::MIN)).expect("preset white minimum");
            assert_eq!(wait!(white.preset_white()).expect("preset white readback"), Kelvin::MIN);
            wait!(white.set_memory_color_temperature(Kelvin::MAX)).expect("Memory A temperature");
            assert_eq!(wait!(white.memory_color_temperature()).expect("Memory A temperature readback"), Kelvin::MAX);
            wait!(white.set_memory_red_gain(WhiteBalanceGain::MIN)).expect("Memory A red gain");
            assert_eq!(wait!(white.memory_red_gain()).expect("red gain readback"), WhiteBalanceGain::MIN);
            wait!(white.set_memory_blue_gain(WhiteBalanceGain::MAX)).expect("Memory A blue gain");
            assert_eq!(wait!(white.memory_blue_gain()).expect("blue gain readback"), WhiteBalanceGain::MAX);
            wait!(white.set_memory_tint(WhiteBalanceOffset::MAX)).expect("Memory A tint");
            assert_eq!(wait!(white.memory_tint()).expect("Memory A tint readback"), WhiteBalanceOffset::MAX);
            let neutral = WhiteBalanceOffset::new(0).expect("neutral tint");
            wait!(white.set_atw_color_temperature_offset(neutral)).expect("ATW temperature offset");
            assert_eq!(wait!(white.atw_color_temperature_offset()).expect("ATW temperature readback"), neutral);
            wait!(white.set_atw_tint_offset(WhiteBalanceOffset::MIN)).expect("ATW tint offset");
            assert_eq!(wait!(white.atw_tint_offset()).expect("ATW tint readback"), WhiteBalanceOffset::MIN);
            assert_sony_writes(&fake, expected);
            wait!(session.close()).expect("close FR7 session");
        }

        fn fr7_black_detail_knee_and_preset_families_use_r7_packets() {
            let script = vec![
                command(&[0x81, 1, 5, 0x48, 0, 0, 0, 0, 0xff]),
                inquiry(&[0x81, 9, 5, 0x48, 0xff], &[0x90, 0x50, 0, 0, 0, 0, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x43, 0, 7, 0xb, 0xc, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x43, 0xff], &[0x90, 0x50, 0, 7, 0xb, 0xc, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x44, 0, 3, 0xd, 0xe, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x44, 0xff], &[0x90, 0x50, 0, 3, 0xd, 0xe, 0xff]),
                command(&[0x81, 1, 0x7e, 1, 0x60, 2, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 1, 0x60, 0xff], &[0x90, 0x50, 2, 0xff]),
                command(&[0x81, 1, 0x7e, 1, 0x6d, 2, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 1, 0x6d, 0xff], &[0x90, 0x50, 2, 0xff]),
                command(&[0x81, 1, 0x7e, 1, 0x54, 4, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 1, 0x54, 0xff], &[0x90, 0x50, 4, 0xff]),
                command(&[0x81, 1, 0x7e, 1, 0x6f, 0, 0, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 1, 0x6f, 0xff], &[0x90, 0x50, 0, 0, 0, 0, 0xff]),
                command(&[0x81, 1, 0x7e, 1, 0x6e, 2, 2, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 1, 0x6e, 0xff], &[0x90, 0x50, 0, 0, 2, 2, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x6d, 3, 0xff], &[0x90, 0x50, 2, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x1b, 1, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x1b, 0xff], &[0x90, 0x50, 1, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x1c, 3, 2, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x1c, 0xff], &[0x90, 0x50, 3, 2, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x27, 0x63, 1, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x27, 0x63, 0xff], &[0x90, 0x50, 1, 0xff]),
                command(&[0x81, 1, 0x7e, 1, 0x0b, 0x63, 0x32, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 1, 0x0b, 0x63, 0xff], &[0x90, 0x50, 0x32, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x67, 0x63, 3, 0xd, 0xe, 0xff]),
                inquiry(&[0x81, 9, 0x7e, 4, 0x67, 0x63, 0xff], &[0x90, 0x50, 3, 0xd, 0xe, 0xff]),
            ];
            let expected = script.len();
            let fake = scripted_camera(script);
            let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config")).expect("FR7 session");
            let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("FR7 camera");
            let image = view.image();
            wait!(image.set_master_black(BlackLevel::MIN)).expect("master black");
            assert_eq!(wait!(image.master_black()).expect("master black readback"), BlackLevel::MIN);
            wait!(image.set_red_black(BlackLevel::MAX)).expect("red black");
            assert_eq!(wait!(image.red_black()).expect("red black readback"), BlackLevel::MAX);
            let neutral = BlackLevel::new(0).expect("neutral black");
            wait!(image.set_blue_black(neutral)).expect("blue black");
            assert_eq!(wait!(image.blue_black()).expect("blue black readback"), neutral);
            wait!(image.set_detail(true)).expect("detail on");
            assert!(wait!(image.detail()).expect("detail state"));
            wait!(image.set_knee(true)).expect("knee on");
            assert!(wait!(image.knee()).expect("knee state"));
            wait!(image.set_knee_mode(KneeMode::Manual)).expect("manual knee");
            assert_eq!(wait!(image.knee_mode()).expect("knee mode readback"), KneeMode::Manual);
            wait!(image.set_knee_slope(KneeSlope::MIN)).expect("knee slope");
            assert_eq!(wait!(image.knee_slope()).expect("knee slope readback"), KneeSlope::MIN);
            wait!(image.set_knee_point(KneePoint::MAX)).expect("knee point");
            assert_eq!(wait!(image.knee_point()).expect("knee point readback"), KneePoint::MAX);
            assert_eq!(wait!(image.knee_permission(KneeParameter::Slope)).expect("knee permission"), KneePermission::DisplayOnly);
            let presets = view.presets();
            wait!(presets.set_speed_selection(PresetSpeedSelection::Separate)).expect("separate preset timing");
            assert_eq!(wait!(presets.speed_selection()).expect("preset timing readback"), PresetSpeedSelection::Separate);
            wait!(presets.set_recall_speed(PresetSpeed::MAX)).expect("common preset speed");
            assert_eq!(wait!(presets.recall_speed()).expect("common speed readback"), PresetSpeed::MAX);
            let preset = PresetNumber::new(99).expect("FR7 preset maximum");
            wait!(presets.set_separate_mode(preset, PresetSeparateMode::Duration)).expect("separate duration mode");
            assert_eq!(wait!(presets.separate_mode(preset)).expect("separate mode readback"), PresetSeparateMode::Duration);
            wait!(presets.set_separate_speed(preset, PresetSpeed::MAX)).expect("separate preset speed");
            assert_eq!(wait!(presets.separate_speed(preset)).expect("separate speed readback"), PresetSpeed::MAX);
            wait!(presets.set_separate_duration(preset, Duration::from_secs(99))).expect("separate preset duration");
            assert_eq!(wait!(presets.separate_duration(preset)).expect("separate duration readback"), Duration::from_secs(99));
            assert_sony_writes(&fake, expected);
            wait!(session.close()).expect("close FR7 session");
        }

        fn fr7_high_resolution_zoom_and_clear_image_readback_are_distinct() {
            let script = vec![
                command(&[0x81, 1, 0x7e, 4, 0x17, 2, 7, 0xf, 0xf, 0xe, 0xff]),
                inquiry(&[0x81, 9, 4, 0x47, 0xff], &[0x90, 0x50, 6, 0, 0, 0, 0xff]),
                command(&[0x81, 1, 0x7e, 4, 0x17, 0, 0, 0, 0, 0, 0xff]),
            ];
            let expected = script.len();
            let fake = scripted_camera(script);
            let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config")).expect("FR7 session");
            let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("FR7 camera");
            let zoom = view.zoom();
            let mut drive = wait!(zoom.tele_high_resolution(HighResolutionZoomSpeed::MAX)).expect("high-resolution tele admission");
            wait!(drive.applied()).expect("high-resolution drive application");
            assert_eq!(wait!(zoom.clear_image_position()).expect("Clear Image Zoom readback"), ClearImageZoomPosition::MAX);
            let mut stop = wait!(zoom.stop_high_resolution()).expect("high-resolution stop admission");
            wait!(stop.applied()).expect("high-resolution stop application");
            assert_eq!(<grafton_visca::command::zoom::StopHighResolution as Request>::CONTROL_CLASS, ControlClass::Urgent);
            assert_applied_only(&grafton_visca::command::zoom::StopHighResolution, AffectedAxes::ZOOM);
            assert_sony_writes(&fake, expected);
            wait!(session.close()).expect("close FR7 session");
        }

        fn high_resolution_zoom_is_observed_and_queued_motion_is_halt_fenced() {
            const DRIVE: &[u8] = &[0x81, 1, 0x7e, 4, 0x17, 2, 0, 0, 0, 0, 0xff];
            const PAN_TILT_STOP: &[u8] = &[0x81, 1, 6, 1, 12, 10, 3, 3, 0xff];
            const ZOOM_STOP: &[u8] = &[0x81, 1, 4, 7, 0, 0xff];
            const FOCUS_STOP: &[u8] = &[0x81, 1, 4, 8, 0, 0xff];
            let mut moving = false;
            let mut sample = 0;
            let fake = FakeCamera::visca(move |request, answer| {
                if request == [0x81, 9, 4, 0x47, 0xff] {
                    sample += 1;
                    let payload = if moving && sample % 2 == 1 { [4, 0, 0, 0] } else { [4, 1, 0, 0] };
                    answer.reply([&[0x90, 0x50][..], &payload, &[0xff]].concat());
                } else {
                    assert!([DRIVE, PAN_TILT_STOP, ZOOM_STOP, FOCUS_STOP].contains(&request), "unexpected or unfenced motion: {request:02x?}");
                    if request == DRIVE { moving = true; }
                    if request == ZOOM_STOP { moving = false; }
                    answer.reply([0x90, 0x41, 0xff]).reply([0x90, 0x51, 0xff]);
                }
            });
            let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config")
                .with_tuning(OperationalTuning::new().with_command_spacing(Duration::from_secs(1)));
            let session = open!(fake, config).expect("FR7 motion session");
            let view = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("FR7 camera");
            assert_applied_only(&grafton_visca::command::zoom::TeleHighResolution::new(HighResolutionZoomSpeed::MIN), AffectedAxes::ZOOM);
            let mut drive = wait!(view.zoom().tele_high_resolution(HighResolutionZoomSpeed::MIN)).expect("slow high-resolution drive");
            wait!(drive.applied()).expect("drive application");
            assert!(wait!(view.motion().is_moving(MotionQuery::new(AffectedAxes::ZOOM))).expect("changing zoom position"));
            let written = fake.write_count();
            let mut queued = wait!(view.zoom().wide_high_resolution(HighResolutionZoomSpeed::MAX)).expect("queued reverse drive");
            assert_eq!(fake.write_count(), written, "ordinary motion is still paced");
            let report = wait!(view.motion().stop_all()).expect("owner halt");
            assert!(matches!(&report.zoom, HaltOutcome::Applied));
            report.into_result().expect("every supported STOP applied");
            let queued_outcome = wait!(queued.applied());
            assert!(matches!(&queued_outcome, Err(Error::MotionSuperseded { axes, context, .. })
                if *axes == AffectedAxes::MOVEMENT
                    && context.stage == FailureStage::Terminal
                    && context.certainty == Certainty::NotAccepted), "{queued_outcome:?}");
            assert!(!wait!(view.motion().is_moving(MotionQuery::new(AffectedAxes::ZOOM))).expect("stable stopped zoom position"));
            let payloads = fake.writes().iter().map(|write| frames::sony_split(write).expect("Sony framing").1.to_vec()).collect::<Vec<_>>();
            assert_eq!(payloads.iter().filter(|packet| packet.as_slice() == DRIVE).count(), 1);
            for stop in [PAN_TILT_STOP, ZOOM_STOP, FOCUS_STOP] {
                assert_eq!(payloads.iter().filter(|packet| packet.as_slice() == stop).count(), 1);
            }
            assert_eq!(payloads.len(), 8, "drive, four samples and three STOPs; no queued reverse drive");
            assert_sony_writes(&fake, 8);
            wait!(session.close()).expect("close motion session");
        }

        fn static_extension_requests_refuse_fr7_permissions_before_io() {
            let fake = FakeCamera::silent();
            let session = open!(fake, SessionConfig::from_compile_time::<PtzOpticsG2>().expect("G2 config")).expect("G2 session");
            let view = session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1).expect("G2 camera");
            rejected_before_io(wait!(view.execute(&grafton_visca::command::exposure::SetAutoGain::new(true))));
            rejected_before_io(wait!(view.execute(&grafton_visca::command::image::SetKneeMode::new(KneeMode::Manual))));
            rejected_before_io(wait!(view.inquire(&grafton_visca::command::exposure::AeLevelInquiry)));
            rejected_before_io(wait!(view.inquire(&grafton_visca::command::exposure::AutoGainInquiry)));
            rejected_before_io(wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::exposure::IrisUpBy::new(IrisStep::MIN))));
            rejected_before_io(wait!(view.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::TeleHighResolution::new(HighResolutionZoomSpeed::MIN))));
            assert_eq!(fake.write_count(), 0);
            wait!(session.close()).expect("close G2 session");
        }

    }
}

#[cfg(all(
    any(feature = "async", feature = "blocking"),
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_runtime_profiles {
    use super::{other_builtin_profiles, rejected_before_io};
    use grafton_visca::{
        config::SessionConfig,
        r#async::Session,
        runtime::Executor,
        types::{
            BlackLevel, HighResolutionZoomSpeed, IrisStep, Kelvin, KneeMode, KneeParameter,
            PresetNumber, PresetSpeed,
        },
    };
    use grafton_visca_test_support::{fake_camera::FakeCamera, runtime_matrix};
    use std::time::Duration;

    async fn runtime_nouns_refuse_fr7_permissions_on_every_other_builtin_before_io<E: Executor>(
        executor: E,
    ) {
        for profile in other_builtin_profiles() {
            let recall_target = grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
                profile.clone(),
            )
            .expect("recall target");
            let supports_recall_speed = profile
                .capabilities()
                .supports(grafton_visca::capabilities::TypedSupportSurface::PresetRecallSpeed);
            let fake = FakeCamera::silent();
            let session = Session::open(
                fake.async_wire(),
                &SessionConfig::new(profile),
                executor.clone(),
            )
            .await
            .expect("other built-in session");
            let view = session
                .camera::<grafton_visca::profiles::AnyProfile>(
                    grafton_visca::types::IndividualCameraId::CAMERA_1,
                )
                .expect("runtime profile view");
            rejected_before_io(view.exposure().set_auto_gain(true).await);
            rejected_before_io(view.exposure().ae_level().await);
            rejected_before_io(view.exposure().iris_up_by(IrisStep::MIN).await);
            rejected_before_io(view.white_balance().set_preset_white(Kelvin::MIN).await);
            rejected_before_io(view.white_balance().preset_white().await);
            rejected_before_io(view.image().set_master_black(BlackLevel::MIN).await);
            rejected_before_io(view.image().master_black().await);
            rejected_before_io(view.image().set_detail(true).await);
            rejected_before_io(view.image().detail().await);
            rejected_before_io(view.image().set_knee_mode(KneeMode::Manual).await);
            rejected_before_io(view.image().knee_permission(KneeParameter::Slope).await);
            rejected_before_io(
                view.zoom()
                    .tele_high_resolution(HighResolutionZoomSpeed::MIN)
                    .await,
            );
            rejected_before_io(view.zoom().clear_image_position().await);
            if supports_recall_speed {
                grafton_visca::command::presets::SetRecallSpeed::for_target(
                    &recall_target,
                    PresetSpeed::MIN,
                )
                .expect("PTZ and FR7 share the neutral recall-speed setting");
            } else {
                rejected_before_io(view.presets().set_recall_speed(PresetSpeed::MIN).await);
            }
            rejected_before_io(view.presets().recall_speed().await);
            rejected_before_io(
                view.presets()
                    .set_separate_duration(PresetNumber::MIN, Duration::from_secs(1))
                    .await,
            );
            rejected_before_io(view.presets().separate_duration(PresetNumber::MIN).await);
            assert_eq!(
                fake.write_count(),
                0,
                "every FR7 refusal precedes transport I/O"
            );
            session.close().await.expect("close other built-in session");
        }
    }

    runtime_matrix!(runtime_nouns_refuse_fr7_permissions_on_every_other_builtin_before_io);
}

#[cfg(all(
    feature = "blocking",
    any(feature = "async", feature = "blocking"),
    feature = "test-utils"
))]
#[test]
fn blocking_runtime_profile_requests_refuse_fr7_permissions_before_io() {
    use grafton_visca::{
        config::SessionConfig,
        operation::AppliedOnly,
        testing::testkit::ManualClock,
        types::{HighResolutionZoomSpeed, IrisStep, KneeMode},
    };
    let clock = ManualClock::new();
    let participant = clock.enter();
    for profile in other_builtin_profiles() {
        let fake = grafton_visca_test_support::fake_camera::FakeCamera::silent();
        let session = clock
            .open_blocking_session(
                &participant,
                fake.blocking_wire().on_clock(&clock),
                SessionConfig::new(profile),
            )
            .expect("runtime-profile blocking session");
        let view = session
            .camera::<grafton_visca::profiles::AnyProfile>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("runtime profile view");
        rejected_before_io(view.execute(&grafton_visca::command::exposure::SetAutoGain::new(true)));
        rejected_before_io(
            view.execute(&grafton_visca::command::image::SetKneeMode::new(
                KneeMode::Manual,
            )),
        );
        rejected_before_io(view.inquire(&grafton_visca::command::exposure::AeLevelInquiry));
        rejected_before_io(view.inquire(&grafton_visca::command::exposure::AutoGainInquiry));
        rejected_before_io(view.submit::<AppliedOnly, _>(
            &grafton_visca::command::exposure::IrisUpBy::new(IrisStep::MIN),
        ));
        rejected_before_io(view.submit::<AppliedOnly, _>(
            &grafton_visca::command::zoom::TeleHighResolution::new(HighResolutionZoomSpeed::MIN),
        ));
        assert_eq!(fake.write_count(), 0);
        session.close().expect("close runtime-profile session");
    }
}
