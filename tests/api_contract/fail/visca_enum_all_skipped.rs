use grafton_visca::value_enum;

#[value_enum]
enum Empty {}
//~^ "value_enum requires at least one variant"

fn main() {}
