#[grafton_visca::value(min = 1, max = 3)]
struct Extension(u8);

#[grafton_visca::value_enum]
//~^ E0080 "value_enum payload domain contains a unit wire code"
enum Overlapping {
    Known = 1,
    Unknown(Extension),
}

fn main() {}
