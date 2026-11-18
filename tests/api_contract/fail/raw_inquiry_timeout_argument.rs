use grafton_visca::{
    profiles::CommandTimeoutClass, raw, request::InquiryRetryClass, request::ResponseDecoder,
};

fn main() {
    let _ = raw::InquiryRequest::new(
    //~^ E0061 "this function takes 4 arguments but 5 arguments were supplied"
        [0x81, 0x09, 0x04, 0x00, 0xff],
        ResponseDecoder::from_fn(|_: &[u8]| Ok::<_, grafton_visca::Error>(0_u8)),
        CommandTimeoutClass::Quick,
        InquiryRetryClass::Inquiry,
        grafton_visca::request::SubmissionClass::Normal,
    );
}
