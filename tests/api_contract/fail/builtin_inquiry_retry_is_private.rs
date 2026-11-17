use grafton_visca::request::CommandRetryClass;

fn main() {
    let _ = CommandRetryClass::BuiltinInquiry;
}

//~ E0599
//~ "named `BuiltinInquiry` found for enum `CommandRetryClass`"
