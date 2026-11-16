use grafton_visca::{transport::HasTransportConfig, transport::TransportConfig};

struct MissingIdentity;

impl HasTransportConfig for MissingIdentity {
    fn transport_config(&self) -> &TransportConfig {
        unreachable!()
    }
}

fn main() {}

//~ E0046
//~ "missing: `identity`"
