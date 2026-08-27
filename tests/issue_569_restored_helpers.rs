//! Behavioural coverage for the 1.x convenience helpers restored in #569.
//!
//! Every wrapper is checked against the explicit form it delegates to, on a
//! scripted transport, so the test proves the bytes rather than the signature.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::{Session, SessionConfig},
    camera::{IdleWait, MotionQuery, TransportKind},
    command::{
        CommandKind, FlipState, FocusLock, ImageFlipMode, NdFilterMode, PanTiltDirection,
        PanTiltLimitCorner, PresetRecallSpeed, VariableSpeedMode,
    },
    profile::ProfileSpec,
    profiles::{PtzOpticsG2, SonyFR7},
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    types::{MotionSyncSpeed, NdiQuality, PanSpeed, TiltSpeed, ZoomPosition},
    units::{Degrees, UnitInterval},
    AffectedAxes, Error, ZoomDomain,
};
use profile_fixtures::MotionSyncTypedSupport;

/// A transport that records every frame and always answers ACK + completion,
/// answering position inquiries from a canned constant position.
#[derive(Debug)]
struct RecordingTransport {
    config: TransportConfig,
    responses: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl RecordingTransport {
    fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses: VecDeque::new(),
                writes: Arc::clone(&writes),
            },
            writes,
        )
    }

    /// Splits a written frame into its optional Sony header and VISCA payload.
    fn split(bytes: &[u8]) -> (Option<u32>, &[u8]) {
        let sony = bytes.len() > 8
            && bytes[0] == 0x01
            && matches!(bytes[1], 0x00 | 0x10 | 0x02 | 0x20)
            && usize::from(u16::from_be_bytes([bytes[2], bytes[3]])) == bytes.len() - 8;
        if sony {
            let sequence = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
            (Some(sequence), &bytes[8..])
        } else {
            (None, bytes)
        }
    }

    /// Wraps a VISCA reply in the Sony reply envelope when one is in use.
    fn envelope(sequence: Option<u32>, payload: Vec<u8>) -> Vec<u8> {
        let Some(sequence) = sequence else {
            return payload;
        };
        let mut frame = Vec::with_capacity(payload.len() + 8);
        frame.extend_from_slice(&[0x01, 0x11]);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        frame.extend_from_slice(&sequence.to_be_bytes());
        frame.extend_from_slice(&payload);
        frame
    }

    fn response_for(sequence: Option<u32>, payload: &[u8]) -> Vec<Vec<u8>> {
        let replies = if payload.starts_with(&[0x81, 0x09, 0x06, 0x12]) {
            vec![vec![0x90, 0x50, 0, 0, 0, 0, 0, 0, 0, 0, 0xff]]
        } else if payload.starts_with(&[0x81, 0x09, 0x04, 0x47])
            || payload.starts_with(&[0x81, 0x09, 0x04, 0x48])
        {
            vec![vec![0x90, 0x50, 0, 0, 0, 0, 0xff]]
        } else {
            vec![vec![0x90, 0x41, 0xff], vec![0x90, 0x51, 0xff]]
        };
        replies
            .into_iter()
            .map(|reply| Self::envelope(sequence, reply))
            .collect()
    }
}

impl HasTransportConfig for RecordingTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<TransportKind> {
        None
    }
}

impl BlockingTransport for RecordingTransport {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        // Only the VISCA payload is recorded: a Sony sequence number differs
        // between two otherwise identical frames.
        let (sequence, payload) = Self::split(bytes);
        self.writes
            .lock()
            .expect("writes lock")
            .push(payload.to_vec());
        let responses = Self::response_for(sequence, payload);
        self.responses.extend(responses);
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_secs(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        let response = self.responses.pop_front().ok_or(Error::Timeout)?;
        dst[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

/// Returns every frame written since the last call and clears the log.
fn drain(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<Vec<u8>> {
    let mut guard = writes.lock().expect("writes lock");
    std::mem::take(&mut *guard)
}

/// Returns the single frame written since the last call.
fn one_frame(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<u8> {
    let mut frames = drain(writes);
    assert_eq!(frames.len(), 1, "expected exactly one written frame");
    frames.remove(0)
}

fn ptz_session() -> (Session, Arc<Mutex<Vec<Vec<u8>>>>) {
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile");
    let (transport, writes) = RecordingTransport::new();
    let session = Session::open(transport, SessionConfig::new(profile)).expect("session");
    (session, writes)
}

fn fr7_session() -> (Session, Arc<Mutex<Vec<Vec<u8>>>>) {
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
    let (transport, writes) = RecordingTransport::new();
    let session = Session::open(transport, SessionConfig::new(profile)).expect("session");
    (session, writes)
}

fn motion_sync_session() -> (Session, Arc<Mutex<Vec<Vec<u8>>>>) {
    let profile =
        ProfileSpec::from_compile_time::<MotionSyncTypedSupport>().expect("motion-sync profile");
    let (transport, writes) = RecordingTransport::new();
    let session = Session::open(transport, SessionConfig::new(profile)).expect("session");
    (session, writes)
}

#[test]
fn directional_pan_tilt_helpers_encode_their_explicit_drive() {
    let (session, writes) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let pan = PanSpeed::new(6).expect("pan speed");
    let tilt = TiltSpeed::new(5).expect("tilt speed");

    for (direction, helper) in [
        (PanTiltDirection::Up, 0_u8),
        (PanTiltDirection::Down, 1),
        (PanTiltDirection::Left, 2),
        (PanTiltDirection::Right, 3),
    ] {
        let accessor = camera.pan_tilt();
        let explicit = accessor
            .move_direction(direction, pan, tilt)
            .expect("explicit drive");
        explicit.applied().expect("explicit applied");
        let explicit_frame = one_frame(&writes);

        let operation = match helper {
            0 => accessor.up(pan, tilt),
            1 => accessor.down(pan, tilt),
            2 => accessor.left(pan, tilt),
            _ => accessor.right(pan, tilt),
        }
        .expect("directional helper");
        operation.applied().expect("helper applied");
        let helper_frame = one_frame(&writes);

        assert_eq!(
            helper_frame, explicit_frame,
            "directional helper for {direction:?} must encode its explicit drive",
        );
    }

    session.shutdown().expect("shutdown");
}

#[test]
fn normalized_zoom_maps_the_unit_interval_across_the_documented_domain() {
    let (session, writes) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let optical_max = *camera.capabilities().zoom_range_optical.end();
    let digital_max = *camera
        .capabilities()
        .zoom_range_digital
        .as_ref()
        .expect("FR7 documents a digital zoom range")
        .end();
    assert_ne!(optical_max, digital_max);

    let explicit = camera
        .zoom()
        .set_position(ZoomPosition::new(optical_max).expect("telephoto end"))
        .expect("explicit zoom target");
    explicit.applied().expect("explicit applied");
    let optical_frame = one_frame(&writes);

    let normalized = camera
        .zoom()
        .set_normalized(UnitInterval::ONE)
        .expect("normalized zoom target");
    normalized.applied().expect("normalized applied");
    assert_eq!(
        one_frame(&writes),
        optical_frame,
        "set_normalized always normalizes across the optical range",
    );

    let in_optical = camera
        .zoom()
        .set_normalized_in_domain(UnitInterval::ONE, ZoomDomain::Optical)
        .expect("optical-domain zoom target");
    in_optical.applied().expect("optical domain applied");
    assert_eq!(one_frame(&writes), optical_frame);

    let explicit_digital = camera
        .zoom()
        .set_position(ZoomPosition::new(digital_max).expect("digital telephoto end"))
        .expect("explicit digital target");
    explicit_digital
        .applied()
        .expect("explicit digital applied");
    let digital_frame = one_frame(&writes);

    let in_digital = camera
        .zoom()
        .set_normalized_in_domain(UnitInterval::ONE, ZoomDomain::OpticalPlusDigital)
        .expect("digital-domain zoom target");
    in_digital.applied().expect("digital domain applied");
    assert_eq!(one_frame(&writes), digital_frame);
    assert_ne!(digital_frame, optical_frame);

    let wide = camera
        .zoom()
        .set_normalized(UnitInterval::ZERO)
        .expect("wide zoom target");
    wide.applied().expect("wide applied");
    let wide_frame = one_frame(&writes);
    let wide_explicit = camera
        .zoom()
        .set_position(ZoomPosition::new(0).expect("wide end"))
        .expect("explicit wide target");
    wide_explicit.applied().expect("explicit wide applied");
    assert_eq!(one_frame(&writes), wide_frame);

    // The endpoints alone cannot tell the two domains apart from a mapping that
    // merely clamps: 0.0 is raw 0 in both and 1.0 is each domain's own maximum
    // whatever the curve in between. Probe the midpoint, where the two domains
    // must land on different raw positions, each the round-half-up half of its
    // own maximum.
    let midpoint = UnitInterval::new(0.5).expect("the midpoint is inside the unit interval");
    let half_optical = ZoomPosition::new(optical_max.div_ceil(2)).expect("half the optical range");
    let half_digital = ZoomPosition::new(digital_max.div_ceil(2)).expect("half the digital range");
    assert_ne!(half_optical, half_digital);

    let explicit_half_optical = camera
        .zoom()
        .set_position(half_optical)
        .expect("explicit optical midpoint");
    explicit_half_optical
        .applied()
        .expect("explicit optical midpoint applied");
    let half_optical_frame = one_frame(&writes);

    let normalized_half = camera
        .zoom()
        .set_normalized(midpoint)
        .expect("normalized midpoint");
    normalized_half
        .applied()
        .expect("normalized midpoint applied");
    assert_eq!(
        one_frame(&writes),
        half_optical_frame,
        "the default domain's midpoint is half the optical range",
    );

    let explicit_half_digital = camera
        .zoom()
        .set_position(half_digital)
        .expect("explicit digital midpoint");
    explicit_half_digital
        .applied()
        .expect("explicit digital midpoint applied");
    let half_digital_frame = one_frame(&writes);
    assert_ne!(half_digital_frame, half_optical_frame);

    let normalized_half_digital = camera
        .zoom()
        .set_normalized_in_domain(midpoint, ZoomDomain::OpticalPlusDigital)
        .expect("normalized digital midpoint");
    normalized_half_digital
        .applied()
        .expect("normalized digital midpoint applied");
    assert_eq!(
        one_frame(&writes),
        half_digital_frame,
        "the combined domain's midpoint is half the digital range",
    );

    session.shutdown().expect("shutdown");
}

#[test]
fn menu_toggle_sends_the_vendor_open_close_control() {
    let (session, writes) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");

    camera.menu().direct(0x00, 0x01).expect("explicit control");
    let explicit_frame = one_frame(&writes);

    camera.menu().toggle_display().expect("menu toggle");
    let toggle_frame = one_frame(&writes);

    assert_eq!(toggle_frame, explicit_frame);
    assert_eq!(
        toggle_frame,
        vec![0x81, 0x01, 0x7e, 0x04, 0x72, 0x00, 0x01, 0xff]
    );

    session.shutdown().expect("shutdown");
}

#[test]
fn nd_filter_stops_map_onto_the_raw_direct_value() {
    let (session, writes) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");

    // 2.0 stops is the minimum density and each raw unit is a quarter stop,
    // so 4.5 stops is raw 10.
    let explicit = camera.nd_filter().set_value(10).expect("explicit nd value");
    explicit.applied().expect("explicit applied");
    let explicit_frame = one_frame(&writes);

    let by_stops = camera.nd_filter().set_stops(4.5).expect("nd stops");
    by_stops.applied().expect("stops applied");
    assert_eq!(one_frame(&writes), explicit_frame);

    assert!(camera.nd_filter().set_stops(1.5).is_err());
    assert!(camera.nd_filter().set_stops(8.0).is_err());
    assert!(
        drain(&writes).is_empty(),
        "a rejected stop count writes nothing"
    );

    session.shutdown().expect("shutdown");
}

/// No built-in profile declares motion sync, so the accessor is driven here
/// over a synthetic profile that does. The helper is exercised through the
/// noun — not by rebuilding the command it happens to construct — so a
/// `set_speed` that ignored its argument would fail this test.
#[test]
fn motion_sync_speed_helper_drives_its_explicit_preset() {
    let (session, writes) = motion_sync_session();
    let camera = session
        .camera::<MotionSyncTypedSupport>()
        .expect("motion-sync camera");

    // The raw-`u8` twin is the explicit form the typed helper delegates to.
    camera
        .motion_sync()
        .set_preset(12)
        .expect("explicit preset");
    let explicit_frame = one_frame(&writes);

    camera
        .motion_sync()
        .set_speed(MotionSyncSpeed::new(12).expect("motion sync speed"))
        .expect("typed helper");
    assert_eq!(
        one_frame(&writes),
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
    assert_ne!(one_frame(&writes), explicit_frame);

    // The bound lives in the argument type, so an out-of-range speed cannot be
    // constructed and therefore cannot reach the wire.
    assert!(MotionSyncSpeed::new(0).is_err());
    assert!(MotionSyncSpeed::new(25).is_err());
    assert!(drain(&writes).is_empty());

    session.shutdown().expect("shutdown");
}

#[test]
fn no_argument_is_moving_samples_every_mechanical_movement_axis() {
    let (session, writes) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    assert!(!camera
        .motion()
        .is_moving()
        .expect("no-argument motion query"));
    let default_frames = drain(&writes);

    assert!(!camera
        .motion()
        .is_moving_axes(MotionQuery::new(AffectedAxes::MOVEMENT))
        .expect("explicit motion query"));
    let explicit_frames = drain(&writes);

    assert_eq!(default_frames, explicit_frames);
    // Two complete snapshots over pan/tilt, zoom, and focus.
    assert_eq!(default_frames.len(), 6);

    session.shutdown().expect("shutdown");
}

#[test]
fn named_idle_wait_presets_poll_only_their_own_axis() {
    let (session, writes) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    camera
        .motion()
        .wait_until_idle(IdleWait::for_zoom().with_interval(Duration::ZERO))
        .expect("zoom idle wait");
    let frames = drain(&writes);
    assert!(!frames.is_empty());
    assert!(
        frames
            .iter()
            .all(|bytes| bytes.as_slice() == [0x81, 0x09, 0x04, 0x47, 0xff]),
        "for_zoom must poll only the zoom position inquiry",
    );

    camera
        .motion()
        .wait_until_idle(IdleWait::from(Duration::from_secs(1)).with_interval(Duration::ZERO))
        .expect("duration idle wait");
    let frames = drain(&writes);
    assert!(frames
        .iter()
        .any(|bytes| bytes.as_slice() == [0x81, 0x09, 0x06, 0x12, 0xff]));
    assert!(frames
        .iter()
        .any(|bytes| bytes.as_slice() == [0x81, 0x09, 0x04, 0x48, 0xff]));

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cameras_expose_their_runtime_capability_inventory() {
    let (session, _writes) = ptz_session();
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

/// The owner correlates every reply by the target address the request encoded,
/// so a broadcast frame cannot travel the per-target command path. 2.0 runs the
/// address-set handshake during serial bring-up instead, which is why no camera
/// noun carries a `trigger_address_assignment` method.
#[test]
fn address_assignment_stays_a_transport_handshake_rather_than_a_camera_command() {
    let (session, writes) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    assert!(matches!(
        camera.execute(&grafton_visca::request::builtin::AddressSet),
        Err(Error::InvalidRequest(_))
    ));
    assert!(
        drain(&writes).is_empty(),
        "the broadcast frame is rejected before any transport write"
    );

    // The 2.0 replacement is `transport::serial::Config::address_set_on_connect`,
    // applied while the transport is still owned by its builder.

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cache_getters_decode_the_combined_flip_pair() {
    let (session, _writes) = ptz_session();
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
    let (session, _writes) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.pan_tilt_limits(), None);

    camera
        .pan_tilt()
        .limit_set(
            PanTiltLimitCorner::UpRight,
            Degrees::new(0.0),
            Degrees::new(0.0),
        )
        .expect("limit set");
    let update = cache.pan_tilt_limits().expect("recorded limit update");
    assert_eq!(update.corner(), PanTiltLimitCorner::UpRight);
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
    let update = cache.pan_tilt_limits().expect("recorded limit clear");
    assert_eq!(update.corner(), PanTiltLimitCorner::DownLeft);
    assert!(update.is_cleared());
    assert_eq!(update.position(), None);

    session.shutdown().expect("shutdown");
}

/// `tally_mode` had no decode-side coverage at all, and it is the one key whose
/// command is reachable only through `execute`: the vendor tally-mode opcode is
/// validated for PTZOptics profiles alone, while the typed `tally()` noun is
/// gated on `HasTally`, which only the two Sony profiles declare. The cache
/// contract is the same either way, so the getter is driven over the profile
/// the command is actually valid for.
#[test]
fn typed_cache_getters_decode_the_vendor_tally_mode() {
    use grafton_visca::command::{TallyFlash, TallyOff, TallyOn};

    let (session, _writes) = ptz_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.tally_mode(), None);

    camera.execute(&TallyOn).expect("tally on");
    assert_eq!(cache.tally_mode(), Some(true));
    camera.execute(&TallyOff).expect("tally off");
    assert_eq!(cache.tally_mode(), Some(false));

    // A flash does not determine the steady-state mode, so the key is
    // invalidated rather than left reporting the last steady value.
    camera.execute(&TallyFlash).expect("tally flash");
    assert_eq!(cache.tally_mode(), None);

    session.shutdown().expect("shutdown");
}

/// `focus_lock` had no decode-side coverage: the projection that stores `1`/`0`
/// was tested, the getter that reads it back was not.
#[test]
fn typed_cache_getters_decode_the_focus_lock_mode() {
    let (session, _writes) = ptz_session();
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

/// The remaining three typed getters without decode-side coverage. All three
/// keys live on the FR7, the only built-in profile that documents digital-zoom
/// toggling, tally brightness, and variable-speed mode together.
#[test]
fn typed_cache_getters_decode_the_remaining_write_only_keys() {
    let (session, _writes) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.digital_zoom(), None);
    assert_eq!(cache.tally_brightness_is_high(), None);
    assert_eq!(cache.variable_speed_mode(), None);

    camera.zoom().set_digital_zoom(true).expect("digital zoom");
    assert_eq!(cache.digital_zoom(), Some(true));
    camera.zoom().set_digital_zoom(false).expect("digital zoom");
    assert_eq!(cache.digital_zoom(), Some(false));

    camera.tally().bright_hi().expect("tally high brightness");
    assert_eq!(cache.tally_brightness_is_high(), Some(true));
    camera.tally().bright_lo().expect("tally low brightness");
    assert_eq!(cache.tally_brightness_is_high(), Some(false));

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
    let (session, _writes) = fr7_session();
    let camera = session.camera::<SonyFR7>().expect("camera");
    let cache = camera.state_cache();

    assert_eq!(cache.spotlight(), None);
    assert_eq!(cache.auto_slow_shutter(), None);

    camera.exposure().spotlight_on().expect("spotlight on");
    assert_eq!(cache.spotlight(), Some(true));
    camera.exposure().spotlight_off().expect("spotlight off");
    assert_eq!(cache.spotlight(), Some(false));

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
    let (session, _writes) = ptz_session();
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

    camera.image().freeze_on().expect("freeze on");
    assert_eq!(cache.image_freeze(), Some(true));
    camera.image().freeze_off().expect("freeze off");
    assert_eq!(cache.image_freeze(), Some(false));

    session.shutdown().expect("shutdown");
}

#[test]
fn typed_cache_getters_decode_the_nd_filter_keys() {
    let (session, _writes) = fr7_session();
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
