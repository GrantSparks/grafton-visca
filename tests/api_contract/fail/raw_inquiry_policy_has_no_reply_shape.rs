use grafton_visca::{raw::{InquiryPolicy, RawReplyShape}, RetryClass, SubmissionClass};

fn main() {
    let _ = InquiryPolicy::new(RetryClass::Inquiry, SubmissionClass::Normal)
        .with_reply_shape(RawReplyShape::NoReply);
}

//~ E0599
//~ "no method named `with_reply_shape` found for struct `InquiryPolicy`"
