use grafton_visca::{transport::HasTransportConfig, transport::IoBounds};

struct MissingIdentity;

impl HasTransportConfig for MissingIdentity {
//~^ E0046 "missing: `identity`"
    fn io_bounds(&self) -> &IoBounds {
        unreachable!()
    }
}

fn main() {}
