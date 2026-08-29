//! Independent wire-level behavior pins for the public noun registry.
//!
//! These tests intentionally keep their expected frames here instead of
//! deriving them from `noun_table` or the command-surface inventory.  The
//! registry is consumed by three public facades; a row that points at the
//! wrong branch, swaps an on/off or direction pair, or uses the wrong profile
//! conversion must fail against these absolute VISCA frames.

#![cfg(any(feature = "blocking", feature = "runtime-tokio"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

const POWER_ON: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff];
const POWER_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x03, 0xff];

const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xff];
const ZOOM_WIDE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xff];

const FOCUS_FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xff];
const FOCUS_NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, 0xff];

const MENU_SELECT: &[u8] = &[0x81, 0x01, 0x06, 0x06, 0x05, 0xff];
const MENU_CANCEL: &[u8] = &[0x81, 0x01, 0x06, 0x06, 0x04, 0xff];

const TALLY_RED_ON: &[u8] = &[0x81, 0x01, 0x7e, 0x01, 0x0a, 0x00, 0x02, 0xff];
const TALLY_RED_OFF: &[u8] = &[0x81, 0x01, 0x7e, 0x01, 0x0a, 0x00, 0x03, 0xff];
const TALLY_GREEN_ON: &[u8] = &[0x81, 0x01, 0x7e, 0x04, 0x1a, 0x00, 0x02, 0xff];
const TALLY_GREEN_OFF: &[u8] = &[0x81, 0x01, 0x7e, 0x04, 0x1a, 0x00, 0x03, 0xff];
const TALLY_ON: &[u8] = &[0x81, 0x0a, 0x02, 0x02, 0x02, 0xff];
const TALLY_OFF: &[u8] = &[0x81, 0x0a, 0x02, 0x02, 0x03, 0xff];

const PAN_TILT_UP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0a, 0x05, 0x03, 0x01, 0xff];
const PAN_TILT_DOWN: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0a, 0x05, 0x03, 0x02, 0xff];

// Sony FR7 documents optical max 0x4000 and combined optical+digital max
// 0x7000.  At 0.5 these become 0x2000 and 0x3800 respectively, encoded as
// VISCA nibbles in the direct-zoom command.
const ZOOM_NORMALIZED_OPTICAL_HALF: &[u8] = &[0x81, 0x01, 0x04, 0x47, 0x02, 0x00, 0x00, 0x00, 0xff];
const ZOOM_NORMALIZED_COMBINED_HALF: &[u8] =
    &[0x81, 0x01, 0x04, 0x47, 0x03, 0x08, 0x00, 0x00, 0xff];

const POWER_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x00, 0xff];
const MENU_STATUS_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x06, 0xff];

/// Return the raw VISCA payload from either a raw frame or Sony's 8-byte
/// encapsulated frame, together with the sequence number needed for a reply.
fn visca_payload(bytes: &[u8]) -> (Option<u32>, &[u8]) {
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

/// Wrap a raw VISCA reply in the Sony reply envelope when the request used
/// one.  The static and dynamic tests compare the unwrapped payload, keeping
/// the noun assertions about command bytes rather than envelope bookkeeping.
fn envelope(sequence: Option<u32>, payload: Vec<u8>) -> Vec<u8> {
    let Some(sequence) = sequence else {
        return payload;
    };
    let mut frame = Vec::with_capacity(payload.len() + 8);
    frame.extend_from_slice(&[0x01, 0x11]);
    frame.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("reply length")
            .to_be_bytes(),
    );
    frame.extend_from_slice(&sequence.to_be_bytes());
    frame.extend_from_slice(&payload);
    frame
}

#[cfg(feature = "blocking")]
mod blocking_surface {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use grafton_visca::{
        blocking::{Session, SessionConfig},
        camera::TransportKind,
        command::CommandKind,
        profile::ProfileSpec,
        profiles::SonyFR7,
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        types::{PanSpeed, TiltSpeed},
        units::UnitInterval,
        Error, ZoomDomain,
    };

    use super::{
        envelope, visca_payload, FOCUS_FAR, FOCUS_NEAR, MENU_CANCEL, MENU_SELECT,
        MENU_STATUS_INQUIRY, PAN_TILT_DOWN, PAN_TILT_UP, POWER_INQUIRY, POWER_OFF, POWER_ON,
        TALLY_GREEN_OFF, TALLY_GREEN_ON, TALLY_OFF, TALLY_ON, TALLY_RED_OFF, TALLY_RED_ON,
        ZOOM_NORMALIZED_COMBINED_HALF, ZOOM_NORMALIZED_OPTICAL_HALF, ZOOM_TELE, ZOOM_WIDE,
    };

    /// A small in-memory Sony VISCA-over-IP camera.  It records raw VISCA
    /// payloads after removing the transport envelope and returns a valid ACK
    /// plus completion for commands, or a typed response for the two boolean
    /// inquiries used below.
    #[derive(Debug)]
    struct ProbeTransport {
        config: TransportConfig,
        responses: VecDeque<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl ProbeTransport {
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
    }

    impl HasTransportConfig for ProbeTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }

        fn standard_transport_kind(&self) -> Option<TransportKind> {
            None
        }
    }

    impl BlockingTransport for ProbeTransport {
        fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            let (sequence, payload) = visca_payload(bytes);
            self.writes
                .lock()
                .expect("writes lock")
                .push(payload.to_vec());

            let reply = if payload == POWER_INQUIRY {
                vec![0x90, 0x50, 0x02, 0xff]
            } else if payload == MENU_STATUS_INQUIRY {
                vec![0x90, 0x50, 0x03, 0xff]
            } else if payload.get(1) == Some(&0x09) {
                vec![0x90, 0x50, 0x00, 0xff]
            } else {
                vec![0x90, 0x41, 0xff]
            };
            self.responses.push_back(envelope(sequence, reply));
            if payload.get(1) != Some(&0x09) {
                self.responses
                    .push_back(envelope(sequence, vec![0x90, 0x51, 0xff]));
            }
            Ok(())
        }

        fn recv_into(&mut self, destination: &mut [u8]) -> Result<usize, Error> {
            self.recv_into_with_timeout(destination, Duration::from_secs(1))
        }

        fn recv_into_with_timeout(
            &mut self,
            destination: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let response = self.responses.pop_front().ok_or(Error::Timeout)?;
            destination[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    fn open() -> (Session, Arc<Mutex<Vec<Vec<u8>>>>) {
        let (transport, writes) = ProbeTransport::new();
        let session = Session::open(
            transport,
            SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile")),
        )
        .expect("session");
        (session, writes)
    }

    fn one_frame(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<u8> {
        let mut guard = writes.lock().expect("writes lock");
        let mut frames = std::mem::take(&mut *guard);
        assert_eq!(frames.len(), 1, "expected exactly one noun write");
        frames.remove(0)
    }

    #[test]
    fn blocking_power_zoom_focus_and_menu_rows_keep_their_wire_identity() {
        let (session, writes) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.power().on().expect("power on");
        assert_eq!(one_frame(&writes), POWER_ON);
        camera.power().off().expect("power off");
        assert_eq!(one_frame(&writes), POWER_OFF);

        camera
            .zoom()
            .tele()
            .expect("zoom tele")
            .applied()
            .expect("tele applied");
        assert_eq!(one_frame(&writes), ZOOM_TELE);
        camera
            .zoom()
            .wide()
            .expect("zoom wide")
            .applied()
            .expect("wide applied");
        assert_eq!(one_frame(&writes), ZOOM_WIDE);

        camera
            .focus()
            .far()
            .expect("focus far")
            .applied()
            .expect("far applied");
        assert_eq!(one_frame(&writes), FOCUS_FAR);
        camera
            .focus()
            .near()
            .expect("focus near")
            .applied()
            .expect("near applied");
        assert_eq!(one_frame(&writes), FOCUS_NEAR);

        camera.menu().select().expect("menu select");
        assert_eq!(one_frame(&writes), MENU_SELECT);
        camera.menu().cancel().expect("menu cancel");
        assert_eq!(one_frame(&writes), MENU_CANCEL);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_tally_branch_and_on_off_rows_are_distinct() {
        let (session, writes) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.tally().red_on().expect("red tally on");
        assert_eq!(one_frame(&writes), TALLY_RED_ON);
        camera.tally().red_off().expect("red tally off");
        assert_eq!(one_frame(&writes), TALLY_RED_OFF);
        camera.tally().green_on().expect("green tally on");
        assert_eq!(one_frame(&writes), TALLY_GREEN_ON);
        camera.tally().green_off().expect("green tally off");
        assert_eq!(one_frame(&writes), TALLY_GREEN_OFF);
        camera.tally().on().expect("tally mode on");
        assert_eq!(one_frame(&writes), TALLY_ON);
        camera.tally().off().expect("tally mode off");
        assert_eq!(one_frame(&writes), TALLY_OFF);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_pan_tilt_delegates_and_normalized_zoom_use_profile_values() {
        let (session, writes) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");
        let pan_speed = PanSpeed::new(0x0a).expect("pan speed");
        let tilt_speed = TiltSpeed::new(0x05).expect("tilt speed");

        camera
            .pan_tilt()
            .up(pan_speed, tilt_speed)
            .expect("pan/tilt up")
            .applied()
            .expect("up applied");
        assert_eq!(one_frame(&writes), PAN_TILT_UP);
        camera
            .pan_tilt()
            .down(pan_speed, tilt_speed)
            .expect("pan/tilt down")
            .applied()
            .expect("down applied");
        assert_eq!(one_frame(&writes), PAN_TILT_DOWN);

        let half = UnitInterval::new(0.5).expect("unit interval");
        camera
            .zoom()
            .set_normalized(half)
            .expect("optical normalized zoom")
            .applied()
            .expect("optical target applied");
        assert_eq!(one_frame(&writes), ZOOM_NORMALIZED_OPTICAL_HALF);
        camera
            .zoom()
            .set_normalized_in_domain(half, ZoomDomain::OpticalPlusDigital)
            .expect("combined normalized zoom")
            .applied()
            .expect("combined target applied");
        assert_eq!(one_frame(&writes), ZOOM_NORMALIZED_COMBINED_HALF);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_same_response_type_inquiries_use_their_own_wire_queries() {
        let (session, writes) = open();
        let camera = session.camera::<SonyFR7>().expect("camera");

        assert!(camera.power().state().expect("power inquiry"));
        assert_eq!(one_frame(&writes), POWER_INQUIRY);
        assert!(!camera.menu().status().expect("menu inquiry"));
        assert_eq!(one_frame(&writes), MENU_STATUS_INQUIRY);

        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_surface {
    use std::{
        future::Future,
        sync::{Arc, Mutex},
    };

    use grafton_visca::{
        profile::ProfileSpec,
        profiles::SonyFR7,
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        types::{PanSpeed, TiltSpeed},
        units::UnitInterval,
        Error, Result, Session, SessionConfig, TokioRuntime, ZoomDomain,
    };

    #[cfg(feature = "dyn-api")]
    use grafton_visca::dynapi::DynSessionCamera;
    #[cfg(feature = "dyn-api")]
    use grafton_visca::profiles::PtzOpticsG2;

    use super::{
        envelope, visca_payload, FOCUS_FAR, FOCUS_NEAR, MENU_CANCEL, MENU_SELECT,
        MENU_STATUS_INQUIRY, PAN_TILT_DOWN, PAN_TILT_UP, POWER_INQUIRY, POWER_OFF, POWER_ON,
        TALLY_GREEN_OFF, TALLY_GREEN_ON, TALLY_OFF, TALLY_ON, TALLY_RED_OFF, TALLY_RED_ON,
        ZOOM_NORMALIZED_COMBINED_HALF, ZOOM_NORMALIZED_OPTICAL_HALF, ZOOM_TELE, ZOOM_WIDE,
    };

    #[derive(Debug)]
    pub(super) struct ProbeTransport {
        config: TransportConfig,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl ProbeTransport {
        fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let (response_tx, responses) = flume::unbounded();
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig::default(),
                    responses,
                    response_tx,
                    writes: Arc::clone(&writes),
                },
                writes,
            )
        }
    }

    impl HasTransportConfig for ProbeTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for ProbeTransport {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let (sequence, payload) = visca_payload(bytes);
            let payload = payload.to_vec();
            self.writes
                .lock()
                .expect("writes lock")
                .push(payload.clone());
            let response_tx = self.response_tx.clone();
            async move {
                let reply = if payload == POWER_INQUIRY {
                    vec![0x90, 0x50, 0x02, 0xff]
                } else if payload == MENU_STATUS_INQUIRY {
                    vec![0x90, 0x50, 0x03, 0xff]
                } else if payload.get(1) == Some(&0x09) {
                    vec![0x90, 0x50, 0x00, 0xff]
                } else {
                    vec![0x90, 0x41, 0xff]
                };
                response_tx
                    .send_async(envelope(sequence, reply))
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                if payload.get(1) != Some(&0x09) {
                    response_tx
                        .send_async(envelope(sequence, vec![0x90, 0x51, 0xff]))
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                }
                Ok(())
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn recv_into<'a>(
            &'a mut self,
            destination: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            let responses = self.responses.clone();
            async move {
                let response = responses
                    .recv_async()
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                destination[..response.len()].copy_from_slice(&response);
                Ok(response.len())
            }
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Ip)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    async fn open_session(transport: ProbeTransport, profile: ProfileSpec) -> Session {
        Session::open(
            transport,
            SessionConfig::new(profile),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session")
    }

    fn one_frame(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<u8> {
        let mut guard = writes.lock().expect("writes lock");
        let mut frames = std::mem::take(&mut *guard);
        assert_eq!(frames.len(), 1, "expected exactly one noun write");
        frames.remove(0)
    }

    #[tokio::test]
    async fn async_power_zoom_focus_and_menu_rows_keep_their_wire_identity() {
        let (transport, writes) = ProbeTransport::new();
        let session = open_session(
            transport,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.power().on().await.expect("power on");
        assert_eq!(one_frame(&writes), POWER_ON);
        camera.power().off().await.expect("power off");
        assert_eq!(one_frame(&writes), POWER_OFF);

        camera
            .zoom()
            .tele()
            .await
            .expect("zoom tele")
            .applied()
            .await
            .expect("tele applied");
        assert_eq!(one_frame(&writes), ZOOM_TELE);
        camera
            .zoom()
            .wide()
            .await
            .expect("zoom wide")
            .applied()
            .await
            .expect("wide applied");
        assert_eq!(one_frame(&writes), ZOOM_WIDE);

        camera
            .focus()
            .far()
            .await
            .expect("focus far")
            .applied()
            .await
            .expect("far applied");
        assert_eq!(one_frame(&writes), FOCUS_FAR);
        camera
            .focus()
            .near()
            .await
            .expect("focus near")
            .applied()
            .await
            .expect("near applied");
        assert_eq!(one_frame(&writes), FOCUS_NEAR);

        camera.menu().select().await.expect("menu select");
        assert_eq!(one_frame(&writes), MENU_SELECT);
        camera.menu().cancel().await.expect("menu cancel");
        assert_eq!(one_frame(&writes), MENU_CANCEL);

        session.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn async_tally_branch_and_on_off_rows_are_distinct() {
        let (transport, writes) = ProbeTransport::new();
        let session = open_session(
            transport,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.tally().red_on().await.expect("red tally on");
        assert_eq!(one_frame(&writes), TALLY_RED_ON);
        camera.tally().red_off().await.expect("red tally off");
        assert_eq!(one_frame(&writes), TALLY_RED_OFF);
        camera.tally().green_on().await.expect("green tally on");
        assert_eq!(one_frame(&writes), TALLY_GREEN_ON);
        camera.tally().green_off().await.expect("green tally off");
        assert_eq!(one_frame(&writes), TALLY_GREEN_OFF);
        camera.tally().on().await.expect("tally mode on");
        assert_eq!(one_frame(&writes), TALLY_ON);
        camera.tally().off().await.expect("tally mode off");
        assert_eq!(one_frame(&writes), TALLY_OFF);

        session.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn async_pan_tilt_delegates_and_normalized_zoom_use_profile_values() {
        let (transport, writes) = ProbeTransport::new();
        let session = open_session(
            transport,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");
        let pan_speed = PanSpeed::new(0x0a).expect("pan speed");
        let tilt_speed = TiltSpeed::new(0x05).expect("tilt speed");

        camera
            .pan_tilt()
            .up(pan_speed, tilt_speed)
            .await
            .expect("pan/tilt up")
            .applied()
            .await
            .expect("up applied");
        assert_eq!(one_frame(&writes), PAN_TILT_UP);
        camera
            .pan_tilt()
            .down(pan_speed, tilt_speed)
            .await
            .expect("pan/tilt down")
            .applied()
            .await
            .expect("down applied");
        assert_eq!(one_frame(&writes), PAN_TILT_DOWN);

        let half = UnitInterval::new(0.5).expect("unit interval");
        camera
            .zoom()
            .set_normalized(half)
            .await
            .expect("optical normalized zoom")
            .applied()
            .await
            .expect("optical target applied");
        assert_eq!(one_frame(&writes), ZOOM_NORMALIZED_OPTICAL_HALF);
        camera
            .zoom()
            .set_normalized_in_domain(half, ZoomDomain::OpticalPlusDigital)
            .await
            .expect("combined normalized zoom")
            .applied()
            .await
            .expect("combined target applied");
        assert_eq!(one_frame(&writes), ZOOM_NORMALIZED_COMBINED_HALF);

        session.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn async_same_response_type_inquiries_use_their_own_wire_queries() {
        let (transport, writes) = ProbeTransport::new();
        let session = open_session(
            transport,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        assert!(camera.power().state().await.expect("power inquiry"));
        assert_eq!(one_frame(&writes), POWER_INQUIRY);
        assert!(!camera.menu().status().await.expect("menu inquiry"));
        assert_eq!(one_frame(&writes), MENU_STATUS_INQUIRY);

        session.shutdown().await.expect("shutdown");
    }

    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_unsupported_capability_fails_before_any_write() {
        let (transport, writes) = ProbeTransport::new();
        let session = open_session(
            transport,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"),
        )
        .await;
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
        let error = camera
            .zoom()
            .set_digital_zoom(true)
            .await
            .expect_err("G2 has no documented digital zoom toggle");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "digital zoom"
            }
        ));
        assert!(writes.lock().expect("writes lock").is_empty());
        session.shutdown().await.expect("shutdown");
    }

    /// Issue #684: the erased tally noun is coherent on PtzOpticsG2 — which has
    /// no typed tally support — so `tally().on()/.off()/.flash()` are refused
    /// exactly like `tally().red_on()`, instead of the tally-mode opcodes
    /// slipping through while the red row was refused.
    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_tally_noun_refuses_as_one_on_a_profile_without_tally() {
        let (transport, writes) = ProbeTransport::new();
        let session = open_session(
            transport,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"),
        )
        .await;
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");

        // The tally-mode opcodes are refused, not admitted through a vendor
        // fallback: the whole noun shares one `HasTally` gate.
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
            writes.lock().expect("writes lock").is_empty(),
            "refused tally rows must not reach the wire",
        );
        session.shutdown().await.expect("shutdown");
    }
}
