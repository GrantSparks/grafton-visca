use grafton_visca::{
    __macro_support::InquiryData, __macro_support::InquiryKind, error::Certainty,
    error::FailureStage, transport::BufferConfig, transport::FrameMeta,
    transport::TransportConfig, types::ImageFlipMode, types::MovementTolerance, types::ViscaSocket,
    wire::Response, Error,
};

fn main() {
    let mut config = TransportConfig::default();
    config = config.with_buffer_config(BufferConfig::for_tcp());
    let mut tolerance = MovementTolerance::default();
    tolerance.zoom = 1;
    let _ = (
        config,
        tolerance,
        FrameMeta::Sony {
            sequence: 1,
        },
    );
    let error = Error::invalid_parameter("zoom", "invalid", "outside range");
    match error {
        Error::InvalidParameter {
            parameter,
            value,
            reason,
            ..
        } => {
            assert_eq!(parameter, "zoom");
            assert_eq!(value, "invalid");
            assert_eq!(reason, "outside range");
        }
        _ => panic!("unexpected constructor result"),
    }
    let _ = Error::timeout(FailureStage::Observation, Certainty::StillLive);
    let _ = Error::connection_closed(None);
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
    let _ = match (InquiryData::Power {
        status: grafton_visca::types::PowerStatus::On,
    }) {
        InquiryData::Power { status } => status.is_on(),
        _ => false,
    };
    let _ = ImageFlipMode::Vertical;
}
