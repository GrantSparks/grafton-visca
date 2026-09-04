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
    ($(($label:literal, $inquiry:ty)),+ $(,)?) => {
        #[test]
        fn all_queryable_generated_inquiries_encode_without_heap_allocation() {
            const QUERYABLE_GENERATED_INQUIRY_COUNT: usize = [$(stringify!($inquiry)),+].len();
            assert_eq!(QUERYABLE_GENERATED_INQUIRY_COUNT, 63);
            $(
                let inquiry = <$inquiry>::default();
                assert_request_encoding_does_not_allocate($label, &inquiry);
            )+
        }
    };
}

// Keep this list synchronized with the queryable section of the built-in
// inquiry table. The two intentionally untyped query requests remain present.
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
    (
        "DynamicRangeInquiry",
        grafton_visca::command::DynamicRangeInquiry
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
    (
        "TallyStatusInquiry",
        grafton_visca::command::TallyStatusInquiry
    ),
    (
        "NightDayModeInquiry",
        grafton_visca::command::NightDayModeInquiry
    ),
    ("NdFilterInquiry", grafton_visca::command::NdFilterInquiry),
    (
        "PictureEffectInquiry",
        grafton_visca::command::PictureEffectInquiry
    ),
    ("FlipStateInquiry", grafton_visca::command::FlipStateInquiry),
    ("StandbyInquiry", grafton_visca::command::StandbyInquiry),
    (
        "FocusRangeInquiry",
        grafton_visca::command::FocusRangeInquiry
    ),
    (
        "IrisControlInquiry",
        grafton_visca::command::IrisControlInquiry
    ),
    ("DefogModeInquiry", grafton_visca::command::DefogModeInquiry),
    (
        "DefogLevelInquiry",
        grafton_visca::command::DefogLevelInquiry
    ),
    (
        "DigitalPtzInquiry",
        grafton_visca::command::DigitalPtzInquiry
    ),
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
    ("AutoTraceInquiry", grafton_visca::command::AutoTraceInquiry),
    (
        "FocusUnlockInquiry",
        grafton_visca::command::FocusUnlockInquiry
    ),
    (
        "SharpnessPositionInquiry",
        grafton_visca::command::SharpnessPositionInquiry
    ),
    (
        "BroadcastDomainInquiry",
        grafton_visca::command::BroadcastDomainInquiry
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
        "TwoToneModeInquiry",
        grafton_visca::command::TwoToneModeInquiry
    ),
    (
        "NdFilterPresetInquiry",
        grafton_visca::command::NdFilterPresetInquiry
    ),
    ("DigitalInquiry", grafton_visca::command::DigitalInquiry),
    (
        "TallyAutoAdjustInquiry",
        grafton_visca::command::TallyAutoAdjustInquiry
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
