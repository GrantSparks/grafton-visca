use grafton_visca::ViscaEnum;

#[derive(Debug, Clone, Copy, ViscaEnum)]
#[visca_enum(exhaustive = true)]
enum ExhaustiveFlag {
    First = 0x01,
}

fn main() {}

//~ "unknown `visca_enum` attribute `exhaustive`; expected `error_type`"
