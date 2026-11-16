use grafton_visca::{
    raw::InquiryPolicy, raw::RawReplyShape, request::InquiryRetryClass, request::SubmissionClass,
};

fn main() {
    let _ = InquiryPolicy::new(InquiryRetryClass::Inquiry, SubmissionClass::Normal)
        .with_reply_shape(RawReplyShape::NoReply);
}

//~ E0599
//~ "no method named `with_reply_shape` found for struct `InquiryPolicy`"
