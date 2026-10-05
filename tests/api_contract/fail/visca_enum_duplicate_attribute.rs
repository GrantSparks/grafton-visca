use grafton_visca::ViscaEnum;

#[derive(Debug, Clone, Copy, ViscaEnum)]
#[visca_enum(error_type = grafton_visca::Error)]
#[visca_enum(error_type = grafton_visca::Error)]
enum DuplicateEnumAttribute {
    First = 0x01,
}

#[derive(Debug, Clone, Copy, ViscaEnum)]
enum DuplicateVariantAttribute {
    #[visca_enum(name = "First")]
    #[visca_enum(name = "Primary")]
    First = 0x01,
}

fn main() {}

//~ "duplicate `visca_enum` attribute `error_type`"
//~ "first `error_type` specified here"
//~ "duplicate `visca_enum` attribute `name`"
//~ "first `name` specified here"
