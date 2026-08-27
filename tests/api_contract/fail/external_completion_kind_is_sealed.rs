struct ExternalKind;

impl grafton_visca::completion::Kind for ExternalKind {}

fn main() {}

//~ E0277
//~ "the trait bound `ExternalKind: completion::private::Sealed` is not satisfied"
