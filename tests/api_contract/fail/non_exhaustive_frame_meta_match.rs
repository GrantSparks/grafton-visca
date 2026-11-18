use grafton_visca::transport::FrameMeta;
fn main() {
    match FrameMeta::Raw {
        //~^ E0004 "non-exhaustive patterns: `_` not covered"
        FrameMeta::Raw => (),
        FrameMeta::Sony { .. } => (),
    }
}
