use grafton_visca::request::{InquiryPolicy, InquiryRetryClass, SubmissionClass};

fn main() {
    let _ = InquiryPolicy::new(InquiryRetryClass::Inquiry, SubmissionClass::Normal).timeout_class();
    //~^ E0599 "named `timeout_class` found for struct `InquiryPolicy`"
}
