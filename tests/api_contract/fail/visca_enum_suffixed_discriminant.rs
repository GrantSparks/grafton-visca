use grafton_visca::value_enum;

#[value_enum]
#[repr(u8)]
enum SuffixedDiscriminant {
    First = 0x01u8,
    //~^ "wire codes must be unsuffixed integer literals"
}

fn main() {}
