use grafton_visca::value_enum;

#[value_enum]
enum DiscriminantCollision {
    Active = 0x01,
    Reserved = 0x01,
}

fn main() {}

//~ "duplicate value_enum wire code"
