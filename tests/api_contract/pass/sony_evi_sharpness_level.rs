use grafton_visca::{capabilities::HasSharpness, profiles::SonyEviH100};

fn requires_level<P: HasSharpness>() {}

fn main() {
    requires_level::<SonyEviH100>();
}
