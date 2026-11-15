use grafton_visca::{raw::CommandPolicy, CommandCategory, ControlClass, RetryClass};

fn main() {
    let _ = CommandPolicy::new(CommandCategory::Quick, RetryClass::Never, ControlClass::Urgent);
}

//~ E0308
//~ "expected `SubmissionClass`, found `ControlClass`"
