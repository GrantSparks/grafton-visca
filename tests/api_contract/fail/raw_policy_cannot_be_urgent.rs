use grafton_visca::{
    profiles::CommandTimeoutClass, raw::CommandPolicy, request::ControlClass, request::RetryClass,
};

fn main() {
    let _ = CommandPolicy::new(
        CommandTimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::Urgent,
    );
}

//~ E0308
//~ "expected `SubmissionClass`, found `ControlClass`"
