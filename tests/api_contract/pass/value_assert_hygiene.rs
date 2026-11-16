use grafton_visca::wire::WireValue;

#[allow(unused_macros)]
macro_rules! assert {
    ($($tokens:tt)*) => {
        compile_error!("value expansion captured the caller's assert macro");
    };
}

#[grafton_visca::value(min = 0, max = 3)]
struct Level(u8);

#[grafton_visca::value(min = -2, max = 2, wire = biased(2))]
struct Shifted(i8);

#[grafton_visca::value(min = -1.0, max = 1.0)]
struct Finite(f32);

#[grafton_visca::value(valid_values = [-1.0, 0.0, 1.0])]
struct FiniteSet(f32);

fn main() -> grafton_visca::Result<()> {
    ::core::assert_eq!(Level::new(3)?.value(), 3);
    ::core::assert_eq!(Shifted::new(-2)?.to_wire(), 0);
    ::core::assert_eq!(Shifted::from_wire(4)?.value(), 2);
    ::core::assert_eq!(Finite::new(0.5)?.value(), 0.5);
    ::core::assert_eq!(FiniteSet::new(1.0)?.value(), 1.0);
    Ok(())
}
