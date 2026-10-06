//! Sony BRC-300 (R12) and Nearus BRC-300 (R21) zoom and focus facts.
//!
//! Both sources give the x12 optical zoom table `0000`..`4000`, the digital
//! table `4000`..`7F00` (x1..x4) reached through `CAM_Zoom` Direct `04 47`,
//! `CAM_DZoom` on/off `04 06 02/03`, and the One Push AF trigger `04 18 01`.

#![allow(clippy::expect_used)]

use grafton_visca::{
    capabilities::TypedSupportSurface,
    command::DigitalZoom,
    profiles::{NearusBRC300, SonyBRC300},
    request::builtin::{FocusTrigger, ZoomTarget},
    types::ZoomPosition,
    CameraId, Error, ProfileSpec, Request,
};

fn wire<R: Request + ?Sized>(request: &R) -> Vec<u8> {
    let mut buffer = vec![0_u8; R::MAX_SIZE];
    let written = request
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("request must encode");
    buffer.truncate(written);
    buffer
}

fn profiles() -> [(&'static str, ProfileSpec); 2] {
    [
        (
            "SonyBRC300",
            ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC-300 profile"),
        ),
        (
            "NearusBRC300",
            ProfileSpec::from_compile_time::<NearusBRC300>().expect("Nearus profile"),
        ),
    ]
}

fn target(position: u16) -> ZoomTarget {
    ZoomTarget::new(ZoomPosition::new(position).expect("zoom position"))
}

#[test]
fn zoom_tables_follow_the_sources() {
    for (name, profile) in profiles() {
        let capabilities = profile.capabilities();
        assert_eq!(capabilities.zoom_range_optical, 0x0000..=0x4000, "{name}");
        assert_eq!(
            capabilities.zoom_range_digital,
            Some(0x4000..=0x7F00),
            "{name}"
        );

        // The optical tele end and the digital x4 end are both reachable
        // through `CAM_Zoom` Direct `04 47`; past `7F00` is refused.
        for position in [0x4000, 0x7F00] {
            target(position)
                .validate_for_profile(&profile)
                .unwrap_or_else(|error| panic!("{name} {position:#06x}: {error}"));
        }
        assert_eq!(
            wire(&target(0x7F00)),
            [0x81, 0x01, 0x04, 0x47, 0x07, 0x0F, 0x00, 0x00, 0xFF]
        );
        assert!(
            matches!(
                target(0x7F01).validate_for_profile(&profile),
                Err(Error::ParameterOutOfRange {
                    min: 0x0000,
                    max: 0x7F00,
                    ..
                })
            ),
            "{name}"
        );
    }
}

#[test]
fn digital_zoom_toggle_and_one_push_focus_are_granted() {
    for (name, profile) in profiles() {
        let capabilities = profile.capabilities();
        for surface in [
            TypedSupportSurface::DigitalZoomToggle,
            TypedSupportSurface::DigitalZoomRange,
            TypedSupportSurface::OnePushFocus,
        ] {
            assert!(capabilities.supports_typed(surface), "{name} {surface:?}");
        }

        for (enabled, byte) in [(true, 0x02), (false, 0x03)] {
            let command = DigitalZoom::new(enabled);
            command
                .validate_for_profile(&profile)
                .unwrap_or_else(|error| panic!("{name} digital zoom: {error}"));
            assert_eq!(wire(&command), [0x81, 0x01, 0x04, 0x06, byte, 0xFF]);
        }

        FocusTrigger::OnePush
            .validate_for_profile(&profile)
            .unwrap_or_else(|error| panic!("{name} one-push focus: {error}"));
        assert_eq!(
            wire(&FocusTrigger::OnePush),
            [0x81, 0x01, 0x04, 0x18, 0x01, 0xFF]
        );
    }
}
