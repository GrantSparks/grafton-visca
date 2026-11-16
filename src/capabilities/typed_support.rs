//! Runtime representation of profile-backed typed API support.
//!
//! Domain records describe runtime facts. `TypedSupportSet` records the
//! optional control and inquiry permissions a profile has source evidence for.

use std::fmt;

macro_rules! define_typed_support_marker {
    (
        $surface:ident,
        $marker:ident,
        $marker_doc:literal,
        $diagnostic:literal
    ) => {
        #[doc = $marker_doc]
        #[diagnostic::on_unimplemented(
            message = $diagnostic,
            label = "profile does not implement the required typed-support marker",
            note = "see the generated marker tables in docs/camera_profile_support.md; add this bound only for profiles with source-backed typed support"
        )]
        pub trait $marker {}
    };
}

macro_rules! define_typed_support_surfaces {
    (
        [
            $(
                {
                    surface: $surface:ident,
                    marker: $marker:ident,
                    $(encodings: [$($encoding_variant:ident => $encoding_tag:literal),+ $(,)?],)?
                    area: $area:literal,
                    surface_doc: $surface_doc:literal,
                    marker_doc: $marker_doc:literal,
                    diagnostic: $diagnostic:literal,
                },
            )*
        ]
    ) => {
        const TYPED_SUPPORT_SURFACE_COUNT: usize = [$(stringify!($surface)),*].len();
        const _: () = assert!(TYPED_SUPPORT_SURFACE_COUNT <= u128::BITS as usize, "typed support vocabulary exceeds inline bitset capacity");

        /// Optional typed API surface backed by a profile support marker.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
        #[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
        #[repr(u8)]
        #[non_exhaustive]
        pub enum TypedSupportSurface {
            $(
                #[doc = $surface_doc]
                $surface,
            )*
        }

        impl TypedSupportSurface {
            /// All known typed support surfaces in their canonical serde order.
            pub const ALL: [Self; TYPED_SUPPORT_SURFACE_COUNT] = [
                $(Self::$surface,)*
            ];

            const fn bit(self) -> u128 {
                1 << (self as u8)
            }

            /// Returns the marker-trait spelling paired with this surface.
            #[cfg(test)]
            pub(crate) const fn marker_trait_name(self) -> &'static str {
                match self {
                    $(Self::$surface => stringify!($marker),)*
                }
            }

            /// Returns the contributor-guide area for this surface.
            #[cfg(test)]
            pub(crate) const fn documentation_area(self) -> &'static str {
                match self {
                    $(Self::$surface => $area,)*
                }
            }

            /// Returns the stable serde spelling for this surface.
            #[cfg(test)]
            pub(crate) fn wire_name(self) -> String {
                let variant = match self { $(Self::$surface => stringify!($surface),)* };
                let mut wire = String::new();
                for (index, letter) in variant.chars().enumerate() {
                    if index != 0 && letter.is_ascii_uppercase() { wire.push('_'); }
                    wire.push(letter.to_ascii_lowercase());
                }
                wire
            }
        }

        $(define_typed_support_marker!($surface, $marker, $marker_doc, $diagnostic);)*
    };
}

super::typed_support_registry::typed_support_registry!(define_typed_support_surfaces);

/// Generates `typed_surface!`, the one marker-to-surface lookup.
///
/// `typed_surface!(HasDirectZoom)` expands to
/// `TypedSupportSurface::DirectZoom`. Each arm comes from one registry row,
/// so the static marker named in a noun-table `where` clause or header gate
/// and the runtime surface the dynamic facade checks cannot disagree, and a
/// marker without a registry row fails to expand.
macro_rules! define_typed_surface_lookup {
    (
        [
            $(
                {
                    surface: $surface:ident,
                    marker: $marker:ident,
                    $($fields:tt)*
                },
            )*
        ]
    ) => {
        macro_rules! typed_surface {
            $(
                ($marker) => {
                    $crate::capabilities::TypedSupportSurface::$surface
                };
            )*
        }

        pub(crate) use typed_surface;
    };
}

super::typed_support_registry::typed_support_registry!(define_typed_surface_lookup);

/// The noun table owns both method names and marker membership. Keep this
/// contributor projection beside the vocabulary rather than restating either
/// fact in the typed-support registry.
#[cfg(test)]
macro_rules! typed_method_vocabulary {
    (@header $entries:ident, [typed $marker:ident], $method:expr) => {
        $entries.push((typed_surface!($marker), $method));
    };
    (@header $entries:ident, $base:tt, $method:expr) => {};
    (
        $(
            @noun $noun:ident {
                accessor: $accessor:ident,
                getter: $getter:ident,
                dyn_trait: $dyn_trait:ident,
                gate: $base:tt,
                doc: $noun_doc:literal,
            };
            $(
                $(#[$doc:meta])*
                $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions;
        $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        #[allow(clippy::vec_init_then_push)] // Header gates expand to optional statements.
        fn noun_typed_methods() -> Vec<(TypedSupportSurface, &'static str)> {
            let mut entries = Vec::new();
            $( $(
                typed_method_vocabulary!(@header entries, $base, concat!(stringify!($getter), "().", stringify!($method)));
                $(
                    entries.push((typed_surface!($gate), concat!(stringify!($getter), "().", stringify!($method))));
                    $(entries.push((typed_surface!($extra), concat!(stringify!($getter), "().", stringify!($method))));)*
                )?
            )* )*
            entries
        }
    };
}

#[cfg(test)]
crate::noun_table::noun_table!(typed_method_vocabulary);

#[cfg(test)]
impl TypedSupportSurface {
    pub(crate) fn documentation_api(self) -> String {
        if self == Self::DigitalZoomRange {
            return "Runtime-only gate for `zoom().set_normalized(..., ZoomDomain::OpticalPlusDigital)`; also requires a documented digital maximum".to_owned();
        }
        let mut methods = Vec::new();
        for (_, method) in noun_typed_methods()
            .into_iter()
            .filter(|(surface, _)| *surface == self)
        {
            if !methods.contains(&method) {
                methods.push(method);
            }
        }
        methods
            .into_iter()
            .map(|method| format!("`{method}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Compact set of typed API surfaces supported by a profile.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TypedSupportSet(u128);

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for TypedSupportSet {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("TypedSupportSet")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <Vec<TypedSupportSurface> as schemars::JsonSchema>::json_schema(generator)
    }
}

#[cfg(feature = "ts-rs")]
impl ts_rs::TS for TypedSupportSet {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_config: &ts_rs::Config) -> String {
        "TypedSupportSet".into()
    }

    fn inline(config: &ts_rs::Config) -> String {
        format!(
            "Array<{}>",
            <TypedSupportSurface as ts_rs::TS>::name(config)
        )
    }

    fn decl(config: &ts_rs::Config) -> String {
        format!(
            "type TypedSupportSet = {};",
            <Self as ts_rs::TS>::inline(config)
        )
    }

    fn decl_concrete(config: &ts_rs::Config) -> String {
        <Self as ts_rs::TS>::decl(config)
    }

    fn visit_dependencies(visitor: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        visitor.visit::<TypedSupportSurface>();
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("TypedSupportSet.ts".into())
    }
}

impl TypedSupportSet {
    /// Empty typed support set.
    pub const EMPTY: Self = Self(0);

    /// Returns an empty typed support set.
    #[must_use]
    pub const fn empty() -> Self {
        Self::EMPTY
    }

    /// Creates a set containing one typed support surface.
    #[must_use]
    pub const fn from_surface(surface: TypedSupportSurface) -> Self {
        Self(surface.bit())
    }

    /// Creates a set containing all listed typed support surfaces.
    ///
    /// Duplicate surfaces are ignored because the representation is a bitset.
    #[must_use]
    pub const fn from_surfaces(surfaces: &[TypedSupportSurface]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < surfaces.len() {
            bits |= surfaces[index].bit();
            index += 1;
        }
        Self(bits)
    }

    /// Returns true when the set contains no typed support surfaces.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true when this set contains `surface`.
    #[must_use]
    pub const fn contains(self, surface: TypedSupportSurface) -> bool {
        self.0 & surface.bit() != 0
    }

    /// Returns this set without `surface`.
    #[must_use]
    pub const fn without(self, surface: TypedSupportSurface) -> Self {
        Self(self.0 & !surface.bit())
    }

    /// Returns true when this set contains every surface in `required`.
    #[must_use]
    pub const fn contains_all(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    /// Returns the union of two typed support sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Iterates over surfaces present in this set.
    pub fn iter(self) -> impl Iterator<Item = TypedSupportSurface> {
        TypedSupportSurface::ALL
            .into_iter()
            .filter(move |surface| self.contains(*surface))
    }
}

impl fmt::Debug for TypedSupportSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for TypedSupportSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TypedSupportSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let surfaces = Vec::<TypedSupportSurface>::deserialize(deserializer)?;
        Ok(Self::from_surfaces(&surfaces))
    }
}

#[cfg(test)]
mod tests {
    use super::{noun_typed_methods, TypedSupportSet, TypedSupportSurface};

    #[test]
    fn every_typed_surface_has_noun_methods_or_the_runtime_only_note() {
        let methods = noun_typed_methods();
        for surface in TypedSupportSurface::ALL {
            let has_methods = methods.iter().any(|(marker, _)| *marker == surface);
            assert_eq!(
                has_methods,
                surface != TypedSupportSurface::DigitalZoomRange,
                "{surface:?}"
            );
            assert!(!surface.documentation_api().is_empty());
        }
    }

    #[test]
    fn empty_set_contains_no_surfaces() {
        let set = TypedSupportSet::empty();

        assert!(set.is_empty());
        assert!(!set.contains(TypedSupportSurface::DirectZoom));
    }

    #[test]
    fn single_surface_set_reports_membership() {
        let set = TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);

        assert!(set.contains(TypedSupportSurface::DirectZoom));
        assert!(!set.contains(TypedSupportSurface::DigitalZoomToggle));
    }

    #[test]
    fn combined_set_reports_all_members() {
        let set = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DigitalZoomToggle,
        ]);

        assert!(set.contains(TypedSupportSurface::DirectZoom));
        assert!(set.contains(TypedSupportSurface::DigitalZoomToggle));
        assert!(!set.contains(TypedSupportSurface::FocusZone));
    }

    #[test]
    fn duplicate_surfaces_are_idempotent() {
        let duplicated = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DirectZoom,
        ]);
        let single = TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);

        assert_eq!(duplicated, single);
    }

    #[test]
    fn contains_all_requires_every_surface() {
        let set = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DigitalZoomRange,
        ]);

        assert!(set.contains_all(TypedSupportSet::from_surface(
            TypedSupportSurface::DirectZoom
        )));
        assert!(!set.contains_all(TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DigitalZoomToggle,
        ])));
    }

    #[test]
    fn appended_focus_zone_inquiry_and_usb_audio_bits_round_trip() {
        let set = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::FocusZoneInquiry,
            TypedSupportSurface::UsbAudio,
        ]);

        assert!(set.contains(TypedSupportSurface::FocusZoneInquiry));
        assert!(set.contains(TypedSupportSurface::UsbAudio));
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            vec![
                TypedSupportSurface::FocusZoneInquiry,
                TypedSupportSurface::UsbAudio,
            ]
        );
    }

    #[test]
    fn generated_private_bits_are_unique_and_dense() {
        for (index, surface) in TypedSupportSurface::ALL.into_iter().enumerate() {
            assert_eq!(
                TypedSupportSet::from_surface(surface).0,
                1_u128 << index,
                "{surface:?} must use the generated bit matching its canonical position"
            );
        }
    }

    #[test]
    fn generated_wire_names_are_unique_and_nonempty() {
        let mut names = std::collections::BTreeSet::new();
        for surface in TypedSupportSurface::ALL {
            assert!(!surface.wire_name().is_empty());
            assert!(
                names.insert(surface.wire_name()),
                "duplicate typed-support wire name for {surface:?}"
            );
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_uses_snake_case_for_every_surface() -> Result<(), serde_json::Error> {
        assert_eq!(TypedSupportSurface::ALL.len(), 82);
        for surface in TypedSupportSurface::ALL {
            let wire = surface.wire_name();
            assert!(wire
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'));
            let json = serde_json::to_string(&surface)?;
            assert_eq!(json, serde_json::to_string(&wire)?);
            assert_eq!(serde_json::from_str::<TypedSupportSurface>(&json)?, surface);
        }
        assert!(serde_json::from_str::<TypedSupportSurface>("\"ptz-optics-snap-focus\"").is_err());
        Ok(())
    }
}
