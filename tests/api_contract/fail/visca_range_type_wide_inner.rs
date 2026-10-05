grafton_visca::visca_range_type! {
    /// `ParameterOutOfRange` reports `i32` bounds, so a wider inner type is rejected.
    WideRange: u32 {
        min: 0,
        max: 70_000
    }
}

fn main() {}

//~ E0277
//~ "`u32` cannot be the inner type of a range-checked VISCA newtype"
//~ "use `u8`, `u16`, `i8`, `i16` or `i32` as the inner type"
