//! Sealed facade permission gates, separate from source-backed `Has*` claims.

use crate::profiles::{AnyProfile, CameraProfile, CompileTimeProfile};

mod sealed {
    pub trait Permits<Gate> {}
}

/// Allows a facade method for a compile-time or runtime-selected profile.
///
/// A compile-time profile requires the corresponding source-backed `Has*`
/// marker. [`crate::profiles::AnyProfile`] permits the call and relies on the registered
/// specification's preparation gates before camera I/O.
#[allow(private_bounds)]
#[diagnostic::on_unimplemented(
    message = "camera profile `{Self}` does not support `{Gate}`",
    label = "this camera profile does not permit this capability",
    note = "this method requires the `Has{Gate}` marker grant; use `AnyProfile` for a runtime-selected camera and inspect its capabilities before calling"
)]
pub trait Permits<Gate>: CameraProfile + sealed::Permits<Gate> {}

macro_rules! define_permits {
    (
        [$(($domain:ident, $domain_marker:ident)),* $(,)?]
        [$( {
            surface: $surface:ident,
            marker: $marker:ident,
            $($fields:tt)*
        }, )*]
    ) => {
        /// Type tokens used by the sealed [`Permits`] facade bounds.
        pub mod gate {
            $(
                #[doc = concat!("Permission token for the `", stringify!($surface), "` surface.")]
                #[derive(Debug, Clone, Copy)]
                pub struct $surface;
            )*
            $(
                #[doc = concat!("Permission token for the `", stringify!($domain), "` domain.")]
                #[derive(Debug, Clone, Copy)]
                pub struct $domain;
            )*
        }

        #[cfg(any(feature = "blocking", feature = "async", test))]
        macro_rules! permit_gate {
            $(($marker) => { $crate::capabilities::gate::$surface };)*
            $(($domain_marker) => { $crate::capabilities::gate::$domain };)*
        }
        #[cfg(any(feature = "blocking", feature = "async"))]
        pub(crate) use permit_gate;

        $(
            impl<P: CompileTimeProfile + super::$marker> sealed::Permits<gate::$surface> for P {}
            impl<P: CompileTimeProfile + super::$marker> Permits<gate::$surface> for P {}
            impl sealed::Permits<gate::$surface> for AnyProfile {}
            impl Permits<gate::$surface> for AnyProfile {}
        )*
        $(
            impl<P: CompileTimeProfile + super::$domain_marker> sealed::Permits<gate::$domain> for P {}
            impl<P: CompileTimeProfile + super::$domain_marker> Permits<gate::$domain> for P {}
            impl sealed::Permits<gate::$domain> for AnyProfile {}
            impl Permits<gate::$domain> for AnyProfile {}
        )*

        #[cfg(test)]
        #[test]
        fn runtime_profile_permits_every_optional_surface_and_domain() {
            fn permits<Gate>() where AnyProfile: Permits<Gate> {}
            $(
                permits::<gate::$surface>();
                let _: Option<gate::$surface> = Option::<permit_gate!($marker)>::None;
            )*
            $(
                permits::<gate::$domain>();
                let _: Option<gate::$domain> = Option::<permit_gate!($domain_marker)>::None;
            )*
        }
    };
}

super::typed_support_registry::typed_support_registry!(
    define_permits,
    [
        (Power, HasPower),
        (Zoom, HasZoom),
        (Focus, HasFocus),
        (PanTilt, HasPanTilt),
        (Presets, HasPresets),
        (Exposure, HasExposure),
        (WhiteBalance, HasWhiteBalance),
        (Image, HasImage),
        (Menu, HasMenu),
    ]
);
