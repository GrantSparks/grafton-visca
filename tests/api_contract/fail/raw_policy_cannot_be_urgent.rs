use grafton_visca::{
    profiles::CommandTimeoutClass, request::CommandPolicy, request::SubmissionClass, request::CommandRetryClass,
};

fn main() {
    let _ = CommandPolicy::new(
        CommandTimeoutClass::Quick,
        CommandRetryClass::Never,
        SubmissionClass::Urgent,
        //~^ E0599 "named `Urgent`"
    );
}
