use grafton_visca::value_enum;

#[value_enum(exhaustive = true)]
//~^ "unknown `value_enum` attribute `exhaustive`; expected `wire` or `crate`"
enum ExhaustiveFlag {
    First = 1,
}

fn main() {}
