//~ E0004
use grafton_visca::transport::FrameMeta;
fn main() {
    match FrameMeta::Raw {
        FrameMeta::Raw => (),
        FrameMeta::Sony { .. } => (),
    }
}
