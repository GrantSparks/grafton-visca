#![cfg(all(feature = "blocking", feature = "dyn-api"))]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    blocking::Session,
    capabilities::Capabilities,
    command::{CommandKind, PowerOn},
    profile::{
        PositionInquirySupport, ProfileEnvelope, ProfileSpec, ProfileTiming, TransportCompatibility,
    },
    transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CommandTimeouts, Error, SessionConfig,
};

#[derive(Clone)]
struct RuntimeProfileCamera {
    replies: VecDeque<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    config: TransportConfig,
}

impl RuntimeProfileCamera {
    fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                replies: VecDeque::new(),
                writes: Arc::clone(&writes),
                config: TransportConfig::default(),
            },
            writes,
        )
    }
}

impl HasTransportConfig for RuntimeProfileCamera {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for RuntimeProfileCamera {
    fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        self.replies.push_back(vec![0x90, 0x41, 0xff]);
        self.replies.push_back(vec![0x90, 0x51, 0xff]);
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
        let Some(reply) = self.replies.pop_front() else {
            return Err(Error::Timeout);
        };
        dst[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }
}

fn runtime_power_profile() -> ProfileSpec {
    let mut capabilities =
        Capabilities::runtime_baseline("Issue 720 runtime camera", 1).expect("baseline");
    capabilities.has_power = true;
    capabilities.power_on_time = Duration::from_secs(1);

    let timing = ProfileTiming::builder()
        .ack_timeout(Duration::from_millis(100))
        .command_timeouts(CommandTimeouts::default())
        .inquiry_timeout(Duration::from_secs(1))
        .cancellation_timeout(Duration::from_secs(1))
        .ambiguity_timeout(Duration::from_secs(1))
        .busy_timeout(Duration::ZERO)
        .raw_inquiry_reply_skew(Duration::ZERO)
        .minimum_inquiry_spacing(Duration::ZERO)
        .minimum_command_spacing(Duration::ZERO)
        .build()
        .expect("timing");

    ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new(Some(5678), None, false))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(timing)
        .maximum_command_sockets(1)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .preset_recall_axes(None)
        .position_inquiries(PositionInquirySupport::new(false, false, false))
        .build()
        .expect("runtime power profile")
}

#[test]
fn blocking_dynamic_view_drives_a_runtime_only_profile() {
    let (transport, writes) = RuntimeProfileCamera::new();
    let session = Session::open(transport, SessionConfig::new(runtime_power_profile()))
        .expect("runtime-profile session");

    let camera = session.camera_dyn().expect("dynamic blocking camera");
    assert_eq!(
        camera.profile().capabilities().model_name,
        "Issue 720 runtime camera"
    );
    camera.execute(&PowerOn::new()).expect("power command");

    assert_eq!(
        *writes.lock().expect("writes lock"),
        vec![vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xff]]
    );
    session.close().expect("close session");
}
