use grafton_visca::SubmissionClass;

fn main() {
    let _ = SubmissionClass::Urgent;
}

//~ E0599
//~ "named `Urgent` found for enum `SubmissionClass`"
