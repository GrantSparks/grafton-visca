//! Independent wire-level behavior pins for the public noun registry.
//!
//! These tests intentionally keep their expected frames here instead of
//! deriving them from `noun_table` or the command-surface inventory.  The
//! registry is consumed by three public facades; a row that points at the
//! wrong branch, swaps an on/off or direction pair, or uses the wrong profile
//! conversion must fail against these absolute VISCA frames.

#![cfg(any(feature = "blocking", feature = "runtime-tokio"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use grafton_visca_test_support::fake_camera;

use fake_camera::{frames, FakeCamera, ZOOM_TELE};

const POWER_ON: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff];
const POWER_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x03, 0xff];

const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];

const FOCUS_FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xff];
const FOCUS_NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, 0xff];

const MENU_SELECT: &[u8] = &[0x81, 0x01, 0x06, 0x06, 0x05, 0xff];
const MENU_CANCEL: &[u8] = &[0x81, 0x01, 0x06, 0x06, 0x04, 0xff];

const TALLY_RED_ON: &[u8] = &[0x81, 0x01, 0x7e, 0x01, 0x0a, 0x00, 0x02, 0xff];
const TALLY_RED_OFF: &[u8] = &[0x81, 0x01, 0x7e, 0x01, 0x0a, 0x00, 0x03, 0xff];
const TALLY_GREEN_ON: &[u8] = &[0x81, 0x01, 0x7e, 0x04, 0x1a, 0x00, 0x02, 0xff];
const TALLY_GREEN_OFF: &[u8] = &[0x81, 0x01, 0x7e, 0x04, 0x1a, 0x00, 0x03, 0xff];

const PAN_TILT_UP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0a, 0x05, 0x03, 0x01, 0xff];
const PAN_TILT_DOWN: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0a, 0x05, 0x03, 0x02, 0xff];
const BRC300_ABSOLUTE_FASTEST: &[u8] = &[
    0x81, 0x01, 0x06, 0x02, 0x18, 0x00, 0x0f, 0x0d, 0x0b, 0x07, 0x00, 0x00, 0x0c, 0x03, 0x00, 0xff,
];
const BRC300_RELATIVE_FASTEST: &[u8] = &[
    0x81, 0x01, 0x06, 0x03, 0x18, 0x00, 0x0f, 0x0d, 0x0b, 0x07, 0x00, 0x00, 0x0c, 0x03, 0x00, 0xff,
];

// Sony FR7 documents optical max 0x4000 and combined optical+digital max
// 0x7000.  At 0.5 these become 0x2000 and 0x3800 respectively, encoded as
// VISCA nibbles in the direct-zoom command.
const ZOOM_NORMALIZED_OPTICAL_HALF: &[u8] = &[0x81, 0x01, 0x04, 0x47, 0x02, 0x00, 0x00, 0x00, 0xff];
const ZOOM_NORMALIZED_COMBINED_HALF: &[u8] =
    &[0x81, 0x01, 0x04, 0x47, 0x03, 0x08, 0x00, 0x00, 0xff];

const POWER_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x00, 0xff];
const MENU_STATUS_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x06, 0xff];
#[cfg(all(feature = "runtime-tokio", feature = "dyn-api"))]
const BACKLIGHT_ON: &[u8] = &[0x81, 0x01, 0x04, 0x33, 0x02, 0xff];
const BACKLIGHT_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x33, 0xff];

/// A small in-memory Sony VISCA-over-IP camera. It answers in the framing of
/// each request, returning ACK plus completion for commands and a typed
/// response for the boolean inquiries used below. The tests compare the
/// recorded VISCA payloads, keeping the noun assertions about command bytes
/// rather than envelope bookkeeping.
fn probe_camera() -> FakeCamera {
    FakeCamera::visca(|payload, answer| {
        let is_inquiry = payload.get(1) == Some(&0x09);
        let level = if payload == POWER_INQUIRY || payload == BACKLIGHT_INQUIRY {
            0x02
        } else if payload == MENU_STATUS_INQUIRY {
            0x03
        } else {
            0x00
        };
        if is_inquiry {
            answer.reply(frames::inquiry_reply(&[level]));
        } else {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

#[cfg(feature = "blocking")]
mod blocking_surface {
    use grafton_visca::{
        blocking::{Session, SessionConfig},
        command::{
            ExposureCommand, ExposureMode, ExposureModeInquiry, FocusZone, IrisControlInquiry,
            PanTiltDirection,
        },
        profile::ProfileSpec,
        profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300, SonyFR7},
        types::{PanSpeed, SpeedLevel, TiltSpeed},
        units::{Degrees, UnitInterval},
        Error, ZoomDomain,
    };

    use super::{
        probe_camera, FakeCamera, BRC300_ABSOLUTE_FASTEST, BRC300_RELATIVE_FASTEST, FOCUS_FAR,
        FOCUS_NEAR, MENU_CANCEL, MENU_SELECT, MENU_STATUS_INQUIRY, PAN_TILT_DOWN, PAN_TILT_UP,
        POWER_INQUIRY, POWER_OFF, POWER_ON, TALLY_GREEN_OFF, TALLY_GREEN_ON, TALLY_RED_OFF,
        TALLY_RED_ON, ZOOM_NORMALIZED_COMBINED_HALF, ZOOM_NORMALIZED_OPTICAL_HALF, ZOOM_TELE,
        ZOOM_WIDE,
    };

    fn open_with(profile: ProfileSpec) -> (Session, FakeCamera) {
        let camera = probe_camera();
        let session =
            Session::open(camera.blocking_wire(), SessionConfig::new(profile)).expect("session");
        (session, camera)
    }

    fn open() -> (Session, FakeCamera) {
        open_with(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"))
    }

    fn open_brc300() -> (Session, FakeCamera) {
        open_with(ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC-300 profile"))
    }

    #[test]
    fn blocking_power_zoom_focus_and_menu_rows_keep_their_wire_identity() {
        let (session, fake) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.power().on().expect("power on");
        assert_eq!(fake.take_only_payload(), POWER_ON);
        camera.power().off().expect("power off");
        assert_eq!(fake.take_only_payload(), POWER_OFF);

        camera
            .zoom()
            .tele()
            .expect("zoom tele")
            .applied()
            .expect("tele applied");
        assert_eq!(fake.take_only_payload(), ZOOM_TELE);
        camera
            .zoom()
            .wide()
            .expect("zoom wide")
            .applied()
            .expect("wide applied");
        assert_eq!(fake.take_only_payload(), ZOOM_WIDE);

        camera
            .focus()
            .far()
            .expect("focus far")
            .applied()
            .expect("far applied");
        assert_eq!(fake.take_only_payload(), FOCUS_FAR);
        camera
            .focus()
            .near()
            .expect("focus near")
            .applied()
            .expect("near applied");
        assert_eq!(fake.take_only_payload(), FOCUS_NEAR);

        camera.menu().select().expect("menu select");
        assert_eq!(fake.take_only_payload(), MENU_SELECT);
        camera.menu().cancel().expect("menu cancel");
        assert_eq!(fake.take_only_payload(), MENU_CANCEL);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_fr7_red_and_green_tally_rows_are_distinct() {
        let (session, fake) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.tally().red_on().expect("red tally on");
        assert_eq!(fake.take_only_payload(), TALLY_RED_ON);
        camera.tally().red_off().expect("red tally off");
        assert_eq!(fake.take_only_payload(), TALLY_RED_OFF);
        camera.tally().green_on().expect("green tally on");
        assert_eq!(fake.take_only_payload(), TALLY_GREEN_ON);
        camera.tally().green_off().expect("green tally off");
        assert_eq!(fake.take_only_payload(), TALLY_GREEN_OFF);
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_pan_tilt_directions_and_normalized_zoom_use_profile_values() {
        let (session, fake) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");
        let pan_speed = PanSpeed::new(0x0a).expect("pan speed");
        let tilt_speed = TiltSpeed::new(0x05).expect("tilt speed");

        camera
            .pan_tilt()
            .move_direction(PanTiltDirection::Up, pan_speed, tilt_speed)
            .expect("pan/tilt up")
            .applied()
            .expect("up applied");
        assert_eq!(fake.take_only_payload(), PAN_TILT_UP);
        camera
            .pan_tilt()
            .move_direction(PanTiltDirection::Down, pan_speed, tilt_speed)
            .expect("pan/tilt down")
            .applied()
            .expect("down applied");
        assert_eq!(fake.take_only_payload(), PAN_TILT_DOWN);

        let half = UnitInterval::new(0.5).expect("unit interval");
        camera
            .zoom()
            .set_normalized(half, ZoomDomain::Optical)
            .expect("optical normalized zoom")
            .applied()
            .expect("optical target applied");
        assert_eq!(fake.take_only_payload(), ZOOM_NORMALIZED_OPTICAL_HALF);
        camera
            .zoom()
            .set_normalized(half, ZoomDomain::OpticalPlusDigital)
            .expect("combined normalized zoom")
            .applied()
            .expect("combined target applied");
        assert_eq!(fake.take_only_payload(), ZOOM_NORMALIZED_COMBINED_HALF);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_brc300_speed_level_noun_mirrors_its_one_position_speed() {
        let (session, fake) = open_brc300();
        let camera = session.camera::<SonyBRC300>().expect("camera");

        camera
            .pan_tilt()
            .absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fastest)
            .expect("BRC-300 absolute noun")
            .applied()
            .expect("BRC-300 absolute applied");
        assert_eq!(fake.take_only_payload(), BRC300_ABSOLUTE_FASTEST);

        camera
            .pan_tilt()
            .relative(Degrees(45.0), Degrees(15.0), SpeedLevel::Fastest)
            .expect("BRC-300 relative noun")
            .applied()
            .expect("BRC-300 relative applied");
        assert_eq!(fake.take_only_payload(), BRC300_RELATIVE_FASTEST);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_same_response_type_inquiries_use_their_own_wire_queries() {
        let (session, fake) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");

        assert!(camera.power().state().expect("power inquiry"));
        assert_eq!(fake.take_only_payload(), POWER_INQUIRY);
        assert!(!camera.menu().status().expect("menu inquiry"));
        assert_eq!(fake.take_only_payload(), MENU_STATUS_INQUIRY);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_direct_exposure_command_rejects_before_any_write() {
        let (session, fake) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");

        let error = camera
            .execute(&ExposureCommand::new(ExposureMode::Auto))
            .expect_err("Sony FR7 does not support the shared exposure-mode command");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "shared exposure-mode family",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "rejected direct exposure-mode command must not reach the transport"
        );

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_direct_exposure_mode_inquiry_rejects_before_any_write() {
        let (session, fake) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");

        let error = camera
            .inquire(&ExposureModeInquiry)
            .expect_err("Sony FR7 does not support the shared exposure-mode inquiry");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "shared exposure-mode family",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "rejected direct exposure-mode inquiry must not reach the transport"
        );

        session.shutdown().expect("shutdown");
    }

    /// `FocusZone::Zone03` is evidenced only for the G2 family (G2 and the
    /// legacy 30X). G3 supports focus-zone selection, so the static
    /// `set_zone` compiles, but the value is refused before any write.
    #[test]
    fn blocking_focus_zone_03_is_refused_on_g3_before_any_write() {
        let (session, fake) =
            open_with(ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile"));
        let camera = session.camera::<PtzOpticsG3>().expect("camera");

        let error = camera
            .focus()
            .set_zone(FocusZone::Zone03)
            .expect_err("G3 has no evidence for focus-zone value 03");
        assert!(
            matches!(
                &error,
                Error::InvalidParameter {
                    parameter: "focus_zone",
                    value,
                    ..
                } if value == "03"
            ),
            "unexpected error: {error:?}"
        );
        assert!(
            fake.writes().is_empty(),
            "a refused focus-zone value must not reach the transport"
        );

        camera
            .focus()
            .set_zone(FocusZone::Center)
            .expect("G3 accepts the documented center zone");
        assert_eq!(
            fake.take_only_payload(),
            [0x81, 0x01, 0x04, 0xAA, 0x01, 0xFF]
        );

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_focus_zone_03_reaches_the_wire_on_the_g2_family() {
        let (session, fake) =
            open_with(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"));
        session
            .camera::<PtzOpticsG2>()
            .expect("camera")
            .focus()
            .set_zone(FocusZone::Zone03)
            .expect("G2 accepts focus-zone value 03 (PTZOptics G2 bench, #795)");
        assert_eq!(
            fake.take_only_payload(),
            [0x81, 0x01, 0x04, 0xAA, 0x03, 0xFF]
        );
        session.shutdown().expect("shutdown");

        let (session, fake) =
            open_with(ProfileSpec::from_compile_time::<PtzOptics30X>().expect("30X profile"));
        session
            .camera::<PtzOptics30X>()
            .expect("camera")
            .focus()
            .set_zone(FocusZone::Zone03)
            .expect("legacy 30X G2 accepts focus-zone value 03 (PTZOptics G2 bench, #795)");
        assert_eq!(
            fake.take_only_payload(),
            [0x81, 0x01, 0x04, 0xAA, 0x03, 0xFF]
        );
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_direct_iris_control_inquiry_rejects_before_any_write() {
        let (session, fake) =
            open_with(ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile"));
        let camera = session.camera::<PtzOpticsG3>().expect("camera");

        let error = camera
            .inquire(&IrisControlInquiry)
            .expect_err("G3 does not document the standard iris control-status inquiry");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "typed inquiry IrisControlInquiry",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "rejected direct iris control-status inquiry must not reach the transport"
        );

        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_surface {
    use grafton_visca::{
        command::{
            ExposureCommand, ExposureMode, ExposureModeInquiry, IrisControlInquiry,
            PanTiltDirection,
        },
        profile::ProfileSpec,
        profiles::{PtzOpticsG3, SonyBRC300, SonyFR7},
        transport::AddressingMode,
        types::{PanSpeed, SpeedLevel, TiltSpeed},
        units::{Degrees, UnitInterval},
        Error, Session, SessionConfig, TokioRuntime, ZoomDomain,
    };

    #[cfg(feature = "dyn-api")]
    use grafton_visca::profiles::{GenericVisca, PtzOpticsG2};

    use super::{
        probe_camera, FakeCamera, BRC300_ABSOLUTE_FASTEST, BRC300_RELATIVE_FASTEST, FOCUS_FAR,
        FOCUS_NEAR, MENU_CANCEL, MENU_SELECT, MENU_STATUS_INQUIRY, PAN_TILT_DOWN, PAN_TILT_UP,
        POWER_INQUIRY, POWER_OFF, POWER_ON, TALLY_GREEN_OFF, TALLY_GREEN_ON, TALLY_RED_OFF,
        TALLY_RED_ON, ZOOM_NORMALIZED_COMBINED_HALF, ZOOM_NORMALIZED_OPTICAL_HALF, ZOOM_TELE,
        ZOOM_WIDE,
    };

    #[cfg(feature = "dyn-api")]
    use super::{BACKLIGHT_INQUIRY, BACKLIGHT_ON};

    async fn open_session(camera: &FakeCamera, profile: ProfileSpec) -> Session {
        Session::open(
            camera.async_wire().with_addressing(AddressingMode::Ip),
            SessionConfig::new(profile),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session")
    }

    #[tokio::test]
    async fn async_power_zoom_focus_and_menu_rows_keep_their_wire_identity() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.power().on().await.expect("power on");
        assert_eq!(fake.take_only_payload(), POWER_ON);
        camera.power().off().await.expect("power off");
        assert_eq!(fake.take_only_payload(), POWER_OFF);

        camera
            .zoom()
            .tele()
            .await
            .expect("zoom tele")
            .applied()
            .await
            .expect("tele applied");
        assert_eq!(fake.take_only_payload(), ZOOM_TELE);
        camera
            .zoom()
            .wide()
            .await
            .expect("zoom wide")
            .applied()
            .await
            .expect("wide applied");
        assert_eq!(fake.take_only_payload(), ZOOM_WIDE);

        camera
            .focus()
            .far()
            .await
            .expect("focus far")
            .applied()
            .await
            .expect("far applied");
        assert_eq!(fake.take_only_payload(), FOCUS_FAR);
        camera
            .focus()
            .near()
            .await
            .expect("focus near")
            .applied()
            .await
            .expect("near applied");
        assert_eq!(fake.take_only_payload(), FOCUS_NEAR);

        camera.menu().select().await.expect("menu select");
        assert_eq!(fake.take_only_payload(), MENU_SELECT);
        camera.menu().cancel().await.expect("menu cancel");
        assert_eq!(fake.take_only_payload(), MENU_CANCEL);

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn async_fr7_red_and_green_tally_rows_are_distinct() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.tally().red_on().await.expect("red tally on");
        assert_eq!(fake.take_only_payload(), TALLY_RED_ON);
        camera.tally().red_off().await.expect("red tally off");
        assert_eq!(fake.take_only_payload(), TALLY_RED_OFF);
        camera.tally().green_on().await.expect("green tally on");
        assert_eq!(fake.take_only_payload(), TALLY_GREEN_ON);
        camera.tally().green_off().await.expect("green tally off");
        assert_eq!(fake.take_only_payload(), TALLY_GREEN_OFF);
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn async_pan_tilt_directions_and_normalized_zoom_use_profile_values() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");
        let pan_speed = PanSpeed::new(0x0a).expect("pan speed");
        let tilt_speed = TiltSpeed::new(0x05).expect("tilt speed");

        camera
            .pan_tilt()
            .move_direction(PanTiltDirection::Up, pan_speed, tilt_speed)
            .await
            .expect("pan/tilt up")
            .applied()
            .await
            .expect("up applied");
        assert_eq!(fake.take_only_payload(), PAN_TILT_UP);
        camera
            .pan_tilt()
            .move_direction(PanTiltDirection::Down, pan_speed, tilt_speed)
            .await
            .expect("pan/tilt down")
            .applied()
            .await
            .expect("down applied");
        assert_eq!(fake.take_only_payload(), PAN_TILT_DOWN);

        let half = UnitInterval::new(0.5).expect("unit interval");
        camera
            .zoom()
            .set_normalized(half, ZoomDomain::Optical)
            .await
            .expect("optical normalized zoom")
            .applied()
            .await
            .expect("optical target applied");
        assert_eq!(fake.take_only_payload(), ZOOM_NORMALIZED_OPTICAL_HALF);
        camera
            .zoom()
            .set_normalized(half, ZoomDomain::OpticalPlusDigital)
            .await
            .expect("combined normalized zoom")
            .applied()
            .await
            .expect("combined target applied");
        assert_eq!(fake.take_only_payload(), ZOOM_NORMALIZED_COMBINED_HALF);

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn async_brc300_speed_level_noun_mirrors_its_one_position_speed() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC-300 profile"),
        )
        .await;
        let camera = session.camera::<SonyBRC300>().expect("camera");

        camera
            .pan_tilt()
            .absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fastest)
            .await
            .expect("BRC-300 absolute noun")
            .applied()
            .await
            .expect("BRC-300 absolute applied");
        assert_eq!(fake.take_only_payload(), BRC300_ABSOLUTE_FASTEST);

        camera
            .pan_tilt()
            .relative(Degrees(45.0), Degrees(15.0), SpeedLevel::Fastest)
            .await
            .expect("BRC-300 relative noun")
            .applied()
            .await
            .expect("BRC-300 relative applied");
        assert_eq!(fake.take_only_payload(), BRC300_RELATIVE_FASTEST);

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn async_same_response_type_inquiries_use_their_own_wire_queries() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        assert!(camera.power().state().await.expect("power inquiry"));
        assert_eq!(fake.take_only_payload(), POWER_INQUIRY);
        assert!(!camera.menu().status().await.expect("menu inquiry"));
        assert_eq!(fake.take_only_payload(), MENU_STATUS_INQUIRY);

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn async_direct_exposure_command_rejects_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        let error = camera
            .execute(&ExposureCommand::new(ExposureMode::Auto))
            .await
            .expect_err("Sony FR7 does not support the shared exposure-mode command");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "shared exposure-mode family",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "rejected direct exposure-mode command must not reach the transport"
        );

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn async_direct_exposure_mode_inquiry_rejects_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        let error = camera
            .inquire(&ExposureModeInquiry)
            .await
            .expect_err("Sony FR7 does not support the shared exposure-mode inquiry");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "shared exposure-mode family",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "rejected direct exposure-mode inquiry must not reach the transport"
        );

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn async_direct_iris_control_inquiry_rejects_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile"),
        )
        .await;
        let camera = session.camera::<PtzOpticsG3>().expect("camera");

        let error = camera
            .inquire(&IrisControlInquiry)
            .await
            .expect_err("G3 does not document the standard iris control-status inquiry");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "typed inquiry IrisControlInquiry",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "rejected direct iris control-status inquiry must not reach the transport"
        );

        session.shutdown().expect("shutdown");
    }

    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_unsupported_capability_fails_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"),
        )
        .await;
        let camera = session.camera_dyn().expect("dynamic camera");
        let error = camera
            .zoom()
            .set_digital_zoom(true)
            .await
            .expect_err("G2 has no documented digital zoom toggle");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "digital zoom",
                ..
            }
        ));
        assert!(fake.writes().is_empty());
        session.shutdown().expect("shutdown");
    }

    /// PTZOptics G2 answers `81 09 00 02 FF` with an unsourced 2-byte payload,
    /// so the erased surface refuses the Sony-format version inquiry before
    /// anything is written (PTZOptics G2 bench, 2026-10-04, #795).
    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_version_inquiry_on_g2_fails_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"),
        )
        .await;
        let camera = session.camera_dyn().expect("dynamic camera");

        let error = camera
            .system()
            .version()
            .await
            .expect_err("G2 does not reply in the Sony version layout");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "typed inquiry VersionInquiry",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "a refused version inquiry must not reach the transport"
        );
        session.shutdown().expect("shutdown");
    }

    /// Only the PTZOptics G2 color-temperature reply layout is sourced (#828
    /// M1), so the erased inquiry refuses every other profile before anything
    /// is written, including G3, which keeps the color-temperature controls.
    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_color_temperature_inquiry_without_a_sourced_reply_fails_before_any_write() {
        for (name, profile) in [
            (
                "PtzOpticsG3",
                ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile"),
            ),
            (
                "SonyEVIH100",
                ProfileSpec::from_compile_time::<grafton_visca::profiles::SonyEVIH100>()
                    .expect("EVI-H100 profile"),
            ),
        ] {
            let fake = probe_camera();
            let session = open_session(&fake, profile).await;
            let camera = session.camera_dyn().expect("dynamic camera");

            let error = camera
                .white_balance()
                .color_temperature()
                .await
                .expect_err("no sourced color-temperature reply layout");
            assert!(
                matches!(
                    error,
                    Error::FeatureNotSupported {
                        feature: "typed inquiry ColorTemperatureInquiry",
                        ..
                    }
                ),
                "{name}: {error:?}"
            );
            assert!(
                fake.writes().is_empty(),
                "{name}: a refused color-temperature inquiry must not reach the transport"
            );
            session.shutdown().expect("shutdown");
        }
    }

    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_focus_zone_03_on_g3_fails_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile"),
        )
        .await;
        let camera = session.camera_dyn().expect("dynamic camera");

        let error = camera
            .focus()
            .set_zone(grafton_visca::command::FocusZone::Zone03)
            .await
            .expect_err("G3 has no evidence for focus-zone value 03");
        assert!(
            matches!(
                &error,
                Error::InvalidParameter {
                    parameter: "focus_zone",
                    value,
                    ..
                } if value == "03"
            ),
            "unexpected error: {error:?}"
        );
        assert!(
            fake.writes().is_empty(),
            "a refused focus-zone value must not reach the transport"
        );
        session.shutdown().expect("shutdown");
    }

    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_flicker_inquiry_requires_its_vendor_marker_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<GenericVisca>().expect("Generic VISCA profile"),
        )
        .await;
        let camera = session.camera_dyn().expect("dynamic camera");

        let error = camera
            .exposure()
            .flicker_mode()
            .await
            .expect_err("Generic VISCA has no source-backed PTZOptics flicker inquiry");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "typed inquiry FlickerModeInquiry",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "a rejected flicker inquiry must not reach the transport"
        );
        session.shutdown().expect("shutdown");
    }

    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_iris_control_status_inquiry_requires_its_distinct_marker_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile"),
        )
        .await;
        let camera = session.camera_dyn().expect("dynamic camera");

        let error = camera
            .exposure()
            .iris_control()
            .await
            .expect_err("G3 has no model-specific source-backed iris control-status inquiry");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "typed inquiry IrisControlInquiry",
                ..
            }
        ));
        assert!(
            fake.writes().is_empty(),
            "a rejected dynamic iris control-status inquiry must not reach the transport"
        );
        session.shutdown().expect("shutdown");
    }

    /// Unsupported image rows remain unavailable whether a profile lacks the
    /// base noun entirely or exposes only other, independently gated image
    /// rows. The erased facade must refuse them before reaching the transport.
    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_image_noun_matches_the_static_base_gate() {
        for (label, profile) in [
            (
                "Generic VISCA",
                ProfileSpec::from_compile_time::<GenericVisca>().expect("Generic VISCA profile"),
            ),
            (
                "Sony BRC-300",
                ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC-300 profile"),
            ),
            (
                "PTZOptics G2",
                ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"),
            ),
        ] {
            let fake = probe_camera();
            let session = open_session(&fake, profile).await;
            let camera = session.camera_dyn().expect("dynamic camera");

            for error in [
                camera
                    .image()
                    .freeze_on()
                    .await
                    .expect_err("image freeze must be refused"),
                camera
                    .image()
                    .freeze_off()
                    .await
                    .expect_err("image freeze-off must be refused"),
                camera
                    .image()
                    .defog_level()
                    .await
                    .expect_err("image defog must be refused"),
            ] {
                assert!(
                    matches!(error, Error::FeatureNotSupported { .. }),
                    "{label} image row must fail through capability validation: {error:?}",
                );
            }
            assert!(
                fake.writes().is_empty(),
                "refused {label} image rows must not reach the wire",
            );
            session.shutdown().expect("shutdown");
        }
    }

    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_brc300_backlight_rows_are_reachable() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC-300 profile"),
        )
        .await;
        let camera = session.camera_dyn().expect("dynamic camera");

        assert!(camera.image().backlight().await.expect("backlight inquiry"));
        assert_eq!(fake.take_only_payload(), BACKLIGHT_INQUIRY);
        camera
            .image()
            .set_backlight(true)
            .await
            .expect("backlight on");
        assert_eq!(fake.take_only_payload(), BACKLIGHT_ON);

        session.shutdown().expect("shutdown");
    }

    /// The FR7 exposes only the source-backed red/green tally rows.  Its
    /// packed PTZOptics status, auto-adjust inquiry, brightness commands, and
    /// tally-mode commands must remain runtime-gated on the erased surface.
    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_fr7_unsupported_tally_rows_fail_before_any_write() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera_dyn().expect("dynamic camera");

        for result in [
            camera.tally().bright_lo().await,
            camera.tally().bright_hi().await,
            camera.tally().status().await.map(|_| ()),
            camera.tally().auto_adjust_enabled().await.map(|_| ()),
            camera.tally().on().await,
            camera.tally().off().await,
            camera.tally().flash().await,
        ] {
            assert!(
                matches!(result, Err(Error::FeatureNotSupported { .. })),
                "unsupported FR7 tally row must fail through capability validation: {result:?}",
            );
        }
        assert!(
            fake.writes().is_empty(),
            "rejected FR7 tally rows must not reach the wire",
        );
        session.shutdown().expect("shutdown");
    }

    /// The erased tally noun rejects both the Sony family and the independently
    /// gated PTZOptics family before I/O when neither surface is supported.
    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_tally_noun_refuses_as_one_on_a_profile_without_tally() {
        let fake = probe_camera();
        let session = open_session(
            &fake,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"),
        )
        .await;
        let camera = session.camera_dyn().expect("dynamic camera");

        // The tally-mode opcodes are refused, not admitted through a vendor
        // fallback; the red row is independently refused as well.
        for result in [
            camera.tally().on().await,
            camera.tally().off().await,
            camera.tally().flash().await,
            camera.tally().red_on().await,
        ] {
            assert!(
                matches!(result, Err(Error::FeatureNotSupported { .. })),
                "tally rows must be refused together on a profile without tally: {result:?}",
            );
        }
        assert!(
            fake.writes().is_empty(),
            "refused tally rows must not reach the wire",
        );
        session.shutdown().expect("shutdown");
    }
}
