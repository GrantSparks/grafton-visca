use grafton_visca::{
    capabilities::{InquirySupport, ProfileMetadata},
    transport::RawVisca,
    CommandTimeouts,
};
use std::time::Duration;
struct Custom;
impl ProfileMetadata for Custom {
    const MODEL_NAME: &'static str = "Custom";
    const DEFAULT_CAMERA_ID: grafton_visca::IndividualCameraId =
        grafton_visca::IndividualCameraId::new(1).expect("individual profile default");
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_secs(1);
    const COMMAND_TIMEOUTS: CommandTimeouts = CommandTimeouts::DEFAULT;
    const SUPPORTS_COMMAND_CANCEL: bool = false;
}
fn main() {}

//~ E0046
//~ "missing: `INQUIRY_SUPPORT`"
