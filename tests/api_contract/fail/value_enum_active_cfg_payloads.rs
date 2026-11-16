#[grafton_visca::value(min = 1, max = 3)]
struct Level(u8);

#[grafton_visca::value_enum]
enum Duplicate {
    #[cfg(all())]
    First(Level),
    Second(Level),
}

fn main() {}

//~ E0080
//~ "value_enum supports at most one checked payload variant"
