use grafton_visca::{
    profiles::CommandTimeoutClass, request::CommandPolicy, request::ControlClass, request::CommandRetryClass,
};

fn main() {
    let _ = CommandPolicy::new(
        CommandTimeoutClass::Quick,
        CommandRetryClass::Never,
        ControlClass::Urgent,
        //~^ E0308 "mismatched types"
    );
}
