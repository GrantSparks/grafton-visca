//! Keep profile-copy fixtures explicit about their independent wire families.

use grafton_visca::profiles::{ProfileSpec, ProfileSpecBuilder};

pub fn with_source_encodings(
    mut builder: ProfileSpecBuilder,
    source: &ProfileSpec,
) -> ProfileSpecBuilder {
    for encoding in source.encodings().iter() {
        builder = builder.encoding(encoding);
    }
    builder
}
