//! Private, read-only hardware evidence collection from the generated inquiry inventory.
//!
//! No probe runs during ordinary tests. To select any built-in profile:
//! ```sh
//! VISCA_CAMERA_IP=10.0.0.30 VISCA_PROFILE=generic_visca \
//!   cargo test --features serde --lib profile_probe::profile_probe -- --ignored --exact --nocapture
//! ```
//! Use `VISCA_PROFILE_SPEC=acme_cam.json` instead of `VISCA_PROFILE` for a
//! custom profile. `VISCA_TRANSPORT=tcp|udp` overrides the first supported
//! network transport; its port comes from the profile unless the address
//! includes one. For a serial-only profile use `VISCA_SERIAL_PORT=/dev/ttyUSB0`
//! instead of `VISCA_CAMERA_IP`, optionally set `VISCA_BAUD_RATE` (default 9600),
//! and enable `serde,transport-serial`. Serial startup remains silent.
//! `VISCA_PROBE_LABEL` optionally selects the report filename.
//!
//! Successful inquiries are candidate evidence, requiring review before any
//! grant is added. A timeout is inconclusive. No controls, setup commands,
//! grants, profile data or encoding selections are changed by this tool.

#![allow(clippy::expect_used, clippy::panic)]

use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    blocking::{Camera, CameraSession},
    command::{
        inquiry_structs::{
            BuiltinInquiryMetadata, InquiryKind, ProbePreparedInquiry, ProbeReply,
            BUILTIN_INQUIRIES,
        },
        nouns::NounInquiryContract,
    },
    config::{CameraConfig, TransportOptions},
    profiles::{AnyProfile, ProfileId, ProfileSelection, ProfileSpec},
    source_evidence::Source,
    Error, TargetRegistration,
};

type ProbeResult<T> = Result<T, String>;

#[derive(Debug)]
struct ProbeSettings {
    selection: ProfileSelection,
    profile: ProfileSpec,
    config: CameraConfig<AnyProfile>,
    label: String,
    transport: ProbeTransport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeTransport {
    kind: String,
    endpoint: String,
    envelope: String,
    baud_rate: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeError {
    stage: String,
    kind: String,
    message: String,
}

impl ProbeError {
    fn new(stage: &str, error: &Error) -> Self {
        Self {
            stage: stage.into(),
            kind: format!("{:?}", error.kind()),
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeRecord {
    inquiry: String,
    surface: String,
    surfaces: Vec<String>,
    argument: Option<u8>,
    request_hex: Option<String>,
    reply_hex: Option<String>,
    decoded: Option<String>,
    error: Option<ProbeError>,
    latency_micros: u64,
    profile_refusal: Option<ProbeError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeReport {
    schema_version: u8,
    library_version: String,
    started_at_unix_seconds: u64,
    label: String,
    selection: ProfileSelection,
    profile: ProfileSpec,
    transport: ProbeTransport,
    request_format: String,
    reply_format: String,
    candidate_observations_only: bool,
    candidate_observation_policy: String,
    version: Option<ProbeRecord>,
    inquiries: Vec<ProbeRecord>,
    candidate_surfaces: Vec<String>,
    close_error: Option<ProbeError>,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(bytes.len().saturating_mul(3));
    for (index, byte) in bytes.iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        write!(output, "{byte:02X}").expect("writing to a String is infallible");
    }
    output
}

fn profile_selection(
    id: Option<&str>,
    spec_file: Option<&str>,
    read: impl FnOnce(&Path) -> std::io::Result<Vec<u8>>,
) -> ProbeResult<ProfileSelection> {
    match (id, spec_file) {
        (Some(_), Some(_)) => {
            Err("set either VISCA_PROFILE or VISCA_PROFILE_SPEC, not both".into())
        }
        (None, None) => Err("set VISCA_PROFILE or VISCA_PROFILE_SPEC explicitly".into()),
        (Some(id), None) => serde_json::from_value::<ProfileId>(id.into())
            .map(ProfileSelection::Builtin)
            .map_err(|error| format!("invalid VISCA_PROFILE `{id}`: {error}")),
        (None, Some(path)) => {
            let bytes = read(Path::new(path))
                .map_err(|error| format!("cannot read VISCA_PROFILE_SPEC `{path}`: {error}"))?;
            serde_json::from_slice::<ProfileSpec>(&bytes)
                .map(ProfileSelection::from)
                .map_err(|error| format!("invalid VISCA_PROFILE_SPEC `{path}`: {error}"))
        }
    }
}

fn report_label(label: &str) -> ProbeResult<String> {
    if label.is_empty()
        || label.len() > 120
        || matches!(label, "." | "..")
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err("VISCA_PROBE_LABEL must be 1..=120 ASCII letters, digits, '.', '_' or '-' and cannot be '.' or '..'".into());
    }
    Ok(label.into())
}

impl ProbeSettings {
    fn new(
        address: &str,
        selection: ProfileSelection,
        requested_transport: Option<&str>,
        baud_rate: Option<u32>,
        label: Option<&str>,
    ) -> ProbeResult<Self> {
        if address.trim().is_empty() {
            return Err("the probe endpoint must be nonempty".into());
        }
        let profile = selection.spec().map_err(|error| error.to_string())?;
        let kind = match requested_transport {
            Some("tcp") => "tcp",
            Some("udp") => "udp",
            Some("serial") => "serial",
            Some(other) => {
                return Err(format!(
                    "invalid VISCA_TRANSPORT `{other}`; use tcp, udp or serial"
                ))
            }
            None if profile
                .transports()
                .supports(crate::transport::StandardTransport::Tcp) =>
            {
                "tcp"
            }
            None if profile
                .transports()
                .supports(crate::transport::StandardTransport::Udp) =>
            {
                "udp"
            }
            None => {
                return Err(
                    "the selected profile has no network transport; use VISCA_SERIAL_PORT".into(),
                )
            }
        };
        if kind != "serial" && baud_rate.is_some() {
            return Err("VISCA_BAUD_RATE requires VISCA_SERIAL_PORT".into());
        }
        let baud_rate = (kind == "serial").then(|| {
            baud_rate.unwrap_or_else(|| crate::transport::SerialConfig::default().baud_rate())
        });
        let options = match kind {
            "tcp" => TransportOptions::tcp(address),
            "udp" => TransportOptions::udp(address),
            _ => TransportOptions::serial(address, baud_rate.expect("serial baud rate")),
        };
        let config = CameraConfig::new(selection.clone(), options);
        // Ordinary pure configuration checks own profile, endpoint and transport
        // validation. SerialStartup::default performs no address setup or clear.
        let endpoint = config
            .connection_plan()
            .map_err(|error| {
                let context = if baud_rate.is_some() {
                    "invalid probe serial configuration"
                } else {
                    "invalid probe connection"
                };
                format!("{context}: {error}")
            })?
            .connection
            .transport
            .endpoint()
            .to_owned();
        let default_label = match &selection {
            ProfileSelection::Builtin(id) => serde_json::to_value(id)
                .map_err(|error| error.to_string())?
                .as_str()
                .ok_or_else(|| "profile identifier did not serialize as a string".to_string())?
                .to_owned(),
            ProfileSelection::Custom(_) => "custom_profile".into(),
        };
        Ok(Self {
            selection,
            label: report_label(label.unwrap_or(&default_label))?,
            transport: ProbeTransport {
                kind: kind.into(),
                endpoint,
                envelope: format!("{:?}", profile.envelope()),
                baud_rate,
            },
            profile,
            config,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ProbeInput {
    address: String,
    transport: Option<String>,
    baud_rate: Option<u32>,
}

fn connection_input(
    camera_ip: Option<&str>,
    serial_port: Option<&str>,
    transport: Option<&str>,
    baud_rate: Option<&str>,
) -> ProbeResult<ProbeInput> {
    match (camera_ip, serial_port) {
        (Some(_), Some(_)) => {
            Err("set either VISCA_CAMERA_IP or VISCA_SERIAL_PORT, not both".into())
        }
        (None, None) => Err("set VISCA_CAMERA_IP or VISCA_SERIAL_PORT explicitly".into()),
        (Some(address), None) => {
            if transport == Some("serial") || baud_rate.is_some() {
                return Err(
                    "serial transport and VISCA_BAUD_RATE require VISCA_SERIAL_PORT".into(),
                );
            }
            Ok(ProbeInput {
                address: address.into(),
                transport: transport.map(str::to_owned),
                baud_rate: None,
            })
        }
        (None, Some(port)) => {
            if transport.is_some_and(|kind| kind != "serial") {
                return Err("VISCA_SERIAL_PORT cannot select a network transport".into());
            }
            let baud_rate = baud_rate
                .map(|value| {
                    value
                        .parse::<u32>()
                        .map_err(|error| format!("invalid VISCA_BAUD_RATE `{value}`: {error}"))
                })
                .transpose()?;
            if baud_rate == Some(0) {
                return Err("VISCA_BAUD_RATE must be greater than zero".into());
            }
            Ok(ProbeInput {
                address: port.into(),
                transport: Some("serial".into()),
                baud_rate,
            })
        }
    }
}

fn validate_transport_feature(input: &ProbeInput) -> ProbeResult<()> {
    if input.transport.as_deref() == Some("serial") && !cfg!(feature = "transport-serial") {
        return Err("the serial probe requires --features serde,transport-serial".into());
    }
    Ok(())
}

// Recover query provenance from a constructor's type without calling it. This
// lets the noun table supply canonical surface names, including mapped queries,
// without another parser or inventory of requests or their arguments.
fn inquiry_contract<Q, Arguments, Constructor>(_constructor: Constructor) -> (InquiryKind, Source)
where
    Q: NounInquiryContract,
    Constructor: FnOnce(Arguments) -> crate::Result<Q>,
{
    (Q::KIND, Q::SOURCE)
}

macro_rules! probe_surface {
    (inquiry, $target:ident, $metadata:ident, $getter:ident, $method:ident,
        [$($(#[$coercion:ident])? $argument:ident: $ty:ty),*], [$($request:tt)*]) => {{
        let (kind, source) = inquiry_contract(|($($argument,)*): ($($ty,)*)| {
            crate::noun_facade::noun_inquiry_request!(
                $target.profile(), $target; $($request)*
            )
        });
        (kind == $metadata.kind && source == $metadata.source)
            .then(|| concat!(stringify!($getter), ".", stringify!($method)).to_string())
    }};
    ($kind:ident, $($rest:tt)*) => { None::<String> };
}

macro_rules! probe_surfaces {
    (
        $(
            @noun $noun:ident {
                accessor: $accessor:ident,
                getter: $getter:ident,
                gate: $base:tt,
                doc: $doc:literal $(,)?
            };
            $(
                $(#[doc = $row_doc:literal])*
                $kind:ident [$($command:ident)?] $method:ident($($(#[$coercion:ident])? $argument:ident: $ty:ty),*) -> $ret:ty
                    $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        fn surfaces(target: &TargetRegistration, metadata: &BuiltinInquiryMetadata) -> Vec<String> {
            use crate::{command, types};
            [$( $( probe_surface!($kind, target, metadata, $getter, $method,
                [$($argument: $ty),*], [$($request)*]), )* )*]
                .into_iter()
                .flatten()
                .collect()
        }
    };
}

crate::noun_table::noun_table!(probe_surfaces);

fn collect_report<'a>(
    settings: &ProbeSettings,
    target: &TargetRegistration,
    inventory: impl IntoIterator<Item = &'a BuiltinInquiryMetadata>,
    mut inquire: impl FnMut(&ProbePreparedInquiry) -> crate::Result<ProbeReply>,
) -> ProbeReport {
    let mut report = ProbeReport {
        schema_version: 1,
        library_version: env!("CARGO_PKG_VERSION").into(),
        started_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        label: settings.label.clone(),
        selection: settings.selection.clone(),
        profile: settings.profile.clone(),
        transport: settings.transport.clone(),
        request_format: "complete VISCA frame; transport envelope not captured".into(),
        reply_format: "VISCA data payload only; address, reply header, terminator and transport envelope not captured".into(),
        candidate_observations_only: true,
        candidate_observation_policy: "Decoder acceptance does not identify register semantics or imply writable controls. Alternate interpretations of the same bytes require independent semantic evidence before any grant is added.".into(),
        version: None,
        inquiries: Vec::new(),
        candidate_surfaces: Vec::new(),
        close_error: None,
    };
    let mut candidate_surfaces = BTreeSet::new();
    for metadata in inventory {
        let surfaces = surfaces(target, metadata);
        let base = ProbeRecord {
            inquiry: metadata.name.into(),
            surface: surfaces
                .first()
                .cloned()
                .unwrap_or_else(|| format!("private.{}", metadata.name)),
            surfaces,
            argument: None,
            request_hex: None,
            reply_hex: None,
            decoded: None,
            error: None,
            latency_micros: 0,
            profile_refusal: (metadata.validate)(target.profile())
                .err()
                .map(|error| ProbeError::new("typed_admission", &error)),
        };
        let prepared = match (metadata.prepare_probe)(target).and_then(|prepared| {
            if prepared.is_empty() {
                Err(Error::InvalidRequest {
                    reason: "the generated inquiry has no checked probe samples".into(),
                })
            } else {
                Ok(prepared)
            }
        }) {
            Ok(prepared) => prepared,
            Err(error) => {
                report.inquiries.push(ProbeRecord {
                    error: Some(ProbeError::new("preparation", &error)),
                    ..base
                });
                continue;
            }
        };
        for inquiry in prepared {
            let mut record = ProbeRecord {
                argument: inquiry.argument,
                request_hex: Some(hex(&inquiry.request)),
                ..base.clone()
            };
            let start = Instant::now();
            match inquire(&inquiry) {
                Ok(reply) => {
                    record.reply_hex = Some(hex(&reply.payload));
                    match reply.decoded {
                        Ok(decoded) => {
                            record.decoded = Some(decoded);
                            candidate_surfaces.extend(record.surfaces.iter().cloned());
                        }
                        Err(error) => record.error = Some(ProbeError::new("decode", &error)),
                    }
                }
                Err(error) => record.error = Some(ProbeError::new("request", &error)),
            }
            record.latency_micros = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
            if metadata.kind == InquiryKind::Version {
                report.version = Some(record.clone());
            }
            report.inquiries.push(record);
        }
    }
    report.candidate_surfaces = candidate_surfaces.into_iter().collect();
    report
}

fn run_probe(settings: &ProbeSettings, camera: &Camera<AnyProfile>) -> ProbeReport {
    collect_report(
        settings,
        camera.registration(),
        BUILTIN_INQUIRIES,
        |inquiry| camera.inquire(&inquiry.inquiry),
    )
}

fn optional_env(name: &str) -> ProbeResult<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(format!("invalid {name}: {error}")),
    }
}

fn report_path(label: &str) -> ProbeResult<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/profile-probe")
        .join(format!("{}.json", report_label(label)?)))
}

#[test]
#[ignore = "manual read-only hardware evidence; requires an explicit endpoint and runtime profile"]
fn profile_probe() -> Result<(), Box<dyn std::error::Error>> {
    if env::var_os("CI").is_some() {
        return Err("manual hardware profile probing is disabled under CI".into());
    }
    let input = connection_input(
        optional_env("VISCA_CAMERA_IP")?.as_deref(),
        optional_env("VISCA_SERIAL_PORT")?.as_deref(),
        optional_env("VISCA_TRANSPORT")?.as_deref(),
        optional_env("VISCA_BAUD_RATE")?.as_deref(),
    )?;
    validate_transport_feature(&input)?;
    let selection = profile_selection(
        optional_env("VISCA_PROFILE")?.as_deref(),
        optional_env("VISCA_PROFILE_SPEC")?.as_deref(),
        |path| fs::read(path),
    )?;
    let settings = ProbeSettings::new(
        &input.address,
        selection,
        input.transport.as_deref(),
        input.baud_rate,
        optional_env("VISCA_PROBE_LABEL")?.as_deref(),
    )?;
    let session = CameraSession::connect(&settings.config)?;
    let mut report = run_probe(&settings, session.camera());
    report.close_error = session
        .close()
        .err()
        .map(|error| ProbeError::new("close", &error));
    let path = report_path(&settings.label)?;
    fs::create_dir_all(path.parent().ok_or("report directory is absent")?)?;
    fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    println!("Read-only inquiry evidence: {}", path.display());
    println!(
        "Candidate surfaces: {}",
        report.candidate_surfaces.join(", ")
    );
    println!(
        "{} Timeouts are inconclusive.",
        report.candidate_observation_policy
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        blocking::Session,
        config::SessionConfig,
        transport::{
            AddressingMode, BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics,
            TransportConfig, TransportIdentity,
        },
    };
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };

    // A bounded script independent of the optional test-utils feature, so the
    // documented blocking + serde invocation compiles and runs its schema proof.
    struct ProbeScriptedTransport {
        replies: VecDeque<Vec<u8>>,
        ready: flume::Receiver<Vec<u8>>,
        deliver: flume::Sender<Vec<u8>>,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
        config: TransportConfig,
    }

    impl ProbeScriptedTransport {
        fn new(replies: impl IntoIterator<Item = Vec<u8>>) -> Self {
            let (deliver, ready) = flume::bounded(1);
            Self {
                replies: replies.into_iter().collect(),
                ready,
                deliver,
                sent: Arc::new(Mutex::new(Vec::new())),
                config: TransportConfig::for_udp(),
            }
        }
    }

    impl HasTransportConfig for ProbeScriptedTransport {
        fn identity(&self) -> TransportIdentity<'_> {
            TransportIdentity::Custom {
                semantics: SendSemantics::Datagram,
                addressing: AddressingMode::Ip,
            }
        }

        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for ProbeScriptedTransport {
        fn send_with_timeout(&mut self, bytes: &[u8], _timeout: Duration) -> crate::Result<()> {
            self.sent.lock().expect("sent frames").push(bytes.to_vec());
            let reply = self
                .replies
                .pop_front()
                .ok_or_else(|| Error::InvalidRequest {
                    reason: "unexpected request after the bounded probe script".into(),
                })?;
            self.deliver
                .try_send(reply)
                .map_err(|error| Error::InvalidRequest {
                    reason: format!("bounded probe reply queue: {error}").into(),
                })
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            timeout: Duration,
        ) -> crate::Result<ReceiveOutcome> {
            self.ready
                .recv_timeout(timeout)
                .map(|reply| ReceiveOutcome::copy_message(&reply, dst))
                .map_err(|_| Error::io_timeout())
        }
    }

    fn metadata(kind: InquiryKind) -> &'static BuiltinInquiryMetadata {
        BUILTIN_INQUIRIES
            .iter()
            .find(|entry| entry.kind == kind)
            .expect("generated inquiry")
    }

    #[test]
    fn report_schema_captures_canonical_decoding_and_raw_discovery() {
        let settings = ProbeSettings::new(
            "127.0.0.1:5678",
            ProfileId::GenericVisca.into(),
            Some("tcp"),
            None,
            Some("scripted"),
        )
        .expect("settings");
        let transport = ProbeScriptedTransport::new([
            vec![0x90, 0x50, 0x02, 0xff],
            vec![0x90, 0x50, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x02, 0xff],
            vec![0x90, 0x50, 0x07, 0xff],
            vec![0x90, 0x50, 0x02, 0xff],
        ]);
        let sent = Arc::clone(&transport.sent);
        let session = Session::open(transport, &SessionConfig::new(settings.profile.clone()))
            .expect("session");
        let camera = session
            .camera::<AnyProfile>(crate::IndividualCameraId::CAMERA_1)
            .expect("camera");
        let inventory = [
            metadata(InquiryKind::Power),
            metadata(InquiryKind::Version),
            metadata(InquiryKind::FocusMode),
            metadata(InquiryKind::Fr7AutoGain),
        ];
        let profile_before = camera.profile().clone();
        let report = collect_report(&settings, camera.registration(), inventory, |query| {
            camera.inquire(&query.inquiry)
        });
        session.close().expect("close");
        assert_eq!(report.inquiries.len(), 4);
        assert!(report.inquiries[0].decoded.is_some());
        assert_eq!(report.inquiries[0].reply_hex.as_deref(), Some("02"));
        assert_eq!(
            report.inquiries[0].request_hex.as_deref(),
            Some("81 09 04 00 FF")
        );
        assert!(report
            .version
            .as_ref()
            .expect("version observation")
            .decoded
            .is_some());
        assert_eq!(
            report.inquiries[2]
                .error
                .as_ref()
                .expect("canonical failure")
                .stage,
            "decode"
        );
        assert_eq!(report.inquiries[2].reply_hex.as_deref(), Some("07"));
        assert!(
            report.inquiries[3].profile_refusal.is_some(),
            "normal typed admission is denied"
        );
        assert!(
            report.inquiries[3].decoded.is_some(),
            "discovery has no grant/source prefilter"
        );
        assert_eq!(
            &profile_before,
            camera.profile(),
            "the probe cannot mutate grants"
        );
        let sent = sent.lock().expect("sent frames");
        assert_eq!(sent.len(), 4);
        assert!(sent.iter().all(|frame| frame.get(1) == Some(&0x09)));
        assert!(report.candidate_observations_only);
        assert!(report.reply_format.contains("payload only"));
        let serialized = serde_json::to_value(&report).expect("report JSON");
        assert_eq!(serialized["schema_version"], 1);
        assert_eq!(serialized["transport"]["kind"], "tcp");
        assert_eq!(
            serde_json::from_value::<ProbeReport>(serialized).expect("schema round trip"),
            report
        );
    }

    #[test]
    fn generated_probe_inventory_is_read_only_for_every_builtin_profile() {
        for id in ProfileId::all() {
            let profile = id.spec().expect("built-in profile");
            let target =
                TargetRegistration::new(profile.capabilities().default_camera_id.into(), profile)
                    .expect("target");
            let mut prepared = 0;
            for entry in BUILTIN_INQUIRIES {
                let queries = (entry.prepare_probe)(&target)
                    .unwrap_or_else(|error| panic!("{} on {id:?}: {error}", entry.name));
                assert!(!queries.is_empty(), "{} on {id:?}", entry.name);
                for query in queries {
                    fn requires_inquiry<Q: crate::Inquiry>(_: &Q) {}
                    requires_inquiry(&query.inquiry);
                    let mut expected = vec![0; query.request.len()];
                    let frame = crate::command::bytes::FrameWriter::new(
                        target.target().into(),
                        &mut expected,
                    )
                    .bytes(crate::command::inquiry_structs::body_for_profile(
                        entry.kind,
                        entry.bytes,
                        target.profile(),
                    ));
                    let written = match query.argument {
                        Some(argument) => frame.byte(argument),
                        None => frame,
                    }
                    .finish()
                    .expect("canonical inquiry frame");
                    expected.truncate(written);
                    assert_eq!(query.request, expected, "{} on {id:?}", entry.name);
                    assert_eq!(query.request.last(), Some(&0xff));
                    assert_eq!(query.inquiry.bytes(), query.request);
                    prepared += 1;
                }
            }
            assert!(prepared >= BUILTIN_INQUIRIES.len());
        }
    }

    #[test]
    fn argument_samples_use_profile_bounds_and_all_closed_selectors() {
        for id in ProfileId::all() {
            let profile = id.spec().expect("profile");
            let target =
                TargetRegistration::new(profile.capabilities().default_camera_id.into(), profile)
                    .expect("target");
            let presets = target
                .profile()
                .capabilities()
                .presets
                .as_ref()
                .expect("presets");
            let queries = (metadata(InquiryKind::Fr7SeparatePresetMode).prepare_probe)(&target)
                .expect("checked argument samples");
            assert_eq!(queries.first().expect("minimum").argument, Some(0));
            assert_eq!(
                queries.last().expect("maximum").argument,
                Some(presets.highest)
            );
            let knee = (metadata(InquiryKind::Fr7KneePermission).prepare_probe)(&target)
                .expect("closed selector samples");
            assert_eq!(
                knee.iter()
                    .filter_map(|query| query.argument)
                    .collect::<Vec<_>>(),
                [0, 1, 2, 3]
            );
        }
    }

    #[test]
    fn sourced_vendor_inquiries_keep_their_read_only_frame() {
        let target = TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            ProfileId::PtzOpticsG2.spec().expect("profile"),
        )
        .expect("target");
        let query = (metadata(InquiryKind::UsbAudio).prepare_probe)(&target).expect("UAC inquiry");
        // R1 §7.10: the inquiry omits the setting byte of the command.
        assert_eq!(query[0].request, [0x81, 0x2a, 0x02, 0xa0, 0x04, 0xff]);
    }

    #[test]
    fn probe_uses_the_profile_selected_canonical_pan_tilt_decoder() {
        for (id, payload_len) in [(ProfileId::GenericVisca, 8), (ProfileId::SonyBrc300, 9)] {
            let target = TargetRegistration::new(
                crate::IndividualCameraId::CAMERA_1,
                id.spec().expect("profile"),
            )
            .expect("target");
            let queries = (metadata(InquiryKind::PanTiltPosition).prepare_probe)(&target)
                .expect("generated position query");
            let decoder = crate::Inquiry::decoder(&queries[0].inquiry);
            let reply = decoder
                .decode(&vec![0; payload_len])
                .expect("captured payload");
            assert!(reply.decoded.is_ok(), "profile-selected grammar for {id:?}");
            let wrong = decoder
                .decode(&vec![0; payload_len + 1])
                .expect("captured malformed payload");
            assert!(
                wrong.decoded.is_err(),
                "canonical framing rejection for {id:?}"
            );
            assert_eq!(wrong.payload.len(), payload_len + 1);
        }
    }

    #[test]
    fn failed_requests_are_reported_without_fabricating_reply_bytes_or_candidates() {
        let settings = ProbeSettings::new(
            "127.0.0.1",
            ProfileId::GenericVisca.into(),
            None,
            None,
            None,
        )
        .expect("settings");
        let target = TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            settings.profile.clone(),
        )
        .expect("target");
        let report = collect_report(&settings, &target, [metadata(InquiryKind::Power)], |_| {
            Err(Error::RuntimeShutdown)
        });
        let record = &report.inquiries[0];
        assert!(record.request_hex.is_some());
        assert!(record.reply_hex.is_none());
        assert!(record.decoded.is_none());
        assert_eq!(
            record.error.as_ref().expect("request error").stage,
            "request"
        );
        assert!(report.candidate_surfaces.is_empty());

        let unpreparable = BuiltinInquiryMetadata {
            prepare_probe: |_| Ok(Vec::new()),
            ..*metadata(InquiryKind::Power)
        };
        let report = collect_report(&settings, &target, [&unpreparable], |_| {
            panic!("an unpreparable inquiry must never reach the transport")
        });
        assert_eq!(
            report.inquiries.len(),
            1,
            "a declared inquiry is never omitted"
        );
        assert_eq!(
            report.inquiries[0]
                .error
                .as_ref()
                .expect("preparation error")
                .stage,
            "preparation"
        );
        assert!(report.inquiries[0].request_hex.is_none());
        assert!(report.inquiries[0].reply_hex.is_none());
    }

    #[test]
    fn malformed_environment_and_profile_json_fail_before_any_connection() {
        let no_file = |_: &Path| Err(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(profile_selection(None, None, no_file).is_err());
        assert!(profile_selection(Some("g2"), None, no_file).is_err());
        assert!(profile_selection(Some("generic_visca"), Some("camera.json"), no_file).is_err());
        assert!(profile_selection(None, Some("missing.json"), no_file)
            .expect_err("read error")
            .contains("cannot read"));
        assert!(
            profile_selection(None, Some("bad.json"), |_| Ok(b"{} trailing".to_vec()))
                .expect_err("invalid JSON")
                .contains("invalid VISCA_PROFILE_SPEC")
        );
        for id in ProfileId::all() {
            let value = serde_json::to_value(id).expect("id JSON");
            let selected = profile_selection(value.as_str(), None, no_file).expect("runtime id");
            let spec = id.spec().expect("same spec");
            assert_eq!(selected.spec().expect("spec"), spec);
            let settings = if spec
                .transports()
                .supports(crate::transport::StandardTransport::Tcp)
                || spec
                    .transports()
                    .supports(crate::transport::StandardTransport::Udp)
            {
                ProbeSettings::new("127.0.0.1", selected, None, None, None)
            } else {
                ProbeSettings::new("/dev/not-opened", selected, Some("serial"), None, None)
            }
            .expect("transport plan for each built-in");
            assert!(!settings.config.serial_startup().address_set());
            assert!(!settings.config.serial_startup().interface_clear());
            assert!(!settings
                .config
                .validated_session_config()
                .expect("validated config")
                .sony_sequence_reset_on_connect());
            let mut json = serde_json::to_value(&spec).expect("spec JSON");
            let bytes = serde_json::to_vec(&json).expect("encoded spec");
            let custom = profile_selection(None, Some("camera.json"), |_| Ok(bytes))
                .expect("validated runtime custom profile");
            assert_eq!(custom.spec().expect("custom spec"), spec);
            json["unknown_probe_fact"] = true.into();
            assert!(
                profile_selection(None, Some("camera.json"), |_| {
                    Ok(serde_json::to_vec(&json).expect("unknown field JSON"))
                })
                .is_err(),
                "unknown spec fields are rejected"
            );
        }
        for label in ["", ".", "..", "../camera", "camera/name", "camera name"] {
            assert!(report_path(label).is_err());
        }
        assert!(ProbeSettings::new("", ProfileId::GenericVisca.into(), None, None, None).is_err());
        assert!(ProbeSettings::new(
            "127.0.0.1",
            ProfileId::GenericVisca.into(),
            Some("invalid"),
            None,
            None
        )
        .is_err());
    }

    #[test]
    fn serial_input_is_explicit_strict_and_keeps_startup_silent() {
        assert!(connection_input(None, None, None, None).is_err());
        assert!(connection_input(Some("127.0.0.1"), Some("/dev/fake"), None, None).is_err());
        assert!(connection_input(Some("127.0.0.1"), None, Some("serial"), None).is_err());
        assert!(connection_input(Some("127.0.0.1"), None, None, Some("9600")).is_err());
        assert!(connection_input(None, Some("/dev/fake"), Some("tcp"), None).is_err());
        for invalid in ["", "0", "-1", "9600 ", "not-a-number", "4294967296"] {
            assert!(connection_input(None, Some("/dev/fake"), None, Some(invalid)).is_err());
        }
        for requested in [None, Some("38400")] {
            let input = connection_input(None, Some("/dev/not-opened"), None, requested)
                .expect("explicit serial selection");
            assert_eq!(
                validate_transport_feature(&input).is_ok(),
                cfg!(feature = "transport-serial")
            );
            let settings = ProbeSettings::new(
                &input.address,
                ProfileId::SonyEviH100.into(),
                input.transport.as_deref(),
                input.baud_rate,
                None,
            )
            .expect("pure serial config");
            let expected_baud = requested.map_or(9600, |_| 38_400);
            assert_eq!(settings.transport.kind, "serial");
            assert_eq!(settings.transport.baud_rate, Some(expected_baud));
            let serial = settings
                .config
                .connection_plan()
                .map(|plan| plan.connection.transport.into_serial())
                .expect("validated serial config");
            assert!(!serial.startup().address_set());
            assert!(!serial.startup().interface_clear());
        }
    }
}
