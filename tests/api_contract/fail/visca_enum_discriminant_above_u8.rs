use grafton_visca::value_enum;

#[value_enum]
enum WideDiscriminant {
    First = 256,
    //~^ "wire code must be an unsuffixed integer literal in 0..=255"
}

fn main() {}
