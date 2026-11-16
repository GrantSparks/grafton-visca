#[grafton_visca::value_enum]
enum Duplicate {
    #[wire(0)]
    First,
    #[cfg_attr(all(), cfg(all()))]
    #[wire(0)]
    Second,
}

fn main() {}

//~ E0080
//~ "duplicate value_enum wire code"
