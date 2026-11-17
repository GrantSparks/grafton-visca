#[grafton_visca::value(min = -2, max = 2, wire = centered(2, u8))]
struct Extension(i8);

#[grafton_visca::value_enum]
enum Overlapping {
    #[cfg(all())]
    Unknown(Extension),
    #[cfg_attr(all(), cfg(all()))]
    Known = 4,
}

fn main() {}

//~ E0080
//~ "value_enum payload domain contains a unit wire code"
