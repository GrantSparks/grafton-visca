struct ExternalKind;

impl grafton_visca::operation::Kind for ExternalKind {}

fn main() {}

//~ E0277
//~ "the trait bound `ExternalKind: operation::private::Sealed` is not satisfied"
