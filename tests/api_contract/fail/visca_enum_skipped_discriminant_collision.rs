use grafton_visca::ViscaEnum;

#[derive(ViscaEnum)]
enum SkippedDiscriminantCollision {
    Active = 0x01,
    #[visca_enum(skip)]
    Reserved = 0x01,
}

fn main() {}

//~ "Discriminant value 0x1 is already used by variant Active"
//~ E0081
