//! Convenient imports for each session facade.

#[cfg(any(feature = "async", feature = "blocking"))]
macro_rules! signature_imports {
    ([$($noun:tt)*] $($motion:tt)*) => {
        crate::__macro_support::__noun_signature_types! { crate = crate; [$($noun)*] [$($motion)*] }
    };
}

#[cfg(any(feature = "async", feature = "blocking"))]
macro_rules! collect_signature_imports {
    ($($noun:tt)*) => {
        crate::noun_table::motion_table!(signature_imports, [$($noun)*]);
    };
}

#[cfg(any(feature = "async", feature = "blocking"))]
macro_rules! shared_prelude {
    ($($facade:ident)::+) => {
        pub use $($facade)::+::*;
        pub use crate::profiles::*;
        pub use crate::config::{CameraConfig, SessionConfig, OperationalTuning, TransportOptions};
        pub use crate::types::CameraId;
        crate::noun_table::noun_table!(collect_signature_imports);
        pub use crate::{Error, Result};
    };
}

/// Imports the async session facade and its configuration and value types.
#[cfg(feature = "async")]
pub mod r#async {
    shared_prelude!(crate::r#async);
}

/// Imports the blocking session facade and its configuration and value types.
#[cfg(feature = "blocking")]
pub mod blocking {
    shared_prelude!(crate::blocking);
}
