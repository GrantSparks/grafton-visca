use grafton_visca::ViscaValue;

grafton_visca::visca_range_type! {
    /// A range whose bounds are swapped can hold no value.
    InvertedRange: u8 {
        min: 5,
        max: 2
    }
}

#[derive(Debug, Clone, Copy, ViscaValue)]
#[visca_value(min = "9", max = "-1")]
struct InvertedValue(i8);

fn main() {}

//~ E0080
//~ "`InvertedRange` declares `min` greater than `max`"
//~ "`InvertedValue` declares `min` greater than `max`"
