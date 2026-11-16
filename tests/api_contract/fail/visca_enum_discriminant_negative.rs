use grafton_visca::value_enum;

#[value_enum]
enum NegativeDiscriminant {
    First = -1,
}

fn main() {}

//~ "wire code must be an unsuffixed integer literal in 0..=255"
