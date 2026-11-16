use grafton_visca::value_enum;

#[value_enum]
enum Empty {}

fn main() {}

//~ "value_enum requires at least one variant"
