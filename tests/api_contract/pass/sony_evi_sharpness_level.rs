use grafton_visca::{
    capabilities::HasSharpnessControl,
    profiles::SonyEVIH100,
};

fn requires_level<P: HasSharpnessControl>() {}

fn main() {
    requires_level::<SonyEVIH100>();
}
