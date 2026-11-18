struct ExternalClass;

impl grafton_visca::request::Class for ExternalClass {
//~^ E0277 "the trait bound `ExternalClass: request::private::Sealed` is not satisfied"
    type Policy = grafton_visca::request::CommandPolicy;
}

fn main() {}
