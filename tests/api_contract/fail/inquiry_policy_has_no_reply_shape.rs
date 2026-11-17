use grafton_visca::request::{InquiryPolicy, InquiryRetryClass, SubmissionClass};

fn main() {
    let _ = InquiryPolicy::new(InquiryRetryClass::Inquiry, SubmissionClass::Normal).with_reply_shape(grafton_visca::raw::RawReplyShape::NoReply);
}

//~ E0599
//~ "named `with_reply_shape` found for struct `InquiryPolicy`"
