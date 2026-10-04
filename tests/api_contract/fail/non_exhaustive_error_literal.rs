use grafton_visca::Error;
fn main() {
    let _ = Error::buffer_too_small(3, 0);
    let _ = Error::BufferTooSmall { required: 3, actual: 0 };
}
//~ E0639
//~ "cannot create non-exhaustive variant using struct expression"
