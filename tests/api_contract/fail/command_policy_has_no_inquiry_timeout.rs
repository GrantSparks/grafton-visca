use grafton_visca::profiles::CommandTimeoutClass;

fn main() {
    let _ = CommandTimeoutClass::Inquiry;
}

//~ E0599
//~ "named `Inquiry` found for enum `CommandTimeoutClass`"
