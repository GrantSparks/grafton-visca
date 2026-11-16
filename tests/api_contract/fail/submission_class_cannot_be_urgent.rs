use grafton_visca::request::SubmissionClass;

fn main() {
    let _ = SubmissionClass::Urgent;
}

//~ E0599
//~ "named `Urgent` found for enum `SubmissionClass`"
