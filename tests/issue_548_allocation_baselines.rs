//! Allocation characterization at the stable 2.0 request and framing boundaries.

#![allow(unsafe_code, clippy::expect_used, clippy::unwrap_used)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::Mutex,
};

use bytes::BytesMut;
use grafton_visca::{
    command::{CommandKind, PowerInquiry},
    request::builtin::{
        FocusInfinity, FocusStop, PanTiltDrive, PanTiltHome, PanTiltReset, PanTiltStop,
        PresetReset, PresetSet, ZoomDrive, ZoomStop,
    },
    transport::{AddressingMode, Envelope, RawVisca, SonyEncapsulated},
    types::{PanSpeed, TiltSpeed, ZoomSpeed},
    CameraId, MetricsSnapshot, PanTiltDirection, PresetNumber, Request,
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
            assert_eq!(QUERYABLE_GENERATED_INQUIRY_COUNT, 74);
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
    ("PowerInquiry", grafton_visca::command::PowerInquiry),
    ("VersionInquiry", grafton_visca::command::VersionInquiry),
    (
        "PanTiltPositionInquiry",
        grafton_visca::command::PanTiltPositionInquiry
    ),
    (
        "ZoomPositionInquiry",
        grafton_visca::command::ZoomPositionInquiry
    ),
    (
        "FocusPositionInquiry",
        grafton_visca::command::FocusPositionInquiry
    ),
    (
        "ExposureModeInquiry",
        grafton_visca::command::ExposureModeInquiry
    ),
    (
        "ExposureCompensationInquiry",
        grafton_visca::command::ExposureCompensationInquiry
    ),
    (
        "ExposureCompensationModeInquiry",
        grafton_visca::command::ExposureCompensationModeInquiry
    ),
    ("IrisInquiry", grafton_visca::command::IrisInquiry),
    ("ShutterInquiry", grafton_visca::command::ShutterInquiry),
    (
        "BrightnessInquiry",
        grafton_visca::command::BrightnessInquiry
    ),
    (
        "WhiteBalanceModeInquiry",
        grafton_visca::command::WhiteBalanceModeInquiry
    ),
    (
        "ColorTemperatureInquiry",
        grafton_visca::command::ColorTemperatureInquiry
    ),
    ("RedGainInquiry", grafton_visca::command::RedGainInquiry),
    ("BlueGainInquiry", grafton_visca::command::BlueGainInquiry),
    (
        "SharpnessModeInquiry",
        grafton_visca::command::SharpnessModeInquiry
    ),
    (
        "SaturationInquiry",
        grafton_visca::command::SaturationInquiry
    ),
    ("HueInquiry", grafton_visca::command::HueInquiry),
    ("GainInquiry", grafton_visca::command::GainInquiry),
    ("GainLimitInquiry", grafton_visca::command::GainLimitInquiry),
    ("BacklightInquiry", grafton_visca::command::BacklightInquiry),
    ("ImageFlipInquiry", grafton_visca::command::ImageFlipInquiry),
    (
        "NoiseReduction2DModeInquiry",
        grafton_visca::command::NoiseReduction2DModeInquiry
    ),
    (
        "NoiseReduction2DInquiry",
        grafton_visca::command::NoiseReduction2DInquiry
    ),
    (
        "NoiseReduction3DInquiry",
        grafton_visca::command::NoiseReduction3DInquiry
    ),
    ("FocusZoneInquiry", grafton_visca::command::FocusZoneInquiry),
    (
        "AutoFocusSensitivityInquiry",
        grafton_visca::command::AutoFocusSensitivityInquiry
    ),
    (
        "FocusNearLimitInquiry",
        grafton_visca::command::FocusNearLimitInquiry
    ),
    ("FocusModeInquiry", grafton_visca::command::FocusModeInquiry),
    (
        "MenuOpenCloseInquiry",
        grafton_visca::command::MenuOpenCloseInquiry
    ),
    ("NdFilterInquiry", grafton_visca::command::NdFilterInquiry),
    (
        "PictureEffectInquiry",
        grafton_visca::command::PictureEffectInquiry
    ),
    ("FlipStateInquiry", grafton_visca::command::FlipStateInquiry),
    (
        "AutoWhiteBalanceSensitivityInquiry",
        grafton_visca::command::AutoWhiteBalanceSensitivityInquiry
    ),
    (
        "ExposureCompensationPositionInquiry",
        grafton_visca::command::ExposureCompensationPositionInquiry
    ),
    ("RedTuningInquiry", grafton_visca::command::RedTuningInquiry),
    (
        "BlueTuningInquiry",
        grafton_visca::command::BlueTuningInquiry
    ),
    ("GammaInquiry", grafton_visca::command::GammaInquiry),
    (
        "SharpnessPositionInquiry",
        grafton_visca::command::SharpnessPositionInquiry
    ),
    (
        "MotionSyncModeInquiry",
        grafton_visca::command::MotionSyncModeInquiry
    ),
    (
        "MotionSyncPresetInquiry",
        grafton_visca::command::MotionSyncPresetInquiry
    ),
    ("UsbAudioInquiry", grafton_visca::command::UsbAudioInquiry),
    (
        "NdFilterPresetInquiry",
        grafton_visca::command::NdFilterPresetInquiry
    ),
    ("TallyRedInquiry", grafton_visca::command::TallyRedInquiry),
    (
        "TallyGreenInquiry",
        grafton_visca::command::TallyGreenInquiry
    ),
    (
        "FlickerModeInquiry",
        grafton_visca::command::FlickerModeInquiry
    ),
    ("ContrastInquiry", grafton_visca::command::ContrastInquiry),
    ("LuminanceInquiry", grafton_visca::command::LuminanceInquiry),
    (
        "Fr7AeLevelInquiry",
        grafton_visca::command::Fr7AeLevelInquiry
    ),
    (
        "Fr7AutoGainInquiry",
        grafton_visca::command::Fr7AutoGainInquiry
    ),
    (
        "Fr7AutoShutterInquiry",
        grafton_visca::command::Fr7AutoShutterInquiry
    ),
    (
        "Fr7AutoIrisInquiry",
        grafton_visca::command::Fr7AutoIrisInquiry
    ),
    (
        "Fr7PresetWhiteInquiry",
        grafton_visca::command::Fr7PresetWhiteInquiry
    ),
    (
        "Fr7MemoryColorTemperatureInquiry",
        grafton_visca::command::Fr7MemoryColorTemperatureInquiry
    ),
    (
        "Fr7MemoryRedGainInquiry",
        grafton_visca::command::Fr7MemoryRedGainInquiry
    ),
    (
        "Fr7MemoryBlueGainInquiry",
        grafton_visca::command::Fr7MemoryBlueGainInquiry
    ),
    (
        "Fr7MemoryTintInquiry",
        grafton_visca::command::Fr7MemoryTintInquiry
    ),
    (
        "Fr7AtwColorTemperatureOffsetInquiry",
        grafton_visca::command::Fr7AtwColorTemperatureOffsetInquiry
    ),
    (
        "Fr7AtwTintOffsetInquiry",
        grafton_visca::command::Fr7AtwTintOffsetInquiry
    ),
    (
        "Fr7MasterBlackInquiry",
        grafton_visca::command::Fr7MasterBlackInquiry
    ),
    (
        "Fr7RedBlackInquiry",
        grafton_visca::command::Fr7RedBlackInquiry
    ),
    (
        "Fr7BlueBlackInquiry",
        grafton_visca::command::Fr7BlueBlackInquiry
    ),
    ("Fr7DetailInquiry", grafton_visca::command::Fr7DetailInquiry),
    ("Fr7KneeInquiry", grafton_visca::command::Fr7KneeInquiry),
    (
        "Fr7KneeModeInquiry",
        grafton_visca::command::Fr7KneeModeInquiry
    ),
    (
        "Fr7KneeSlopeInquiry",
        grafton_visca::command::Fr7KneeSlopeInquiry
    ),
    (
        "Fr7KneePointInquiry",
        grafton_visca::command::Fr7KneePointInquiry
    ),
    (
        "Fr7KneePermissionInquiry",
        grafton_visca::command::Fr7KneePermissionInquiry::new(
            grafton_visca::command::Fr7KneeParameter::Setting
        )
    ),
    (
        "Fr7ClearImageZoomPositionInquiry",
        grafton_visca::command::Fr7ClearImageZoomPositionInquiry
    ),
    (
        "Fr7PresetSpeedSelectionInquiry",
        grafton_visca::command::Fr7PresetSpeedSelectionInquiry
    ),
    (
        "Fr7CommonPresetSpeedInquiry",
        grafton_visca::command::Fr7CommonPresetSpeedInquiry
    ),
    (
        "Fr7SeparatePresetModeInquiry",
        grafton_visca::command::Fr7SeparatePresetModeInquiry::new(
            grafton_visca::types::Fr7PresetNumber::MIN
        )
    ),
    (
        "Fr7SeparatePresetSpeedInquiry",
        grafton_visca::command::Fr7SeparatePresetSpeedInquiry::new(
            grafton_visca::types::Fr7PresetNumber::MIN
        )
    ),
    (
        "Fr7SeparatePresetDurationInquiry",
        grafton_visca::command::Fr7SeparatePresetDurationInquiry::new(
            grafton_visca::types::Fr7PresetNumber::MIN
        )
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
    let drive = PanTiltDrive::new(PanTiltDirection::Up, speed(), tilt()).expect("drive");
    let zoom = ZoomDrive::TeleVariable(zoom_speed());
    let preset = PresetSet::new(PresetNumber::new(1).expect("preset"));
    let reset = PresetReset::new(PresetNumber::new(1).expect("preset"));
    let pan_stop = PanTiltStop::new(speed(), tilt());

    assert_request_encoding_does_not_allocate("pan/tilt home", &PanTiltHome);
    assert_request_encoding_does_not_allocate("pan/tilt reset", &PanTiltReset);
    assert_request_encoding_does_not_allocate("pan/tilt stop", &pan_stop);
    assert_request_encoding_does_not_allocate("pan/tilt drive", &drive);
    assert_request_encoding_does_not_allocate("zoom stop", &ZoomStop);
    assert_request_encoding_does_not_allocate("zoom drive", &zoom);
    assert_request_encoding_does_not_allocate("focus infinity", &FocusInfinity);
    assert_request_encoding_does_not_allocate("focus stop", &FocusStop);
    assert_request_encoding_does_not_allocate("preset set", &preset);
    assert_request_encoding_does_not_allocate("preset reset", &reset);
    assert_request_encoding_does_not_allocate("power inquiry", &PowerInquiry);
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
        profiles::PtzOpticsG2, testing::preparation, OperationalTuning, ProfileSpec,
    };
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    let target = grafton_visca::TargetRegistration::new(CameraId::CAMERA_1, profile)
        .expect("target registration");
    let tuning = OperationalTuning::new();
    let raw = grafton_visca::raw::Plain::new(
        [0x81, 1, 4, 0, 2, 0xff],
        grafton_visca::TimeoutClass::Quick,
        grafton_visca::RetryClass::Never,
        grafton_visca::ControlClass::Normal,
    )
    .expect("inline raw request");
    let allocations = allocations_during(|| {
        preparation::check_command(&raw, &target, tuning)
            .expect("an owned inline raw frame stays inline in preparation");
        preparation::check_command(&grafton_visca::command::PowerOn::new(), &target, tuning)
            .expect("power command prepares");
        preparation::check_inquiry(&PowerInquiry, &target, tuning).expect("power inquiry prepares");
    });
    assert_eq!(allocations, 0, "pure built-in preparation allocated");
}

#[cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]
#[test]
fn profiled_position_inquiry_preparation_and_decoder_clones_stay_inline() {
    use grafton_visca::{
        command::PanTiltPositionInquiry,
        profiles::{PtzOpticsG2, SonyBRC300, SonyFR7},
        testing::preparation,
        Inquiry, OperationalTuning, ProfileSpec,
    };
    let profiles = [
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
        ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC-300"),
        ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7"),
    ];
    let targets = profiles.map(|profile| {
        grafton_visca::TargetRegistration::new(CameraId::CAMERA_1, profile)
            .expect("target registration")
    });
    for target in &targets {
        let allocations = allocations_during(|| {
            for _ in 0..100 {
                preparation::check_inquiry(
                    &PanTiltPositionInquiry,
                    target,
                    OperationalTuning::new(),
                )
                .expect("position inquiry prepares");
                let decoder = PanTiltPositionInquiry.decoder_for_profile(target.profile());
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
fn assert_timer_reset_does_not_allocate(runtime: &impl grafton_visca::Executor) {
    use grafton_visca::ResettableTimer;
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
    let executor = grafton_visca::TokioExecutor::from_handle(runtime.handle().clone());
    assert_timer_reset_does_not_allocate(&executor);
}

#[cfg(feature = "runtime-smol")]
#[test]
fn warmed_smol_timer_resets_do_not_allocate() {
    assert_timer_reset_does_not_allocate(&grafton_visca::SmolExecutor::new());
}

#[cfg(all(feature = "async", feature = "test-utils"))]
#[test]
fn warmed_manual_clock_timer_resets_do_not_allocate() {
    use grafton_visca::testing::testkit::{ManualClock, ManualClockExecutor};
    use grafton_visca::{Executor, ResettableTimer};
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
}

#[cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]
#[test]
fn warmed_receive_decoding_and_batch_recycling_do_not_allocate() {
    use grafton_visca::{
        profiles::{GenericVisca, SonyFR7},
        testing::receive::ReceiveProbe,
        transport::{SendSemantics, TransportConfig},
        ProfileSpec,
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
        (ProfileSpec::from_compile_time::<SonyFR7>().unwrap(), SONY),
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
