use grafton_visca::ViscaEnum;

#[derive(Debug, Clone, Copy, ViscaEnum)]
enum WideDiscriminant {
    First = 256,
}

fn main() {}

//~ "discriminant must be an unsuffixed integer literal in 0..=255"
//~ ":5:13"
