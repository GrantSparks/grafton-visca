use grafton_visca::{
    request::InquiryPolicy,
    request::{CommandRetryClass, SubmissionClass},
};

fn main() {
    let _ = InquiryPolicy::new(CommandRetryClass::Standard, SubmissionClass::Normal);
}

//~ E0308
//~ "expected `InquiryRetryClass`, found `CommandRetryClass`"
