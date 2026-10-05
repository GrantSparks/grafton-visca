use grafton_visca::ViscaEnum;

#[derive(Debug, Clone, Copy, ViscaEnum)]
#[repr(u8)]
enum SuffixedDiscriminant {
    First = 0x01u8,
}

fn main() {}

//~ "discriminant must be an unsuffixed integer literal in 0..=255"
