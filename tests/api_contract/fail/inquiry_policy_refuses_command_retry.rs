use grafton_visca::request::{InquiryPolicy, CommandRetryClass, SubmissionClass};

fn main() {
    let _ = InquiryPolicy::new(CommandRetryClass::Standard, SubmissionClass::Normal);
    //~^ E0308 "mismatched types"
}
