//! Behavioural coverage for the convenience helpers #569 added.
//!
//! Every wrapper is checked against the explicit form it delegates to, on a
//! fake camera, so the test proves the bytes rather than the signature.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::time::{Duration, Instant};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    camera::{IdleWait, MotionQuery},
    command::{
        FlipState, FocusLock, ImageFlipMode, NdFilterMode, PanTiltDirection, PanTiltLimitCorner,
        PresetRecallSpeed, VariableSpeedMode,
    },
    profile::ProfileSpec,
    profiles::{PtzOpticsG2, SonyEVIH100, SonyFR7},
    types::{MotionSyncSpeed, NdiQuality, PanSpeed, TiltSpeed, ZoomPosition},
    units::{Degrees, UnitInterval},
    AffectedAxes, Error, ZoomDomain,
};

use fake_camera::{frames, FakeCamera};
use profile_fixtures::MotionSyncTypedSupport;

/// A camera that always answers ACK + completion, answering position
/// inquiries with a canned constant position. It records the VISCA payloads
/// only: a Sony sequence number differs between two otherwise identical
/// frames.
fn recording_camera() -> FakeCamera {
    FakeCamera::visca(|payload, answer| {
        if payload.starts_with(&[0x81, 0x09, 0x06, 0x12]) {
            answer.reply(frames::inquiry_reply(&[0; 8]));
        } else if payload.starts_with(&[0x81, 0x09, 0x04, 0x47])
            || payload.starts_with(&[0x81, 0x09, 0x04, 0x48])
        {
            answer.reply(frames::inquiry_reply(&[0; 4]));
        } else {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

fn open_session(profile: ProfileSpec) -> (Session, FakeCamera) {
    let camera = recording_camera();
    let session =
        Session::open(camera.blocking_wire(), SessionConfig::new(profile)).expect("session");
    (session, camera)
}

fn ptz_session() -> (Session, FakeCamera) {
    open_session(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile"))
}

fn fr7_session() -> (Session, FakeCamera) {
    open_session(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"))
}

fn evi_h100_session() -> (Session, FakeCamera) {
    open_session(ProfileSpec::from_compile_time::<SonyEVIH100>().expect("EVI-H100 profile"))
}

fn motion_sync_session() -> (Session, FakeCamera) {
    open_session(
        ProfileSpec::from_compile_time::<MotionSyncTypedSupport>().expect("motion-sync profile"),
    )
}

#[test]
fn directional_pan_tilt_drive_encodes_each_direction() {
    let (session, fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let pan = PanSpeed::new(6).expect("pan speed");
    let tilt = TiltSpeed::new(5).expect("tilt speed");

    // `81 01 06 01 vv ww 0p 0t FF`: pan speed, tilt speed, then the direction.
    for (direction, pan_byte, tilt_byte) in [
        (PanTiltDirection::Up, 0x03, 0x01),
        (PanTiltDirection::Down, 0x03, 0x02),
        (PanTiltDirection::Left, 0x01, 0x03),
        (PanTiltDirection::Right, 0x02, 0x03),
    ] {
        let mut operation = camera
            .pan_tilt()
            .move_direction(direction, pan, tilt)
            .expect("directional drive");
        operation.applied().expect("drive applied");

        assert_eq!(
            fake.take_only_payload(),
            vec![0x81, 0x01, 0x06, 0x01, 0x06, 0x05, pan_byte, tilt_byte, 0xff],
            "{direction:?} must encode its drive bytes",
        );
    }

    session.shutdown().expect("shutdown");
}

#[test]
fn normalized_zoom_maps_the_unit_interval_across_the_documented_domain() {
    let (session, fake) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let optical_max = *camera.capabilities().zoom_range_optical.end();
    let digital_max = *camera
        .capabilities()
        .zoom_range_digital
        .as_ref()
        .expect("FR7 documents a digital zoom range")
        .end();
    assert_ne!(optical_max, digital_max);

    let mut explicit = camera
        .zoom()
        .set_position(ZoomPosition::new(optical_max).expect("telephoto end"))
        .expect("explicit zoom target");
    explicit.applied().expect("explicit applied");
    let optical_frame = fake.take_only_payload();

    let mut normalized = camera
        .zoom()
        .set_normalized(UnitInterval::ONE, ZoomDomain::Optical)
        .expect("optical-domain zoom target");
    normalized.applied().expect("optical domain applied");
    assert_eq!(
        fake.take_only_payload(),
        optical_frame,
        "the optical domain normalizes across the optical range",
    );

    let mut explicit_digital = camera
        .zoom()
        .set_position(ZoomPosition::new(digital_max).expect("digital telephoto end"))
        .expect("explicit digital target");
    explicit_digital
        .applied()
        .expect("explicit digital applied");
    let digital_frame = fake.take_only_payload();

    let mut in_digital = camera
        .zoom()
        .set_normalized(UnitInterval::ONE, ZoomDomain::OpticalPlusDigital)
        .expect("digital-domain zoom target");
    in_digital.applied().expect("digital domain applied");
    assert_eq!(fake.take_only_payload(), digital_frame);
    assert_ne!(digital_frame, optical_frame);

    let mut wide = camera
        .zoom()
        .set_normalized(UnitInterval::ZERO, ZoomDomain::Optical)
        .expect("wide zoom target");
    wide.applied().expect("wide applied");
    let wide_frame = fake.take_only_payload();
    let mut wide_explicit = camera
        .zoom()
        .set_position(ZoomPosition::new(0).expect("wide end"))
        .expect("explicit wide target");
    wide_explicit.applied().expect("explicit wide applied");
    assert_eq!(fake.take_only_payload(), wide_frame);

    // The endpoints alone cannot tell the two domains apart from a mapping that
    // merely clamps: 0.0 is raw 0 in both and 1.0 is each domain's own maximum
    // whatever the curve in between. Probe the midpoint, where the two domains
    // must land on different raw positions, each the round-half-up half of its
    // own maximum.
    let midpoint = UnitInterval::new(0.5).expect("the midpoint is inside the unit interval");
    let half_optical = ZoomPosition::new(optical_max.div_ceil(2)).expect("half the optical range");
    let half_digital = ZoomPosition::new(digital_max.div_ceil(2)).expect("half the digital range");
    assert_ne!(half_optical, half_digital);

    let mut explicit_half_optical = camera
        .zoom()
        .set_position(half_optical)
        .expect("explicit optical midpoint");
    explicit_half_optical
        .applied()
        .expect("explicit optical midpoint applied");
    let half_optical_frame = fake.take_only_payload();

    let mut normalized_half = camera
        .zoom()
        .set_normalized(midpoint, ZoomDomain::Optical)
        .expect("normalized midpoint");
    normalized_half
        .applied()
        .expect("normalized midpoint applied");
    assert_eq!(
        fake.take_only_payload(),
        half_optical_frame,
        "the optical domain's midpoint is half the optical range",
    );

    let mut explicit_half_digital = camera
        .zoom()
        .set_position(half_digital)
        .expect("explicit digital midpoint");
    explicit_half_digital
        .applied()
        .expect("explicit digital midpoint applied");
    let half_digital_frame = fake.take_only_payload();
    assert_ne!(half_digital_frame, half_optical_frame);

    let mut normalized_half_digital = camera
        .zoom()
        .set_normalized(midpoint, ZoomDomain::OpticalPlusDigital)
        .expect("normalized digital midpoint");
    normalized_half_digital
        .applied()
        .expect("normalized digital midpoint applied");
    assert_eq!(
        fake.take_only_payload(),
        half_digital_frame,
        "the combined domain's midpoint is half the digital range",
    );

    session.shutdown().expect("shutdown");
}

#[test]
fn menu_toggle_sends_the_vendor_open_close_control() {
    let (session, fake) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");

    camera.menu().direct(0x00, 0x01).expect("explicit control");
    let explicit_frame = fake.take_only_payload();

    camera.menu().toggle_display().expect("menu toggle");
    let toggle_frame = fake.take_only_payload();

    assert_eq!(toggle_frame, explicit_frame);
    assert_eq!(
        toggle_frame,
        vec![0x81, 0x01, 0x7e, 0x04, 0x72, 0x00, 0x01, 0xff]
    );

    session.shutdown().expect("shutdown");
}

/// No built-in profile declares motion sync, so the accessor is driven here
/// over a synthetic profile that does. The helper is exercised through the
/// noun — not by rebuilding the command it happens to construct — so a
/// `set_speed` that ignored its argument would fail this test.
#[test]
fn motion_sync_speed_helper_drives_its_explicit_preset() {
    let (session, fake) = motion_sync_session();
    let camera = session
        .camera::<MotionSyncTypedSupport>()
        .expect("motion-sync camera");

    // `81 0A 11 14 pp FF`, pp = speed: the literal frame for speed 12.
    let explicit_frame = vec![0x81, 0x0A, 0x11, 0x14, 0x0C, 0xFF];
    camera
        .motion_sync()
        .set_speed(MotionSyncSpeed::new(12).expect("motion sync speed"))
        .expect("typed helper");
    assert_eq!(
        fake.take_only_payload(),
        explicit_frame,
        "set_speed must encode the speed it was given"
    );

    // A different speed must not encode the same frame, which is what makes
    // the equality above a statement about the argument rather than about the
    // opcode alone.
    camera
        .motion_sync()
        .set_speed(MotionSyncSpeed::new(1).expect("motion sync speed"))
        .expect("typed helper");
    assert_ne!(fake.take_only_payload(), explicit_frame);

    // The bound lives in the argument type, so an out-of-range speed cannot be
    // constructed and therefore cannot reach the wire.
    assert!(MotionSyncSpeed::new(0).is_err());
    assert!(MotionSyncSpeed::new(25).is_err());
    assert!(fake.take_payloads().is_empty());

    session.shutdown().expect("shutdown");
}

#[test]
fn default_motion_query_samples_every_mechanical_movement_axis() {
    let (session, fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    assert!(!camera
        .motion()
        .is_moving(MotionQuery::default())
        .expect("default motion query"));
    let default_frames = fake.take_payloads();

    assert!(!camera
        .motion()
        .is_moving(MotionQuery::new(AffectedAxes::MOVEMENT))
        .expect("explicit motion query"));
    let explicit_frames = fake.take_payloads();

    assert_eq!(default_frames, explicit_frames);
    // Two complete snapshots over pan/tilt, zoom, and focus.
    assert_eq!(default_frames.len(), 6);

    session.shutdown().expect("shutdown");
}

/// #712: matched raw inquiry replies do not create a one-second dispatch hold.
/// PtzOpticsG2's 150 ms physical inquiry spacing still permits sustained
/// polling above 5 Hz, and it never delays an urgent stop command.
#[test]
fn successful_raw_inquiries_keep_polling_fast_and_urgent_stop_immediate() {
    let (session, fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    let polling_started = Instant::now();
    for _ in 0..6 {
        camera.zoom().position().expect("raw zoom position reply");
    }
    let polling_elapsed = polling_started.elapsed();
    assert!(
        polling_elapsed <= Duration::from_millis(1_200),
        "six replies exceeded the 5 Hz floor: {polling_elapsed:?}"
    );

    let stop_started = Instant::now();
    let mut stop = camera.zoom().stop().expect("urgent stop reaches the wire");
    let stop_latency = stop_started.elapsed();
    assert!(
        stop_latency < Duration::from_millis(50),
        "matched inquiry delayed urgent stop by {stop_latency:?}"
    );
    stop.applied().expect("urgent stop applies");

    assert_eq!(fake.take_payloads().len(), 7);
    session.shutdown().expect("shutdown");
}

#[test]
fn named_idle_wait_presets_poll_only_their_own_axis() {
    let (session, fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    camera
        .motion()
        .wait_until_idle(IdleWait::for_zoom().with_interval(Duration::ZERO))
        .expect("zoom idle wait");
    let frames = fake.take_payloads();
    assert!(!frames.is_empty());
    assert!(
        frames
            .iter()
            .all(|bytes| bytes.as_slice() == [0x81, 0x09, 0x04, 0x47, 0xff]),
        "for_zoom must poll only the zoom position inquiry",
    );

    camera
        .motion()
        .wait_until_idle(IdleWait::from(Duration::from_secs(30)).with_interval(Duration::ZERO))
        .expect("duration idle wait");
    let frames = fake.take_payloads();
    assert!(frames
        .iter()
        .any(|bytes| bytes.as_slice() == [0x81, 0x09, 0x06, 0x12, 0xff]));
    assert!(
        frames
            .iter()
            .any(|bytes| bytes.as_slice() == [0x81, 0x09, 0x04, 0x47, 0xff]),
        "duration idle wait must poll zoom",
    );
    assert!(frames
        .iter()
        .any(|bytes| bytes.as_slice() == [0x81, 0x09, 0x04, 0x48, 0xff]));

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cameras_expose_their_runtime_capability_inventory() {
    let (session, _fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let capabilities = camera.capabilities();
    assert_eq!(capabilities.model_name, "PtzOptics G2");
    assert!(capabilities.has_pan_tilt);

    // `camera.capabilities()` is defined as `camera.profile().capabilities()`,
    // so comparing those two by address asserts nothing. Compare against an
    // *independently* built spec for the same compile-time profile instead:
    // separate storage (so the pointer check is a real inequality), equal
    // content (so the camera view really is the registry's inventory).
    let independent = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile");
    assert!(!std::ptr::eq(capabilities, independent.capabilities()));
    assert_eq!(capabilities, independent.capabilities());

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cache_getters_decode_the_combined_flip_pair() {
    let (session, _fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.flip_state(), None);

    camera
        .image()
        .set_flip_mode(ImageFlipMode::Both)
        .expect("combined flip");
    assert_eq!(
        cache.flip_state(),
        Some(FlipState {
            horizontal: true,
            vertical: true,
        })
    );

    camera
        .image()
        .set_flip_mode(ImageFlipMode::Horizontal)
        .expect("combined flip");
    assert_eq!(
        cache.flip_state(),
        Some(FlipState {
            horizontal: true,
            vertical: false,
        })
    );

    // A single-axis opcode moves one axis and leaves the other unknown, so the
    // complete pair stops being known rather than going stale.
    camera.image().enable_flip().expect("separate flip");
    assert_eq!(cache.flip_state(), None);

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cache_getters_decode_pan_tilt_limit_updates() {
    let (session, fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.pan_tilt_limits(), None);

    camera
        .pan_tilt()
        .limit_set(
            PanTiltLimitCorner::DownLeft,
            Degrees::new(0.0),
            Degrees::new(0.0),
        )
        .expect("limit set");
    assert_eq!(
        fake.take_only_payload(),
        [
            0x81, 0x01, 0x06, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xFF,
        ]
    );
    let update = cache.pan_tilt_limits().expect("recorded limit update");
    assert_eq!(update.corner(), PanTiltLimitCorner::DownLeft);
    assert!(!update.is_cleared());
    assert_eq!(
        update.position().map(|position| position.raw_values()),
        Some((0, 0))
    );

    // The origin is the one point where both axes agree and both signs vanish,
    // so it cannot see a swapped, dropped, or truncated axis. Probe a corner
    // whose two axes differ: 90 degrees of pan and 30 of tilt, converted by the
    // profile's own degrees-to-units factors.
    camera
        .pan_tilt()
        .limit_set(
            PanTiltLimitCorner::UpRight,
            Degrees::new(90.0),
            Degrees::new(30.0),
        )
        .expect("limit set");
    assert_eq!(
        fake.take_only_payload(),
        [
            0x81, 0x01, 0x06, 0x07, 0x00, 0x01, 0x00, 0x05, 0x01, 0x00, 0x00, 0x01, 0x0B, 0x00,
            0xFF,
        ]
    );
    let update = cache.pan_tilt_limits().expect("recorded limit update");
    assert_eq!(update.corner(), PanTiltLimitCorner::UpRight);
    assert!(!update.is_cleared());
    let (pan, tilt) = update
        .position()
        .expect("a limit set records a position")
        .raw_values();
    assert_ne!(pan, tilt, "the probe must be able to see swapped axes");
    assert_eq!((pan, tilt), (1296, 432));

    camera
        .pan_tilt()
        .limit_clear(PanTiltLimitCorner::DownLeft)
        .expect("limit clear");
    assert_eq!(
        fake.take_only_payload(),
        [
            0x81, 0x01, 0x06, 0x07, 0x01, 0x00, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
            0xFF,
        ]
    );
    let update = cache.pan_tilt_limits().expect("recorded limit clear");
    assert_eq!(update.corner(), PanTiltLimitCorner::DownLeft);
    assert!(update.is_cleared());
    assert_eq!(update.position(), None);

    camera
        .pan_tilt()
        .limit_clear(PanTiltLimitCorner::UpRight)
        .expect("limit clear");
    assert_eq!(
        fake.take_only_payload(),
        [
            0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
            0xFF,
        ]
    );
    let update = cache.pan_tilt_limits().expect("recorded limit clear");
    assert_eq!(update.corner(), PanTiltLimitCorner::UpRight);
    assert!(update.is_cleared());
    assert_eq!(update.position(), None);

    session.shutdown().expect("shutdown");
}

/// PTZOptics tally-mode candidates are a distinct, unevidenced family from the
/// FR7 red/green tally commands and must fail before transport I/O.
#[test]
fn ptzoptics_tally_mode_candidates_are_rejected_on_fr7() {
    use grafton_visca::command::{TallyFlash, TallyOff, TallyOn};

    let (session, fake) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.tally_mode(), None);
    for result in [
        camera.execute(&TallyOn),
        camera.execute(&TallyOff),
        camera.execute(&TallyFlash),
    ] {
        assert!(
            matches!(result, Err(Error::FeatureNotSupported { .. })),
            "unsupported PTZOptics tally mode must be rejected before I/O: {result:?}"
        );
    }
    assert!(
        fake.take_payloads().is_empty(),
        "rejected tally rows must not reach I/O"
    );
    assert_eq!(cache.tally_mode(), None);

    session.shutdown().expect("shutdown");
}

/// Rejection through direct request admission also leaves the cache unknown.
#[test]
fn direct_vendor_tally_mode_rejection_does_not_mutate_cache() {
    use grafton_visca::command::{TallyFlash, TallyOff, TallyOn};

    let (session, fake) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.tally_mode(), None);

    for result in [
        camera.execute(&TallyOn),
        camera.execute(&TallyOff),
        camera.execute(&TallyFlash),
    ] {
        assert!(
            matches!(result, Err(Error::FeatureNotSupported { .. })),
            "unsupported PTZOptics tally mode must be rejected before I/O: {result:?}"
        );
    }
    assert!(
        fake.take_payloads().is_empty(),
        "rejected tally rows must not reach I/O"
    );
    assert_eq!(cache.tally_mode(), None);

    session.shutdown().expect("shutdown");
}

/// `focus_lock` had no decode-side coverage: the projection that stores `1`/`0`
/// was tested, the getter that reads it back was not.
#[test]
fn typed_cache_getters_decode_the_focus_lock_mode() {
    let (session, _fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.focus_lock(), None);

    camera
        .focus()
        .set_lock(FocusLock::On)
        .expect("focus lock on");
    assert_eq!(cache.focus_lock(), Some(true));

    camera
        .focus()
        .set_lock(FocusLock::Off)
        .expect("focus lock off");
    assert_eq!(cache.focus_lock(), Some(false));

    session.shutdown().expect("shutdown");
}

/// Source-backed FR7 write-only keys remain available; unevidenced tally
/// brightness remains unknown.
#[test]
fn typed_cache_getters_decode_the_remaining_write_only_keys() {
    let (session, _fake) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.digital_zoom(), None);
    assert_eq!(cache.tally_brightness_is_high(), None);
    assert_eq!(cache.variable_speed_mode(), None);

    camera.zoom().set_digital_zoom(true).expect("digital zoom");
    assert_eq!(cache.digital_zoom(), Some(true));
    camera.zoom().set_digital_zoom(false).expect("digital zoom");
    assert_eq!(cache.digital_zoom(), Some(false));

    assert_eq!(cache.tally_brightness_is_high(), None);

    camera
        .advanced()
        .set_variable_speed_mode(VariableSpeedMode::Fine50)
        .expect("variable speed mode");
    assert_eq!(cache.variable_speed_mode(), Some(VariableSpeedMode::Fine50));
    camera
        .advanced()
        .set_variable_speed_mode(VariableSpeedMode::Standard24)
        .expect("variable speed mode");
    assert_eq!(
        cache.variable_speed_mode(),
        Some(VariableSpeedMode::Standard24)
    );

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cache_getters_decode_the_sony_write_only_toggles() {
    let (session, _fake) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.spotlight(), None);
    camera.exposure().spotlight_on().expect("spotlight on");
    assert_eq!(cache.spotlight(), Some(true));
    camera.exposure().spotlight_off().expect("spotlight off");
    assert_eq!(cache.spotlight(), Some(false));

    session.shutdown().expect("shutdown");

    let (session, _fake) = evi_h100_session();
    let camera = session.camera::<SonyEVIH100>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.auto_slow_shutter(), None);

    camera
        .exposure()
        .auto_slow_shutter_on()
        .expect("auto slow shutter on");
    assert_eq!(cache.auto_slow_shutter(), Some(true));
    camera
        .exposure()
        .auto_slow_shutter_off()
        .expect("auto slow shutter off");
    assert_eq!(cache.auto_slow_shutter(), Some(false));

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cache_getters_decode_the_scalar_and_boolean_keys() {
    let (session, fake) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.preset_recall_speed(), None);
    assert_eq!(cache.multicast_streaming(), None);
    assert_eq!(cache.ndi_quality(), None);
    assert_eq!(cache.image_freeze(), None);

    camera
        .presets()
        .set_recall_speed(PresetRecallSpeed::new(12).expect("preset recall speed"))
        .expect("recall speed");
    assert_eq!(cache.preset_recall_speed(), Some(12));

    camera.advanced().multicast_on().expect("multicast on");
    assert_eq!(cache.multicast_streaming(), Some(true));
    camera.advanced().multicast_off().expect("multicast off");
    assert_eq!(cache.multicast_streaming(), Some(false));

    camera
        .advanced()
        .set_ndi_quality(NdiQuality::Medium)
        .expect("ndi quality");
    assert_eq!(cache.ndi_quality(), Some(NdiQuality::Medium));

    let writes_before = fake.write_count();
    let error = camera
        .execute(&grafton_visca::command::ImageFreeze::on())
        .expect_err("unsupported image-freeze must be rejected before I/O");
    assert!(
        matches!(error, Error::FeatureNotSupported { .. }),
        "image-freeze must be rejected by the profile gate: {error:?}"
    );
    assert_eq!(
        fake.write_count(),
        writes_before,
        "rejected image-freeze must not reach I/O"
    );
    assert_eq!(cache.image_freeze(), None);

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cache_getters_decode_the_nd_filter_keys() {
    let (session, _fake) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.nd_filter_mode(), None);
    assert_eq!(cache.auto_nd_filter(), None);

    camera
        .nd_filter()
        .set_mode(NdFilterMode::Variable)
        .expect("nd mode");
    assert_eq!(cache.nd_filter_mode(), Some(NdFilterMode::Variable));
    camera
        .nd_filter()
        .set_mode(NdFilterMode::Preset)
        .expect("nd mode");
    assert_eq!(cache.nd_filter_mode(), Some(NdFilterMode::Preset));

    camera.nd_filter().auto_on().expect("auto nd on");
    assert_eq!(cache.auto_nd_filter(), Some(true));
    camera.nd_filter().auto_off().expect("auto nd off");
    assert_eq!(cache.auto_nd_filter(), Some(false));

    session.shutdown().expect("shutdown");
}
