use grafton_visca::{capabilities::{InquirySupport, ProfileMetadata}, transport::RawVisca, CommandTimeouts};
use std::time::Duration;
struct Custom;
impl ProfileMetadata for Custom {
    const MODEL_NAME: &'static str = "Custom";
    const DEFAULT_CAMERA_ID: u8 = 1;
    type Envelope = RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_secs(1);
    const COMMAND_TIMEOUTS: CommandTimeouts = CommandTimeouts::DEFAULT;
    const INQUIRY_SUPPORT: InquirySupport = InquirySupport::None;
}
fn main() {}

//~ E0046
//~ "missing: `SUPPORTS_COMMAND_CANCEL`"
