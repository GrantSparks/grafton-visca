use grafton_visca::request::builtin::{
//~^ E0603 "module `builtin` is private"
    BuiltinTypedRequestCoverage, BUILTIN_TYPED_REQUEST_INVENTORY,
    //~^ E0432 "unresolved imports `grafton_visca::request::builtin::BuiltinTypedRequestCoverage`, `grafton_visca::request::builtin::BUILTIN_TYPED_REQUEST_INVENTORY`"
};

fn main() {}
