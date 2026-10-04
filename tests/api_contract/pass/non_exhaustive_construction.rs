use grafton_visca::{
    camera::MovementTolerance,
    capabilities::ValidationError,
    command::{FlipState, InquiryData, InquiryKind, Response},
    transport::{BufferConfig, FrameMeta, FrameSequence, TransportConfig},
    Certainty, Error, FailureStage, ViscaSocket,
};

fn main() {
    let mut config = TransportConfig::default();
    config.buffer_config = BufferConfig::for_raw_ip();
    let mut tolerance = MovementTolerance::default();
    tolerance.zoom = 1;
    let _ = (config, tolerance, FrameMeta::new(Some(FrameSequence::Full32(1))));
    let error = Error::invalid_parameter("zoom", "invalid", "outside range");
    match error {
        Error::InvalidParameter { parameter, value, reason, .. } => {
            assert_eq!(parameter, "zoom");
            assert_eq!(value, "invalid");
            assert_eq!(reason, "outside range");
        }
        _ => panic!("unexpected constructor result"),
    }
    let _ = Error::timeout(FailureStage::Observation, Certainty::StillLive);
    let _ = Error::connection_closed(None);
    let _ = ValidationError::out_of_range("zoom", 2.0, 0.0, 1.0);
    let _ = ValidationError::invalid_value("zoom", "invalid");
    match Response::cmd_ack(Some(ViscaSocket::S1)) {
        Response::CmdAck { socket, .. } => assert_eq!(socket, Some(ViscaSocket::S1)),
        _ => panic!("unexpected acknowledgment"),
    }
    let _ = Response::completion(None);
    let _ = Response::unknown(None, vec![]);
    #[cfg(feature = "async")]
    {
        use grafton_visca::transport::ReceiveOutcome;
    match ReceiveOutcome::complete(3) {
        ReceiveOutcome::Complete { bytes, .. } => assert_eq!(bytes, 3),
        _ => panic!("unexpected receive outcome"),
    }
    let _ = ReceiveOutcome::truncated(3);
    let _ = ReceiveOutcome::possibly_truncated(3);
    }
    // Registry matches include a wildcard; fixed wire records retain literals.
    let _ = match InquiryKind::Power {
        InquiryKind::Power => true,
        _ => false,
    };
    let _ = match (InquiryData::Power { on: true }) {
        InquiryData::Power { on } => on,
        _ => false,
    };
    let _ = FlipState { horizontal: false, vertical: true };
}
