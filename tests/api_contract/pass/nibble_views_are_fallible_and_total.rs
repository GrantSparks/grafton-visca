use grafton_visca::{wire::Nibbles, wire::Payload, Error};

fn checked(data: &[u8; 4]) -> Result<(), Error> {
    let view = Nibbles::from_array(data)?;
    let _: Option<u8> = view.byte(usize::MAX);
    let _: Option<u16> = view.u16_quad(usize::MAX);
    let _: Option<i16> = view.i16_quad(usize::MAX);
    Payload::new(data).assert_nibbles()?;
    Ok(())
}

fn main() {
    let _ = checked(&[0, 1, 2, 3]);
}
