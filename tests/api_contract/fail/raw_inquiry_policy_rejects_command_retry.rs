use grafton_visca::{
    raw::InquiryPolicy,
    request::{RetryClass, SubmissionClass},
};

fn main() {
    let _ = InquiryPolicy::new(RetryClass::Standard, SubmissionClass::Normal);
}

//~ E0308
//~ "expected `InquiryRetryClass`, found `RetryClass`"
