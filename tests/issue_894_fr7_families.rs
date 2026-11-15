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
    all(feature = "dyn-api", feature = "runtime-smol")
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

#[cfg(feature = "dyn-api")]
fn other_builtin_profiles() -> [grafton_visca::ProfileSpec; 8] {
    use grafton_visca::profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100,
    };
    use grafton_visca::ProfileSpec;
    [
        ProfileSpec::from_compile_time::<GenericVisca>().expect("Generic VISCA"),
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
        ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3"),
        ProfileSpec::from_compile_time::<PtzOptics30X>().expect("30X"),
        ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC300"),
        ProfileSpec::from_compile_time::<NearusBRC300>().expect("Nearus BRC300"),
        ProfileSpec::from_compile_time::<SonyBRCH900>().expect("BRC H900"),
        ProfileSpec::from_compile_time::<SonyEVIH100>().expect("EVI H100"),
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
        camera::MotionQuery,
        command::{
            Fr7AeLevelInquiry, Fr7AutoGain, Fr7AutoGainInquiry, Fr7IrisUp, Fr7KneeMode,
            Fr7KneeParameter, Fr7KneePermission, Fr7PresetSeparateMode, Fr7PresetSpeedSelection,
            Fr7SetKneeMode, Fr7ZoomHighResolution, Fr7ZoomHighResolutionStop, WhiteBalanceMode,
        },
        completion::AppliedOnly,
        profiles::{PtzOpticsG2, SonyFR7},
        types::{
            AeLevel, BlackLevel, ClearImageZoomPosition, Fr7PresetNumber, HighResolutionZoomSpeed,
            IrisStep, KneePoint, KneeSlope, PresetDuration, PresetSpeed, TintOffset,
            WhiteBalanceGain, WhiteKelvin,
        },
        AffectedAxes, Certainty, ControlClass, Error, FailureStage, HaltOutcome, OperationCommand,
        OperationalTuning, Request, SessionConfig,
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
            let session = open!(fake, SessionConfig::from_compile_time::<SonyFR7>().expect("FR7 config")).expect("FR7 session");
            let view = session.camera::<SonyFR7>().expect("FR7 camera");
            let profile = view.profile();
            assert!(profile.supports_axes(AffectedAxes::IRIS), "relative iris is a physical axis");
            assert!(!profile.position_inquiries().supports(AffectedAxes::IRIS), "auto-iris state is not position feedback");
            assert!(profile.capabilities().exposure.as_ref().expect("exposure domain").iris_range.is_none(), "no shared absolute iris range is invented");
            let exposure = view.exposure();
            wait!(exposure.ae_level_up()).expect("AE up");
            assert_eq!(wait!(exposure.ae_level()).expect("AE level"), AeLevel::MAX);
            wait!(exposure.set_auto_gain(true)).expect("AGC on");
            assert!(wait!(exposure.auto_gain()).expect("AGC state"));
            wait!(exposure.set_auto_shutter(false)).expect("auto shutter off");
            assert!(!wait!(exposure.auto_shutter()).expect("auto shutter state"));
            assert_applied_only(&Fr7IrisUp::new(IrisStep::MAX), AffectedAxes::IRIS);
            let mut iris = wait!(exposure.iris_up_by(IrisStep::MAX)).expect("relative iris admission");
            wait!(iris.applied()).expect("relative iris application");
            // Auto-iris readback reports a mode, not the iris step's endpoint.
            wait!(exposure.set_auto_iris(true)).expect("auto iris on");
            assert!(wait!(exposure.auto_iris()).expect("auto iris state"));
            let white = view.white_balance();
            wait!(white.preset()).expect("preset white mode");
            assert_eq!(wait!(white.mode()).expect("WB mode"), WhiteBalanceMode::Preset);
            wait!(white.set_from_scene()).expect("WB SET");
            assert_eq!(wait!(white.mode()).expect("Memory A mode readback"), WhiteBalanceMode::Manual);
            wait!(white.set_preset_white(WhiteKelvin::MIN)).expect("preset white minimum");
            assert_eq!(wait!(white.preset_white()).expect("preset white readback"), WhiteKelvin::MIN);
            wait!(white.set_memory_temperature(WhiteKelvin::MAX)).expect("Memory A temperature");
            assert_eq!(wait!(white.memory_temperature()).expect("Memory A temperature readback"), WhiteKelvin::MAX);
            wait!(white.set_memory_red_gain(WhiteBalanceGain::MIN)).expect("Memory A red gain");
            assert_eq!(wait!(white.memory_red_gain()).expect("red gain readback"), WhiteBalanceGain::MIN);
            wait!(white.set_memory_blue_gain(WhiteBalanceGain::MAX)).expect("Memory A blue gain");
            assert_eq!(wait!(white.memory_blue_gain()).expect("blue gain readback"), WhiteBalanceGain::MAX);
            wait!(white.set_memory_tint(TintOffset::MAX)).expect("Memory A tint");
            assert_eq!(wait!(white.memory_tint()).expect("Memory A tint readback"), TintOffset::MAX);
            let neutral = TintOffset::new(0).expect("neutral tint");
            wait!(white.set_atw_temperature_offset(neutral)).expect("ATW temperature offset");
            assert_eq!(wait!(white.atw_temperature_offset()).expect("ATW temperature readback"), neutral);
            wait!(white.set_atw_tint_offset(TintOffset::MIN)).expect("ATW tint offset");
            assert_eq!(wait!(white.atw_tint_offset()).expect("ATW tint readback"), TintOffset::MIN);
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
            let session = open!(fake, SessionConfig::from_compile_time::<SonyFR7>().expect("FR7 config")).expect("FR7 session");
            let view = session.camera::<SonyFR7>().expect("FR7 camera");
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
            wait!(image.set_knee_mode(Fr7KneeMode::Manual)).expect("manual knee");
            assert_eq!(wait!(image.knee_mode()).expect("knee mode readback"), Fr7KneeMode::Manual);
            wait!(image.set_knee_slope(KneeSlope::MIN)).expect("knee slope");
            assert_eq!(wait!(image.knee_slope()).expect("knee slope readback"), KneeSlope::MIN);
            wait!(image.set_knee_point(KneePoint::MAX)).expect("knee point");
            assert_eq!(wait!(image.knee_point()).expect("knee point readback"), KneePoint::MAX);
            assert_eq!(wait!(image.knee_permission(Fr7KneeParameter::Slope)).expect("knee permission"), Fr7KneePermission::DisplayOnly);
            let presets = view.presets();
            wait!(presets.select_speed(Fr7PresetSpeedSelection::Separate)).expect("separate preset timing");
            assert_eq!(wait!(presets.speed_selection()).expect("preset timing readback"), Fr7PresetSpeedSelection::Separate);
            wait!(presets.set_common_speed(PresetSpeed::MAX)).expect("common preset speed");
            assert_eq!(wait!(presets.common_speed()).expect("common speed readback"), PresetSpeed::MAX);
            let preset = Fr7PresetNumber::MAX;
            wait!(presets.set_separate_mode(preset, Fr7PresetSeparateMode::Duration)).expect("separate duration mode");
            assert_eq!(wait!(presets.separate_mode(preset)).expect("separate mode readback"), Fr7PresetSeparateMode::Duration);
            wait!(presets.set_separate_speed(preset, PresetSpeed::MAX)).expect("separate preset speed");
            assert_eq!(wait!(presets.separate_speed(preset)).expect("separate speed readback"), PresetSpeed::MAX);
            wait!(presets.set_separate_duration(preset, PresetDuration::MAX)).expect("separate preset duration");
            assert_eq!(wait!(presets.separate_duration(preset)).expect("separate duration readback"), PresetDuration::MAX);
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
            let session = open!(fake, SessionConfig::from_compile_time::<SonyFR7>().expect("FR7 config")).expect("FR7 session");
            let view = session.camera::<SonyFR7>().expect("FR7 camera");
            let zoom = view.zoom();
            let mut drive = wait!(zoom.tele_high_resolution(HighResolutionZoomSpeed::MAX)).expect("high-resolution tele admission");
            wait!(drive.applied()).expect("high-resolution drive application");
            assert_eq!(wait!(zoom.clear_image_position()).expect("Clear Image Zoom readback"), ClearImageZoomPosition::MAX);
            let mut stop = wait!(zoom.stop_high_resolution()).expect("high-resolution stop admission");
            wait!(stop.applied()).expect("high-resolution stop application");
            assert_eq!(<Fr7ZoomHighResolutionStop as Request>::CONTROL_CLASS, ControlClass::Urgent);
            assert_applied_only(&Fr7ZoomHighResolutionStop, AffectedAxes::ZOOM);
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
            let config = SessionConfig::from_compile_time::<SonyFR7>().expect("FR7 config")
                .with_tuning(OperationalTuning::new().command_spacing(Duration::from_secs(1)));
            let session = open!(fake, config).expect("FR7 motion session");
            let view = session.camera::<SonyFR7>().expect("FR7 camera");
            assert_applied_only(&Fr7ZoomHighResolution::Tele(HighResolutionZoomSpeed::MIN), AffectedAxes::ZOOM);
            let mut drive = wait!(view.zoom().tele_high_resolution(HighResolutionZoomSpeed::MIN)).expect("slow high-resolution drive");
            wait!(drive.applied()).expect("drive application");
            assert!(wait!(view.motion().is_moving(MotionQuery::new(AffectedAxes::ZOOM))).expect("changing zoom position"));
            let written = fake.write_count();
            let mut queued = wait!(view.zoom().wide_high_resolution(HighResolutionZoomSpeed::MAX)).expect("queued reverse drive");
            assert_eq!(fake.write_count(), written, "ordinary motion is still paced");
            let report = wait!(view.motion().stop_all_motion()).expect("owner halt");
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
            let view = session.camera::<PtzOpticsG2>().expect("G2 camera");
            rejected_before_io(wait!(view.execute(&Fr7AutoGain::new(true))));
            rejected_before_io(wait!(view.execute(&Fr7SetKneeMode::new(Fr7KneeMode::Manual))));
            rejected_before_io(wait!(view.inquire(&Fr7AeLevelInquiry)));
            rejected_before_io(wait!(view.inquire(&Fr7AutoGainInquiry)));
            rejected_before_io(wait!(view.submit::<AppliedOnly, _>(&Fr7IrisUp::new(IrisStep::MIN))));
            rejected_before_io(wait!(view.submit::<AppliedOnly, _>(&Fr7ZoomHighResolution::Tele(HighResolutionZoomSpeed::MIN))));
            assert_eq!(fake.write_count(), 0);
            wait!(session.close()).expect("close G2 session");
        }

    }
}

#[cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_dynamic {
    use super::{other_builtin_profiles, rejected_before_io};
    use grafton_visca::{
        command::{Fr7KneeMode, Fr7KneeParameter},
        types::{
            BlackLevel, Fr7PresetNumber, HighResolutionZoomSpeed, IrisStep, PresetDuration,
            PresetSpeed, WhiteKelvin,
        },
        Executor, Session, SessionConfig,
    };
    use grafton_visca_test_support::{fake_camera::FakeCamera, runtime_matrix};

    async fn dynamic_nouns_refuse_fr7_permissions_on_every_other_builtin_before_io<E: Executor>(
        executor: E,
    ) {
        for profile in other_builtin_profiles() {
            let fake = FakeCamera::silent();
            let session = Session::open(
                fake.async_wire(),
                SessionConfig::new(profile),
                executor.clone(),
            )
            .await
            .expect("other built-in session");
            let view = session.camera_dyn().expect("runtime profile view");
            rejected_before_io(view.exposure().set_auto_gain(true).await);
            rejected_before_io(view.exposure().ae_level().await);
            rejected_before_io(view.exposure().iris_up_by(IrisStep::MIN).await);
            rejected_before_io(
                view.white_balance()
                    .set_preset_white(WhiteKelvin::MIN)
                    .await,
            );
            rejected_before_io(view.white_balance().preset_white().await);
            rejected_before_io(view.image().set_master_black(BlackLevel::MIN).await);
            rejected_before_io(view.image().master_black().await);
            rejected_before_io(view.image().set_detail(true).await);
            rejected_before_io(view.image().detail().await);
            rejected_before_io(view.image().set_knee_mode(Fr7KneeMode::Manual).await);
            rejected_before_io(view.image().knee_permission(Fr7KneeParameter::Slope).await);
            rejected_before_io(
                view.zoom()
                    .tele_high_resolution(HighResolutionZoomSpeed::MIN)
                    .await,
            );
            rejected_before_io(view.zoom().clear_image_position().await);
            rejected_before_io(view.presets().set_common_speed(PresetSpeed::MIN).await);
            rejected_before_io(view.presets().common_speed().await);
            rejected_before_io(
                view.presets()
                    .set_separate_duration(Fr7PresetNumber::MIN, PresetDuration::MIN)
                    .await,
            );
            rejected_before_io(view.presets().separate_duration(Fr7PresetNumber::MIN).await);
            assert_eq!(
                fake.write_count(),
                0,
                "every FR7 refusal precedes transport I/O"
            );
            session.close().await.expect("close other built-in session");
        }
    }

    runtime_matrix!(dynamic_nouns_refuse_fr7_permissions_on_every_other_builtin_before_io);
}

#[cfg(all(feature = "blocking", feature = "dyn-api", feature = "test-utils"))]
#[test]
fn blocking_runtime_profile_requests_refuse_fr7_permissions_before_io() {
    use grafton_visca::{
        command::{
            Fr7AeLevelInquiry, Fr7AutoGain, Fr7AutoGainInquiry, Fr7IrisUp, Fr7KneeMode,
            Fr7SetKneeMode, Fr7ZoomHighResolution,
        },
        completion::AppliedOnly,
        testing::testkit::ManualClock,
        types::{HighResolutionZoomSpeed, IrisStep},
        SessionConfig,
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
        let view = session.camera_dyn().expect("runtime profile view");
        rejected_before_io(view.execute(&Fr7AutoGain::new(true)));
        rejected_before_io(view.execute(&Fr7SetKneeMode::new(Fr7KneeMode::Manual)));
        rejected_before_io(view.inquire(&Fr7AeLevelInquiry));
        rejected_before_io(view.inquire(&Fr7AutoGainInquiry));
        rejected_before_io(view.submit::<AppliedOnly, _>(&Fr7IrisUp::new(IrisStep::MIN)));
        rejected_before_io(
            view.submit::<AppliedOnly, _>(&Fr7ZoomHighResolution::Tele(
                HighResolutionZoomSpeed::MIN,
            )),
        );
        assert_eq!(fake.write_count(), 0);
        session.close().expect("close runtime-profile session");
    }
}
