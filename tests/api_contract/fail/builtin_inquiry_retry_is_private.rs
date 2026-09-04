use grafton_visca::RetryClass;

fn main() {
    let _ = RetryClass::BuiltinInquiry;
}

//~ E0599
//~ "named `BuiltinInquiry` found for enum `RetryClass`"
