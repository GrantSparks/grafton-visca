use grafton_visca::value_enum;

#[value_enum(wire = u8, wire = u16)]
//~^ "duplicate `value_enum` attribute `wire`"
//~^^ "first `wire` specified here"
enum DuplicateEnumAttribute {
    First = 1,
}

#[value_enum]
enum DuplicateVariantAttribute {
    #[wire(1)]
    #[wire(2)]
    //~^ "duplicate variant wire code"
    First,
}

fn main() {}
