use grafton_visca::{
    raw::InquiryRequest,
    request::{ResponseDecoder, CommandRetryClass, SubmissionClass},
};

fn main() {
    let _ = InquiryRequest::new(
        [0x81, 0x09, 0x04, 0x00, 0xff],
        ResponseDecoder::from_fn(|_: &[u8]| Ok::<_, grafton_visca::Error>(0_u8)),
        CommandRetryClass::Movement,
        SubmissionClass::Normal,
    );
}

//~ E0308
//~ "expected `InquiryRetryClass`, found `CommandRetryClass`"
