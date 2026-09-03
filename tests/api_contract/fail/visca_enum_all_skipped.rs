use grafton_visca::ViscaEnum;

#[derive(ViscaEnum)]
enum AllSkipped {
    #[visca_enum(skip)]
    Reserved = 0xFF,
}

fn main() {}

//~ "ViscaEnum requires at least one non-skipped variant"
