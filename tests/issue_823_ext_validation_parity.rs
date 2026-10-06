//! Issue #823: the public `*Ext` validation helpers and the request path
//! (`Request::validate_for_profile` over `ProfileSpec::from_compile_time`)
//! must accept and reject exactly the same values.
//!
//! Each parameter is probed at `min - 1`, `min`, `max` and `max + 1` (where
//! representable in `u8`). A value that its newtype refuses to construct is a
//! rejection on the request side, exactly as a caller would experience it.

#![allow(clippy::expect_used)]

use grafton_visca::{
    capabilities::{
        focus::FocusExt, image_processing::ImageProcessingExt, pan_tilt::PanTiltExt,
        presets::PresetsExt, zoom::ZoomExt, CapabilityRange, Focus, ImageProcessing, PanTilt,
        Presets, ValidationError, Zoom,
    },
    command::{Contrast, HueCommand},
    profiles::{PtzOpticsG2, SonyBRC300, SonyEVIH100, SonyFR7},
    request::builtin::{FocusDrive, PanTiltDrive, PresetSet, ZoomDrive},
    types::{ContrastLevel, FocusSpeed, HueLevel, PanSpeed, TiltSpeed, ZoomSpeed},
    CompileTimeProfile, PanTiltDirection, PresetNumber, ProfileSpec, Request,
};

/// Values worth probing for a closed `u8` range: both edges and one step
/// outside each, when that step exists.
fn boundary_values(min: u8, max: u8) -> Vec<u8> {
    let mut values = Vec::new();
    if let Some(below) = min.checked_sub(1) {
        values.push(below);
    }
    values.push(min);
    values.push(max);
    if let Some(above) = max.checked_add(1) {
        values.push(above);
    }
    values
}

/// Asserts the helper and the request path agree on every probe and that both
/// follow `expected` (the closed range the profile publishes, or `None`).
fn assert_parity(
    label: &str,
    expected: Option<CapabilityRange<u8>>,
    probes: &[u8],
    ext: impl Fn(u8) -> Result<u8, ValidationError>,
    request: impl Fn(u8) -> bool,
) {
    for &value in probes {
        let ext_result = ext(value);
        let ext_accepts = ext_result.is_ok();
        let request_accepts = request(value);
        assert_eq!(
            ext_accepts, request_accepts,
            "{label} {value}: Ext returned {ext_result:?}, request path accepted={request_accepts}"
        );
        let in_range = expected.is_some_and(|range| range.contains(value));
        assert_eq!(
            ext_accepts, in_range,
            "{label} {value}: both paths disagree with the published range {expected:?}"
        );
        match (expected, &ext_result) {
            (None, Err(ValidationError::NotSupported(_))) => {}
            (None, other) => panic!("{label} {value}: expected NotSupported, got {other:?}"),
            (Some(_), Ok(returned)) => assert_eq!(*returned, value, "{label} must not clamp"),
            (Some(_), Err(ValidationError::NotSupported(_))) => {
                panic!("{label} {value}: a published range must not report NotSupported")
            }
            (Some(_), Err(_)) => {}
        }
    }
}

fn spec<P: CompileTimeProfile>() -> ProfileSpec {
    ProfileSpec::from_compile_time::<P>().expect("built-in profile lowers")
}

fn probes_for(range: Option<CapabilityRange<u8>>) -> Vec<u8> {
    match range {
        Some(range) => boundary_values(range.min(), range.max()),
        // No published range: any value is unsupported; probe a spread.
        None => vec![0, 1, 14, 15, 255],
    }
}

fn check_profile<P: CompileTimeProfile>(camera: &P) {
    let spec = spec::<P>();
    let accepts =
        |result: Option<Result<(), grafton_visca::Error>>| result.is_some_and(|r| r.is_ok());

    // Pan speed: request path checks `Capabilities::pan_speed` = 1..=MAX_PAN_SPEED.
    let pan_range = CapabilityRange::<u8>::new(1, <P as PanTilt>::MAX_PAN_SPEED);
    assert_parity(
        "pan speed",
        Some(pan_range),
        &boundary_values(pan_range.min(), pan_range.max()),
        |v| camera.validate_pan_speed(v),
        |v| {
            accepts(PanSpeed::new(v).ok().and_then(|pan| {
                let tilt = TiltSpeed::new(1).ok()?;
                let drive = PanTiltDrive::new(PanTiltDirection::Up, pan, tilt).ok()?;
                Some(drive.validate_for_profile(&spec))
            }))
        },
    );

    // Tilt speed: 1..=MAX_TILT_SPEED.
    let tilt_range = CapabilityRange::<u8>::new(1, <P as PanTilt>::MAX_TILT_SPEED);
    assert_parity(
        "tilt speed",
        Some(tilt_range),
        &boundary_values(tilt_range.min(), tilt_range.max()),
        |v| camera.validate_tilt_speed(v),
        |v| {
            accepts(TiltSpeed::new(v).ok().and_then(|tilt| {
                let pan = PanSpeed::new(1).ok()?;
                let drive = PanTiltDrive::new(PanTiltDirection::Up, pan, tilt).ok()?;
                Some(drive.validate_for_profile(&spec))
            }))
        },
    );

    // Zoom speed: ZOOM_SPEED_RANGE.
    let zoom_range = <P as Zoom>::ZOOM_SPEED_RANGE;
    assert_parity(
        "zoom speed",
        Some(zoom_range),
        &boundary_values(zoom_range.min(), zoom_range.max()),
        |v| camera.validate_zoom_speed(v),
        |v| {
            accepts(
                ZoomSpeed::new(v)
                    .ok()
                    .map(|speed| ZoomDrive::TeleVariable(speed).validate_for_profile(&spec)),
            )
        },
    );

    // Focus speed: 0..=MAX_FOCUS_SPEED.
    let focus_range = CapabilityRange::<u8>::new(0, <P as Focus>::MAX_FOCUS_SPEED);
    assert_parity(
        "focus speed",
        Some(focus_range),
        &boundary_values(focus_range.min(), focus_range.max()),
        |v| camera.validate_focus_speed(v),
        |v| {
            accepts(
                FocusSpeed::new(v)
                    .ok()
                    .map(|speed| FocusDrive::FarVariable(speed).validate_for_profile(&spec)),
            )
        },
    );

    // Preset number: 0..=HIGHEST_PRESET.
    let preset_range = CapabilityRange::<u8>::new(0, <P as Presets>::HIGHEST_PRESET);
    assert_parity(
        "preset number",
        Some(preset_range),
        &boundary_values(preset_range.min(), preset_range.max()),
        |v| camera.validate_preset_number(v),
        |v| {
            accepts(
                PresetNumber::new(v)
                    .ok()
                    .map(|preset| PresetSet::new(preset).validate_for_profile(&spec)),
            )
        },
    );

    // Optional image ranges: hue and contrast.
    let hue = <P as ImageProcessing>::HUE_RANGE;
    assert_parity(
        "hue",
        hue,
        &probes_for(hue),
        |v| camera.validate_hue(v),
        |v| {
            accepts(
                HueLevel::new(v)
                    .ok()
                    .map(|level| HueCommand::new(level).validate_for_profile(&spec)),
            )
        },
    );
    let contrast = <P as ImageProcessing>::CONTRAST_RANGE;
    assert_parity(
        "contrast",
        contrast,
        &probes_for(contrast),
        |v| camera.validate_contrast(v),
        |v| {
            accepts(
                ContrastLevel::new(v)
                    .ok()
                    .map(|level| Contrast::new(level).validate_for_profile(&spec)),
            )
        },
    );
}

#[test]
fn ptzoptics_g2_ext_helpers_match_request_validation() {
    check_profile(&PtzOpticsG2);
}

#[test]
fn sony_fr7_ext_helpers_match_request_validation() {
    check_profile(&SonyFR7);
}

/// The speed helpers reject out-of-range values; they never clamp.
#[test]
fn speed_helpers_reject_instead_of_clamping() {
    let camera = PtzOpticsG2;
    assert!(camera.validate_pan_speed(0).is_err());
    assert!(camera
        .validate_pan_speed(<PtzOpticsG2 as PanTilt>::MAX_PAN_SPEED + 1)
        .is_err());
    assert!(camera.validate_tilt_speed(0).is_err());
    assert!(camera
        .validate_tilt_speed(<PtzOpticsG2 as PanTilt>::MAX_TILT_SPEED + 1)
        .is_err());
    assert!(camera
        .validate_zoom_speed(<PtzOpticsG2 as Zoom>::ZOOM_SPEED_RANGE.max() + 1)
        .is_err());
    assert!(camera
        .validate_focus_speed(<PtzOpticsG2 as Focus>::MAX_FOCUS_SPEED + 1)
        .is_err());
}

/// The BRC-300 publishes neither hue nor contrast, so this exercises the
/// `NotSupported` arm of the shared check against the request path, and the
/// guard below keeps that profile a genuine `None` case.
#[test]
fn sony_brc300_unpublished_image_ranges_match_request_validation() {
    assert!(<SonyBRC300 as ImageProcessing>::HUE_RANGE.is_none());
    assert!(<SonyBRC300 as ImageProcessing>::CONTRAST_RANGE.is_none());
    check_profile(&SonyBRC300);
}

/// The EVI-H100 publishes R8's color hue and aperture ranges; the helpers and
/// request validation agree on them too.
#[test]
fn sony_evi_h100_image_ranges_match_request_validation() {
    assert!(<SonyEVIH100 as ImageProcessing>::HUE_RANGE.is_some());
    check_profile(&SonyEVIH100);
}
