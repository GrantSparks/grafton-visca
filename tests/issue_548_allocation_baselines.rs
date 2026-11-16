//! Allocation characterization at the stable 2.0 request and framing boundaries.

#![allow(unsafe_code, clippy::expect_used, clippy::unwrap_used)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::Mutex,
};

use bytes::BytesMut;
use grafton_visca::{
    observability::MetricsSnapshot,
    request::Request,
    transport::{AddressingMode, Envelope, RawVisca, SonyEncapsulated},
    types::{CameraId, PanSpeed, PanTiltDirection, PresetNumber, TiltSpeed, ZoomSpeed},
    wire::CommandKind,
};

static ALLOCATION_TEST_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|count| count.set(count.get().saturating_add(1)));
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

struct CountingGuard;

impl Drop for CountingGuard {
    fn drop(&mut self) {
        let _ = COUNTING.try_with(|counting| counting.set(false));
    }
}

fn allocations_during(f: impl FnOnce()) -> usize {
    let _lock = ALLOCATION_TEST_LOCK.lock().expect("allocation lock");
    COUNT.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    {
        let _guard = CountingGuard;
        f();
    }
    COUNT.with(Cell::get)
}

fn assert_request_encoding_does_not_allocate<R: Request>(label: &str, request: &R) {
    let mut buffer = [0_u8; 64];
    let allocations = allocations_during(|| {
        let len = request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("typed request must encode");
        std::hint::black_box(&buffer[..len]);
    });
    assert_eq!(allocations, 0, "{label} Request::write_into allocated");
}

macro_rules! assert_generated_inquiry_inventory {
    ($(($label:literal, $inquiry:expr)),+ $(,)?) => {
        #[test]
        fn all_queryable_generated_inquiries_encode_without_heap_allocation() {
            const QUERYABLE_GENERATED_INQUIRY_COUNT: usize = [$(stringify!($inquiry)),+].len();
            assert_eq!(QUERYABLE_GENERATED_INQUIRY_COUNT, 73);
            $(
                let inquiry = $inquiry;
                assert_request_encoding_does_not_allocate($label, &inquiry);
            )+
        }
    };
}

// Keep this list synchronized with the queryable section of the built-in
// inquiry table. Retired, unsourced requests are deliberately absent.
assert_generated_inquiry_inventory!(
    ("power.status", grafton_visca::command::power::StatusInquiry),
    (
        "zoom.position",
        grafton_visca::command::zoom::PositionInquiry
    ),
    (
        "zoom.clear_image_position",
        grafton_visca::command::zoom::ClearImagePositionInquiry
    ),
    (
        "system.version",
        grafton_visca::command::system::VersionInquiry
    ),
    (
        "pan_tilt.position",
        grafton_visca::command::pan_tilt::PositionInquiry
    ),
    (
        "focus.position",
        grafton_visca::command::focus::PositionInquiry
    ),
    ("focus.mode", grafton_visca::command::focus::ModeInquiry),
    (
        "focus.near_limit",
        grafton_visca::command::focus::NearLimitInquiry
    ),
    ("focus.zone", grafton_visca::command::focus::ZoneInquiry),
    (
        "focus.sensitivity",
        grafton_visca::command::focus::SensitivityInquiry
    ),
    (
        "presets.speed_selection",
        grafton_visca::command::presets::SpeedSelectionInquiry
    ),
    (
        "presets.recall_speed",
        grafton_visca::command::presets::RecallSpeedInquiry
    ),
    (
        "presets.separate_mode",
        grafton_visca::command::presets::SeparateModeInquiry::new(
            grafton_visca::types::PresetNumber::MIN
        )
    ),
    (
        "presets.separate_speed",
        grafton_visca::command::presets::SeparateSpeedInquiry::new(
            grafton_visca::types::PresetNumber::MIN
        )
    ),
    (
        "presets.separate_duration",
        grafton_visca::command::presets::SeparateDurationInquiry::new(
            grafton_visca::types::PresetNumber::MIN
        )
    ),
    (
        "exposure.mode",
        grafton_visca::command::exposure::ModeInquiry
    ),
    (
        "exposure.shutter",
        grafton_visca::command::exposure::ShutterInquiry
    ),
    (
        "exposure.compensation",
        grafton_visca::command::exposure::CompensationInquiry
    ),
    (
        "exposure.compensation_enabled",
        grafton_visca::command::exposure::CompensationEnabledInquiry
    ),
    (
        "exposure.iris",
        grafton_visca::command::exposure::IrisInquiry
    ),
    (
        "exposure.brightness",
        grafton_visca::command::exposure::BrightnessInquiry
    ),
    (
        "exposure.gain",
        grafton_visca::command::exposure::GainInquiry
    ),
    (
        "exposure.gain_limit",
        grafton_visca::command::exposure::GainLimitInquiry
    ),
    (
        "exposure.anti_flicker",
        grafton_visca::command::exposure::AntiFlickerInquiry
    ),
    (
        "exposure.ae_level",
        grafton_visca::command::exposure::AeLevelInquiry
    ),
    (
        "exposure.auto_gain",
        grafton_visca::command::exposure::AutoGainInquiry
    ),
    (
        "exposure.auto_shutter",
        grafton_visca::command::exposure::AutoShutterInquiry
    ),
    (
        "exposure.auto_iris",
        grafton_visca::command::exposure::AutoIrisInquiry
    ),
    (
        "white_balance.mode",
        grafton_visca::command::white_balance::ModeInquiry
    ),
    (
        "white_balance.sensitivity",
        grafton_visca::command::white_balance::SensitivityInquiry
    ),
    (
        "white_balance.color_temperature",
        grafton_visca::command::white_balance::ColorTemperatureInquiry
    ),
    (
        "white_balance.red_gain",
        grafton_visca::command::white_balance::RedGainInquiry
    ),
    (
        "white_balance.blue_gain",
        grafton_visca::command::white_balance::BlueGainInquiry
    ),
    (
        "white_balance.red_tuning",
        grafton_visca::command::white_balance::RedTuningInquiry
    ),
    (
        "white_balance.blue_tuning",
        grafton_visca::command::white_balance::BlueTuningInquiry
    ),
    (
        "white_balance.preset_white",
        grafton_visca::command::white_balance::PresetWhiteInquiry
    ),
    (
        "white_balance.memory_color_temperature",
        grafton_visca::command::white_balance::MemoryColorTemperatureInquiry
    ),
    (
        "white_balance.memory_red_gain",
        grafton_visca::command::white_balance::MemoryRedGainInquiry
    ),
    (
        "white_balance.memory_blue_gain",
        grafton_visca::command::white_balance::MemoryBlueGainInquiry
    ),
    (
        "white_balance.memory_tint",
        grafton_visca::command::white_balance::MemoryTintInquiry
    ),
    (
        "white_balance.atw_color_temperature_offset",
        grafton_visca::command::white_balance::AtwColorTemperatureOffsetInquiry
    ),
    (
        "white_balance.atw_tint_offset",
        grafton_visca::command::white_balance::AtwTintOffsetInquiry
    ),
    (
        "image.saturation",
        grafton_visca::command::image::SaturationInquiry
    ),
    ("image.hue", grafton_visca::command::image::HueInquiry),
    (
        "image.luminance",
        grafton_visca::command::image::LuminanceInquiry
    ),
    (
        "image.contrast",
        grafton_visca::command::image::ContrastInquiry
    ),
    ("image.gamma", grafton_visca::command::image::GammaInquiry),
    (
        "image.sharpness_mode",
        grafton_visca::command::image::SharpnessModeInquiry
    ),
    (
        "image.sharpness",
        grafton_visca::command::image::SharpnessInquiry
    ),
    (
        "image.noise_reduction_2d",
        grafton_visca::command::image::NoiseReduction2dInquiry
    ),
    (
        "image.noise_reduction_2d_mode",
        grafton_visca::command::image::NoiseReduction2dModeInquiry
    ),
    (
        "image.noise_reduction_3d",
        grafton_visca::command::image::NoiseReduction3dInquiry
    ),
    ("image.flip", grafton_visca::command::image::FlipInquiry),
    (
        "image.flip_mode",
        grafton_visca::command::image::FlipModeInquiry
    ),
    (
        "image.picture_effect",
        grafton_visca::command::image::PictureEffectInquiry
    ),
    (
        "image.master_black",
        grafton_visca::command::image::MasterBlackInquiry
    ),
    (
        "image.red_black",
        grafton_visca::command::image::RedBlackInquiry
    ),
    (
        "image.blue_black",
        grafton_visca::command::image::BlueBlackInquiry
    ),
    ("image.detail", grafton_visca::command::image::DetailInquiry),
    ("image.knee", grafton_visca::command::image::KneeInquiry),
    (
        "image.knee_mode",
        grafton_visca::command::image::KneeModeInquiry
    ),
    (
        "image.knee_slope",
        grafton_visca::command::image::KneeSlopeInquiry
    ),
    (
        "image.knee_point",
        grafton_visca::command::image::KneePointInquiry
    ),
    (
        "image.knee_permission",
        grafton_visca::command::image::KneePermissionInquiry::new(
            grafton_visca::types::KneeParameter::Setting
        )
    ),
    ("tally.red", grafton_visca::command::tally::RedInquiry),
    ("tally.green", grafton_visca::command::tally::GreenInquiry),
    (
        "nd_filter.density",
        grafton_visca::command::nd_filter::DensityInquiry
    ),
    (
        "nd_filter.preset",
        grafton_visca::command::nd_filter::PresetInquiry
    ),
    (
        "motion_sync.enabled",
        grafton_visca::command::motion_sync::EnabledInquiry
    ),
    ("menu.visible", grafton_visca::command::menu::VisibleInquiry),
    (
        "streaming.usb_audio",
        grafton_visca::command::streaming::UsbAudioInquiry
    ),
    (
        "zoom.normalized",
        grafton_visca::command::zoom::NormalizedInquiry::for_target(
            &grafton_visca::config::TargetRegistration::new(
                CameraId::CAMERA_1,
                grafton_visca::profiles::ProfileSpec::from_compile_time::<
                    grafton_visca::profiles::PtzOpticsG2,
                >()
                .expect("source profile")
            )
            .expect("target registration"),
            grafton_visca::types::ZoomDomain::Optical
        )
        .expect("normalized inquiry")
    ),
    (
        "exposure.backlight_compensation",
        grafton_visca::command::exposure::BacklightCompensationInquiry
    ),
);

fn speed() -> PanSpeed {
    PanSpeed::new(3).expect("valid pan speed")
}

fn tilt() -> TiltSpeed {
    TiltSpeed::new(4).expect("valid tilt speed")
}

fn zoom_speed() -> ZoomSpeed {
    ZoomSpeed::new(3).expect("valid zoom speed")
}

#[test]
fn representative_typed_requests_encode_without_heap_allocation() {
    let drive = grafton_visca::command::pan_tilt::Drive::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            grafton_visca::profiles::ProfileSpec::from_compile_time::<
                grafton_visca::profiles::PtzOpticsG2,
            >()
            .expect("source profile"),
        )
        .expect("target registration"),
        PanTiltDirection::Up,
        grafton_visca::types::PanTiltSpeed {
            pan: speed(),
            tilt: tilt(),
        },
    )
    .expect("drive");
    let zoom = grafton_visca::command::zoom::TeleVariable::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            grafton_visca::profiles::ProfileSpec::from_compile_time::<
                grafton_visca::profiles::PtzOpticsG2,
            >()
            .expect("source profile"),
        )
        .expect("target registration"),
        zoom_speed(),
    )
    .expect("profile-aware request");
    let preset = grafton_visca::command::presets::Store::new(PresetNumber::new(1).expect("preset"));
    let reset = grafton_visca::command::presets::Clear::new(PresetNumber::new(1).expect("preset"));
    let pan_stop = grafton_visca::command::pan_tilt::Stop::for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            grafton_visca::profiles::ProfileSpec::from_compile_time::<
                grafton_visca::profiles::PtzOpticsG2,
            >()
            .expect("source profile"),
        )
        .expect("target registration"),
    )
    .expect("profile-safe stop");

    assert_request_encoding_does_not_allocate(
        "pan/tilt home",
        &grafton_visca::command::pan_tilt::Home,
    );
    assert_request_encoding_does_not_allocate(
        "pan/tilt reset",
        &grafton_visca::command::pan_tilt::Reset,
    );
    assert_request_encoding_does_not_allocate("pan/tilt stop", &pan_stop);
    assert_request_encoding_does_not_allocate("pan/tilt drive", &drive);
    assert_request_encoding_does_not_allocate("zoom stop", &grafton_visca::command::zoom::Stop);
    assert_request_encoding_does_not_allocate("zoom drive", &zoom);
    assert_request_encoding_does_not_allocate(
        "focus infinity",
        &grafton_visca::command::focus::Infinity,
    );
    assert_request_encoding_does_not_allocate("focus stop", &grafton_visca::command::focus::Stop);
    assert_request_encoding_does_not_allocate("preset set", &preset);
    assert_request_encoding_does_not_allocate("preset reset", &reset);
    assert_request_encoding_does_not_allocate(
        "shared on/off encoder",
        &grafton_visca::command::image::SetFlip::new(true),
    );
    assert_request_encoding_does_not_allocate(
        "FR7 shared on/off encoder",
        &grafton_visca::command::exposure::SetAutoGain::new(false),
    );
    assert_request_encoding_does_not_allocate(
        "value-enum operand",
        &grafton_visca::command::streaming::SetNdiQuality::new(
            grafton_visca::types::NdiQuality::High,
        ),
    );
    let duration = grafton_visca::command::presets::SetSeparateDuration::new(
        PresetNumber::MIN,
        std::time::Duration::from_millis(1200),
    )
    .expect("checked preset duration");
    assert_request_encoding_does_not_allocate("checked preset duration", &duration);
    assert_request_encoding_does_not_allocate(
        "power inquiry",
        &grafton_visca::command::power::StatusInquiry,
    );
}

fn assert_warmed_framing_reuses_buffer<E: Envelope>(label: &str, envelope: E) {
    const WIRE: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
    let mut output = BytesMut::with_capacity(32);
    envelope
        .frame_into(WIRE, CommandKind::Command, &mut output)
        .expect("the warmed allocation test frame is valid");
    let pointer = output.as_ptr();
    let capacity = output.capacity();

    let allocations = allocations_during(|| {
        for _ in 0..128 {
            let metadata = envelope
                .frame_into(WIRE, CommandKind::Command, &mut output)
                .expect("the warmed allocation test frame is valid");
            std::hint::black_box(metadata);
        }
    });
    assert_eq!(allocations, 0, "{label} warmed framing allocated");
    assert_eq!(
        output.as_ptr(),
        pointer,
        "{label} replaced its framing buffer"
    );
    assert_eq!(
        output.capacity(),
        capacity,
        "{label} grew its framing buffer"
    );
}

/// Issue #571: the owner counters are a hot-path observation, so the published
/// snapshot must stay a scalar copy. The `Copy` bound is the real guard — a
/// counter that grew owned state (a per-code map, a label string) would fail to
/// compile here rather than quietly allocate on every metrics read.
#[test]
fn observing_the_metrics_snapshot_does_not_allocate() {
    const fn assert_copy<T: Copy>() {}
    assert_copy::<MetricsSnapshot>();

    let snapshot = MetricsSnapshot::default();
    let allocations = allocations_during(|| {
        for _ in 0..128 {
            let observed = snapshot;
            std::hint::black_box((
                observed.ack_timeouts,
                observed.completion_timeouts,
                observed.inquiry_timeouts,
                observed.busy_errors,
                observed.protocol_errors,
                observed.retries_scheduled,
                observed.received_frames,
                observed.ignored_unmatched_sequenced_replies,
            ));
        }
    });
    assert_eq!(allocations, 0, "metrics snapshot observation allocated");
}

#[test]
fn raw_and_sony_framing_reuse_one_warmed_buffer() {
    assert_warmed_framing_reuses_buffer("raw VISCA", RawVisca::new(AddressingMode::Ip));
    assert_warmed_framing_reuses_buffer(
        "Sony encapsulation",
        SonyEncapsulated::new(AddressingMode::Ip),
    );
}

/// Preparation is measured before admission, where the engine starts sharing
/// the inert frame with its transmit effects.
#[cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]
#[test]
fn built_in_command_and_inquiry_preparation_stay_inline() {
    use grafton_visca::{
        config::OperationalTuning,
        profiles::{ProfileSpec, PtzOpticsG2},
        testing::preparation,
    };
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    let target = grafton_visca::config::TargetRegistration::new(CameraId::CAMERA_1, profile)
        .expect("target registration");
    let tuning = OperationalTuning::new();
    let raw = grafton_visca::raw::PlainRequest::new(
        [0x81, 1, 4, 0, 2, 0xff],
        grafton_visca::profiles::CommandTimeoutClass::Quick,
        grafton_visca::request::RetryClass::Never,
        grafton_visca::request::SubmissionClass::Normal,
    )
    .expect("inline raw request");
    let allocations = allocations_during(|| {
        preparation::check_command(&raw, &target, tuning)
            .expect("an owned inline raw frame stays inline in preparation");
        preparation::check_command(&grafton_visca::command::power::On, &target, tuning)
            .expect("power command prepares");
        preparation::check_inquiry(
            &grafton_visca::command::power::StatusInquiry,
            &target,
            tuning,
        )
        .expect("power inquiry prepares");
    });
    assert_eq!(allocations, 0, "pure built-in preparation allocated");
}

#[cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]
#[test]
fn profiled_position_inquiry_preparation_and_decoder_clones_stay_inline() {
    use grafton_visca::{
        config::OperationalTuning,
        profiles::{ProfileSpec, PtzOpticsG2, SonyBrc300, SonyFr7},
        request::Inquiry,
        testing::preparation,
    };
    let profiles = [
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
        ProfileSpec::from_compile_time::<SonyBrc300>().expect("BRC-300"),
        ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7"),
    ];
    let targets = profiles.map(|profile| {
        grafton_visca::config::TargetRegistration::new(CameraId::CAMERA_1, profile)
            .expect("target registration")
    });
    for target in &targets {
        let allocations = allocations_during(|| {
            for _ in 0..100 {
                preparation::check_inquiry(
                    &grafton_visca::command::pan_tilt::PositionInquiry,
                    target,
                    OperationalTuning::new(),
                )
                .expect("position inquiry prepares");
                let decoder = grafton_visca::command::pan_tilt::PositionInquiry
                    .decoder_for_profile(target.profile());
                std::hint::black_box(decoder.clone());
            }
        });
        assert_eq!(allocations, 0, "profiled position preparation allocated");
    }
}

#[cfg(any(
    feature = "runtime-tokio",
    feature = "runtime-smol",
    all(feature = "async", feature = "test-utils")
))]
// Counts allocations on the owner thread. External reactor bookkeeping is
// outside this boundary; the actor regression separately pins timer factories.
fn assert_timer_reset_does_not_allocate(runtime: &impl grafton_visca::runtime::Executor) {
    use grafton_visca::runtime::ResettableTimer;
    use std::{future::Future, task::Context, time::Duration};
    let initial = runtime.now();
    let mut timer = Box::pin(runtime.timer(initial + Duration::from_secs(60)));
    let mut context = Context::from_waker(std::task::Waker::noop());
    assert!(timer.as_mut().poll(&mut context).is_pending());
    let allocations = allocations_during(|| {
        for seconds in 61..=160 {
            timer.as_mut().reset(initial + Duration::from_secs(seconds));
            assert!(timer.as_mut().poll(&mut context).is_pending());
        }
    });
    assert_eq!(allocations, 0, "rearming a warmed owner timer allocated");
}

#[cfg(feature = "runtime-tokio")]
#[test]
fn warmed_tokio_timer_resets_do_not_allocate() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let executor = grafton_visca::runtime::TokioExecutor::from_handle(runtime.handle().clone());
    assert_timer_reset_does_not_allocate(&executor);
}

#[cfg(feature = "runtime-smol")]
#[test]
fn warmed_smol_timer_resets_do_not_allocate() {
    assert_timer_reset_does_not_allocate(&grafton_visca::runtime::SmolExecutor::new());
}

#[cfg(all(feature = "async", feature = "test-utils"))]
#[test]
fn warmed_manual_clock_timer_resets_do_not_allocate() {
    use grafton_visca::runtime::{Executor, ResettableTimer};
    use grafton_visca::testing::testkit::{ManualClock, ManualClockExecutor};
    use std::{future::Future, task::Context, time::Duration};
    let clock = ManualClock::new();
    // A noop waker cannot participate in quiescence. Keep this thread runnable
    // so the baseline measures pending resets, not automatic clock advances.
    let _participant = clock.enter();
    let executor = ManualClockExecutor::local(&clock);
    assert_timer_reset_does_not_allocate(&executor);

    let mut timer = Box::pin(executor.timer(clock.now() + Duration::from_secs(1)));
    let mut context = Context::from_waker(std::task::Waker::noop());
    assert!(timer.as_mut().poll(&mut context).is_pending());
    clock.advance(Duration::from_secs(1));
    assert!(timer.as_mut().poll(&mut context).is_ready());
    let next = clock.now() + Duration::from_secs(1);
    let allocations = allocations_during(|| {
        timer.as_mut().reset(next);
        assert!(timer.as_mut().poll(&mut context).is_pending());
    });
    assert_eq!(allocations, 0, "rearming a completed owned timer allocated");
    let now = clock.now();
    let allocations = allocations_during(|| {
        timer.as_mut().reset(now);
        assert!(timer.as_mut().poll(&mut context).is_ready());
        timer.as_mut().reset(now + Duration::from_secs(1));
        assert!(timer.as_mut().poll(&mut context).is_pending());
        timer.as_mut().reset(now + Duration::from_secs(1));
        assert!(timer.as_mut().poll(&mut context).is_pending());
        timer.as_mut().reset(now);
        assert!(timer.as_mut().poll(&mut context).is_ready());
    });
    assert_eq!(
        allocations, 0,
        "owned timer reset/wake bookkeeping allocated"
    );
    let deadline = clock.now() + Duration::from_secs(1);
    let mut batch: [_; 4] = std::array::from_fn(|_| Box::pin(executor.timer(deadline)));
    for timer in &mut batch {
        assert!(timer.as_mut().poll(&mut context).is_pending());
    }
    let allocations = allocations_during(|| {
        clock.advance(Duration::from_secs(1));
        for timer in &mut batch {
            assert!(timer.as_mut().poll(&mut context).is_ready());
        }
    });
    assert_eq!(allocations, 0, "the four-timer wake batch allocated");
}

#[cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]
#[test]
fn warmed_receive_decoding_and_batch_recycling_do_not_allocate() {
    use grafton_visca::{
        profiles::{GenericVisca, ProfileSpec, SonyFr7},
        testing::receive::ReceiveProbe,
        transport::{SendSemantics, TransportConfig},
    };
    const RAW: &[u8] = &[0x90, 0x41, 0xff, 0x90, 0x51, 0xff];
    const SONY: &[u8] = &[
        0x01, 0x11, 0x00, 0x03, 0, 0, 0, 1, 0x90, 0x41, 0xff, 0x01, 0x11, 0x00, 0x03, 0, 0, 0, 2,
        0x90, 0x51, 0xff,
    ];
    for (profile, chunk) in [
        (
            ProfileSpec::from_compile_time::<GenericVisca>().unwrap(),
            RAW,
        ),
        (ProfileSpec::from_compile_time::<SonyFr7>().unwrap(), SONY),
    ] {
        for semantics in [SendSemantics::Stream, SendSemantics::Datagram] {
            let mut probe =
                ReceiveProbe::new(&profile, TransportConfig::default(), semantics).unwrap();
            assert_eq!(probe.receive(chunk).unwrap(), 2);
            let mut observed = 0;
            let allocations = allocations_during(|| {
                for _ in 0..128 {
                    observed += probe.receive(std::hint::black_box(chunk)).unwrap();
                }
            });
            assert_eq!(observed, 256);
            assert_eq!(allocations, 0, "warmed receive allocated: {semantics:?}");
        }
    }
}

#[cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]
#[test]
fn warmed_malformed_receive_decoding_does_not_allocate() {
    use grafton_visca::{
        profiles::{GenericVisca, ProfileSpec, SonyFr7},
        testing::receive::ReceiveProbe,
        transport::{SendSemantics, TransportConfig},
    };
    const RAW_ECHO_AND_ACK: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff, 0x90, 0x41, 0xff];
    const RAW_MALFORMED_AND_ACK: &[u8] = &[0x90, 0x61, 0xff, 0x90, 0x41, 0xff];
    const SONY_ZERO_PAYLOAD: &[u8] = &[0x01, 0x11, 0, 0, 0, 0, 0, 1];
    // A complete Sony command-shaped packet cannot classify as a response.
    const SONY_WRONG_TYPE: &[u8] = &[0x01, 0x00, 0, 3, 0, 0, 0, 1, 0x90, 0x41, 0xff];
    // Datagram boundaries prove the last declared payload byte is missing.
    const SONY_LENGTH_MISMATCH: &[u8] = &[0x01, 0x11, 0, 4, 0, 0, 0, 1, 0x90, 0x41, 0xff];
    for (profile, chunk, semantics, expected) in [
        (
            ProfileSpec::from_compile_time::<GenericVisca>().unwrap(),
            RAW_ECHO_AND_ACK,
            SendSemantics::Stream,
            1,
        ),
        (
            ProfileSpec::from_compile_time::<GenericVisca>().unwrap(),
            RAW_ECHO_AND_ACK,
            SendSemantics::Datagram,
            0,
        ),
        (
            ProfileSpec::from_compile_time::<SonyFr7>().unwrap(),
            SONY_WRONG_TYPE,
            SendSemantics::Stream,
            0,
        ),
        (
            ProfileSpec::from_compile_time::<SonyFr7>().unwrap(),
            SONY_WRONG_TYPE,
            SendSemantics::Datagram,
            0,
        ),
        (
            ProfileSpec::from_compile_time::<SonyFr7>().unwrap(),
            SONY_LENGTH_MISMATCH,
            SendSemantics::Datagram,
            0,
        ),
        (
            ProfileSpec::from_compile_time::<GenericVisca>().unwrap(),
            RAW_MALFORMED_AND_ACK,
            SendSemantics::Stream,
            1,
        ),
        (
            ProfileSpec::from_compile_time::<GenericVisca>().unwrap(),
            RAW_MALFORMED_AND_ACK,
            SendSemantics::Datagram,
            0,
        ),
        (
            ProfileSpec::from_compile_time::<SonyFr7>().unwrap(),
            SONY_ZERO_PAYLOAD,
            SendSemantics::Stream,
            0,
        ),
        (
            ProfileSpec::from_compile_time::<SonyFr7>().unwrap(),
            SONY_ZERO_PAYLOAD,
            SendSemantics::Datagram,
            0,
        ),
    ] {
        let mut probe = ReceiveProbe::new(&profile, TransportConfig::default(), semantics).unwrap();
        assert_eq!(probe.receive(chunk).unwrap(), expected);
        let mut observed = 0;
        let allocations = allocations_during(|| {
            for _ in 0..128 {
                observed += probe.receive(std::hint::black_box(chunk)).unwrap();
            }
        });
        assert_eq!(observed, 128 * expected);
        assert_eq!(allocations, 0, "malformed input allocated: {semantics:?}");
    }
}

#[cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]
#[test]
fn warmed_owner_command_and_frame_turns_do_not_allocate() {
    use grafton_visca::{
        profiles::{GenericVisca, ProfileSpec, SonyFr7},
        testing::turns::TurnProbe,
        transport::{SendSemantics, TransportConfig},
    };

    for (label, profile) in [
        (
            "raw",
            ProfileSpec::from_compile_time::<GenericVisca>().unwrap(),
        ),
        ("Sony", ProfileSpec::from_compile_time::<SonyFr7>().unwrap()),
    ] {
        for semantics in [SendSemantics::Stream, SendSemantics::Datagram] {
            let mut probe =
                TurnProbe::new(&profile, TransportConfig::default(), semantics).unwrap();
            probe.stage_command().unwrap();
            probe.command_turn().unwrap();
            probe.frame_turn().unwrap();
            let mut command_allocations = 0;
            for _ in 0..8 {
                probe.stage_command().unwrap();
                command_allocations += allocations_during(|| probe.command_turn().unwrap());
            }
            let frame_allocations = allocations_during(|| {
                for _ in 0..128 {
                    probe.frame_turn().unwrap();
                }
            });
            eprintln!("{label} {semantics:?}: command allocations={command_allocations}/8; inert frame allocations={frame_allocations}/128");
            assert_eq!(
                command_allocations, 0,
                "{label} {semantics:?} warmed command turns allocated"
            );
            assert_eq!(
                frame_allocations, 0,
                "{label} {semantics:?} warmed inert frame turns allocated"
            );
        }
    }
}

#[cfg(any(feature = "blocking", all(feature = "async", feature = "test-utils")))]
fn assert_builtin_views_do_not_allocate(select: impl Fn()) {
    select();
    let allocations = allocations_during(|| {
        for _ in 0..1_000 {
            select();
        }
    });
    assert_eq!(allocations, 0, "built-in camera view selection allocated");
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_builtin_camera_and_into_typed_views_do_not_allocate() {
    use grafton_visca::{
        blocking::Session,
        config::SessionConfig,
        profiles::{AnyProfile, ProfileId, PtzOpticsG2, SonyBrc300},
    };
    use grafton_visca_test_support::fake_camera::FakeCamera;

    let g2 = Session::open(
        FakeCamera::new(|_, _| {}).blocking_wire(),
        &SessionConfig::new(ProfileId::PtzOpticsG2.spec().unwrap()),
    )
    .unwrap();
    let brc = Session::open(
        FakeCamera::new(|_, _| {}).blocking_wire(),
        &SessionConfig::new(ProfileId::SonyBrc300.spec().unwrap()),
    )
    .unwrap();
    assert_builtin_views_do_not_allocate(|| {
        drop(std::hint::black_box(
            g2.camera::<PtzOpticsG2>(CameraId::CAMERA_1).unwrap(),
        ));
        drop(std::hint::black_box(
            brc.camera::<AnyProfile>(CameraId::CAMERA_1)
                .unwrap()
                .into_typed::<SonyBrc300>()
                .unwrap(),
        ));
    });
    g2.close().unwrap();
    brc.close().unwrap();
}

#[cfg(all(feature = "async", feature = "test-utils"))]
#[test]
fn async_builtin_camera_and_into_typed_views_do_not_allocate() {
    use grafton_visca::{
        config::SessionConfig,
        profiles::{AnyProfile, ProfileId, PtzOpticsG2, SonyBrc300},
        r#async::Session,
        testing::testkit::{ManualClock, ManualClockExecutor},
    };
    use grafton_visca_test_support::fake_camera::FakeCamera;

    let clock = ManualClock::new();
    let executor = ManualClockExecutor::local(&clock);
    executor.run_until(async {
        let g2 = Session::open(
            FakeCamera::new(|_, _| {}).async_wire(),
            &SessionConfig::new(ProfileId::PtzOpticsG2.spec().unwrap()),
            executor.clone(),
        )
        .await
        .unwrap();
        let brc = Session::open(
            FakeCamera::new(|_, _| {}).async_wire(),
            &SessionConfig::new(ProfileId::SonyBrc300.spec().unwrap()),
            executor.clone(),
        )
        .await
        .unwrap();
        assert_builtin_views_do_not_allocate(|| {
            drop(std::hint::black_box(
                g2.camera::<PtzOpticsG2>(CameraId::CAMERA_1).unwrap(),
            ));
            drop(std::hint::black_box(
                brc.camera::<AnyProfile>(CameraId::CAMERA_1)
                    .unwrap()
                    .into_typed::<SonyBrc300>()
                    .unwrap(),
            ));
        });
        g2.close().await.unwrap();
        brc.close().await.unwrap();
    });
}

#[cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]
#[test]
fn warmed_raw_and_sony_retries_reuse_owner_buffers() {
    use grafton_visca::{
        profiles::{GenericVisca, ProfileSpec, SonyFr7},
        testing::turns::TurnProbe,
        transport::{SendSemantics, TransportConfig},
    };

    for profile in [
        ProfileSpec::from_compile_time::<GenericVisca>().unwrap(),
        ProfileSpec::from_compile_time::<SonyFr7>().unwrap(),
    ] {
        for semantics in [SendSemantics::Stream, SendSemantics::Datagram] {
            let mut probe =
                TurnProbe::new(&profile, TransportConfig::default(), semantics).unwrap();
            probe.stage_retry().unwrap();
            probe.retry_turn().unwrap();
            for _ in 0..8 {
                probe.stage_retry().unwrap();
                let allocations = allocations_during(|| probe.retry_turn().unwrap());
                assert_eq!(allocations, 0, "safe retry allocated: {semantics:?}");
            }
        }
    }
}
