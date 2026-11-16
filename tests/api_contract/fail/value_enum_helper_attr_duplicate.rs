#[grafton_visca::value_enum]
enum Duplicate {
    #[wire(0)]
    First,
    #[cfg_attr(any(), doc = "No presence condition is added")]
    #[wire(0)]
    Second,
}

fn main() {}

//~ "duplicate value_enum wire code"
