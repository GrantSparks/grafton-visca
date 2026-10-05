use grafton_visca::ViscaEnum;

#[derive(Debug, Clone, Copy, ViscaEnum)]
enum NegativeDiscriminant {
    First = -1,
}

fn main() {}

//~ "discriminant must be an unsuffixed integer literal in 0..=255"
//~ ":5:13"
