use grafton_visca::transport::FrameMeta;
fn main() {
    let _ = FrameMeta { sequence: None };
}
//~ E0639
//~ "cannot create non-exhaustive struct using struct expression"
