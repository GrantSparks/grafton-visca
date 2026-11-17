use grafton_visca::request::CommandRetryClass;

fn main() {
    let _ = CommandRetryClass::Inquiry;
}

//~ E0599
//~ "named `Inquiry` found for enum `CommandRetryClass`"
