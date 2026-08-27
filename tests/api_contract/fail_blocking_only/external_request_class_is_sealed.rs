struct ExternalClass;

impl grafton_visca::request::Class for ExternalClass {}

fn main() {}

//~ E0277
//~ "the trait bound `ExternalClass: request::private::Sealed` is not satisfied"
