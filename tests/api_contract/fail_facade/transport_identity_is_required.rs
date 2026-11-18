use grafton_visca::{transport::HasTransportConfig, transport::ValidatedTransportConfig};

struct MissingIdentity;

impl HasTransportConfig for MissingIdentity {
//~^ E0046 "missing: `identity`"
    fn transport_config(&self) -> &ValidatedTransportConfig {
        unreachable!()
    }
}

fn main() {}
