use grafton_visca::wire::Nibbles;

fn main() -> grafton_visca::Result<()> {
    let _: u8 = Nibbles::<1>::from_array(&[0xF])?.value();
    let _: u16 = Nibbles::<3>::from_array(&[0xF; 3])?.value();
    let _: u16 = Nibbles::<4>::from_array(&[0xF; 4])?.value();
    let _: i16 = Nibbles::<4>::from_array(&[0xF; 4])?.signed();
    let _: i32 = Nibbles::<5>::from_array(&[0xF; 5])?.signed();
    let (first, second): (Nibbles<'_, 4>, Nibbles<'_, 4>) =
        Nibbles::<8>::from_array(&[0xF; 8])?.halves();
    let _ = (first.value(), second.signed());
    let _: u8 = Nibbles::<1>::from_array(&[0xF])?.zero_extended_nibble()?;
    let _: u8 = Nibbles::<2>::from_array(&[0xF; 2])?.zero_extended_u8()?;
    Ok(())
}
