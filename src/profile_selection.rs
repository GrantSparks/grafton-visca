//! Runtime profile selection and the sealed profile kind used by camera facades.

use std::{fmt, marker::PhantomData};

use crate::profiles::{CompileTimeProfile, ProfileId, ProfileSpec};

/// A camera profile resolved from the target's runtime registration.
///
/// This is the default profile parameter for camera facades and configurations.
/// Its methods validate the registered specification before camera I/O.
/// It makes no compile-time `Has*` capability claims.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AnyProfile;

/// A built-in profile identifier or a complete custom profile specification.
///
/// With serde, built-ins are their pinned identifier strings and custom profiles
/// are specification objects. Both forms resolve through [`Self::spec`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(untagged, export))]
pub enum ProfileSelection {
    /// A built-in profile resolved through the current registry.
    Builtin(ProfileId),
    /// A validated custom profile with all of its protocol facts.
    Custom(Box<ProfileSpec>),
}

impl ProfileSelection {
    /// Resolves the selection to a validated, owned specification.
    pub fn spec(&self) -> crate::Result<ProfileSpec> {
        match self {
            Self::Builtin(id) => id.spec(),
            Self::Custom(spec) => Ok(spec.as_ref().clone()),
        }
    }
}

impl From<ProfileId> for ProfileSelection {
    fn from(id: ProfileId) -> Self {
        Self::Builtin(id)
    }
}

impl From<ProfileSpec> for ProfileSelection {
    fn from(spec: ProfileSpec) -> Self {
        Self::Custom(Box::new(spec))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ProfileSelection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Builtin(id) => serde::Serialize::serialize(id, serializer),
            Self::Custom(spec) => serde::Serialize::serialize(spec, serializer),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ProfileSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SelectionVisitor;

        impl<'de> serde::de::Visitor<'de> for SelectionVisitor {
            type Value = ProfileSelection;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .write_str("a built-in profile id string or a profile specification object")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                // ProfileId's generated deserializer owns the complete pinned
                // id inventory, including the valid-id list in this diagnostic.
                <ProfileId as serde::Deserialize>::deserialize(
                    serde::de::value::StrDeserializer::<E>::new(value),
                )
                .map(ProfileSelection::Builtin)
                .map_err(|error| E::custom(format_args!("unknown profile id `{value}`: {error}")))
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                <ProfileSpec as serde::Deserialize>::deserialize(
                    serde::de::value::MapAccessDeserializer::new(map),
                )
                .map(ProfileSelection::from)
            }
        }

        deserializer.deserialize_any(SelectionVisitor)
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for ProfileSelection {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("ProfileSelection")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "oneOf": [
                generator.subschema_for::<ProfileId>(),
                generator.subschema_for::<ProfileSpec>(),
            ]
        })
    }
}

mod sealed {
    pub trait CameraProfile {}

    impl<P: super::CompileTimeProfile> CameraProfile for P {}
    impl CameraProfile for super::AnyProfile {}
}

/// A compile-time profile or the runtime-selected [`crate::profiles::AnyProfile`].
///
/// This trait is sealed. Community profiles implement [`crate::profiles::CompileTimeProfile`]
/// and acquire this implementation automatically.
#[allow(private_bounds)]
pub trait CameraProfile: sealed::CameraProfile {
    /// The declared optional grants, or runtime validation for [`crate::profiles::AnyProfile`].
    #[doc(hidden)]
    const TYPED_SUPPORT: Option<crate::capabilities::TypedSupportSet>;

    /// The configuration's checked profile selection.
    ///
    /// Runtime selections retain either persisted form. Typed selections also
    /// prove equality with the compile-time profile's complete specification.
    type Selection: Clone + fmt::Debug + Send + Sync;

    /// Checks a selection for this profile kind.
    #[doc(hidden)]
    fn select(profile: ProfileSelection) -> crate::Result<Self::Selection>;

    /// Resolves the configuration's checked selection.
    #[doc(hidden)]
    fn selected_spec(selection: &Self::Selection) -> crate::Result<ProfileSpec>;

    /// Checks whether a registered specification can back this facade.
    #[doc(hidden)]
    fn validate_spec(profile: &ProfileSpec) -> crate::Result<()>;
}

impl CameraProfile for AnyProfile {
    const TYPED_SUPPORT: Option<crate::capabilities::TypedSupportSet> = None;

    type Selection = ProfileSelection;

    fn select(profile: ProfileSelection) -> crate::Result<Self::Selection> {
        Ok(profile)
    }

    fn selected_spec(selection: &Self::Selection) -> crate::Result<ProfileSpec> {
        selection.spec()
    }

    fn validate_spec(_profile: &ProfileSpec) -> crate::Result<()> {
        Ok(())
    }
}

/// Implementation type for a configuration's checked compile-time selection.
///
/// Its serde representation is the same string-or-object representation as
/// [`crate::profiles::ProfileSelection`]. Deserialization verifies every profile fact.
#[doc(hidden)]
pub struct TypedProfileSelection<P> {
    selection: Option<ProfileSelection>,
    profile: PhantomData<fn() -> P>,
}

impl<P> Clone for TypedProfileSelection<P> {
    fn clone(&self) -> Self {
        Self {
            selection: self.selection.clone(),
            profile: PhantomData,
        }
    }
}

impl<P> fmt::Debug for TypedProfileSelection<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TypedProfileSelection")
            .field(&self.selection)
            .finish()
    }
}

impl<P> TypedProfileSelection<P> {
    /// Defers a typed constructor's fallible profile resolution until use.
    pub(crate) const fn deferred() -> Self {
        Self {
            selection: None,
            profile: PhantomData,
        }
    }
}

impl<P: CompileTimeProfile> TypedProfileSelection<P> {
    fn spec(&self) -> crate::Result<ProfileSpec> {
        // A checked registry identity makes view selection cheap, but it must
        // not let a downstream declaration bypass its own specification at
        // configuration resolution.
        if P::ID.is_some() {
            return ProfileSpec::from_compile_time::<P>();
        }
        self.selection
            .as_ref()
            .map_or_else(ProfileSpec::from_compile_time::<P>, ProfileSelection::spec)
    }
}

impl<P: CompileTimeProfile> CameraProfile for P {
    const TYPED_SUPPORT: Option<crate::capabilities::TypedSupportSet> =
        Some(<P as CompileTimeProfile>::TYPED_SUPPORT);

    type Selection = TypedProfileSelection<P>;

    fn select(selection: ProfileSelection) -> crate::Result<Self::Selection> {
        if let (Some(expected), ProfileSelection::Builtin(actual)) = (P::ID, &selection) {
            if expected != *actual || P::TYPED_SUPPORT != actual.typed_support() {
                return Err(profile_mismatch());
            }
        } else {
            Self::validate_spec(&selection.spec()?)?;
        }
        Ok(TypedProfileSelection {
            selection: Some(selection),
            profile: PhantomData,
        })
    }

    fn selected_spec(selection: &Self::Selection) -> crate::Result<ProfileSpec> {
        selection.spec()
    }

    fn validate_spec(profile: &ProfileSpec) -> crate::Result<()> {
        // ProfileSpec's only constructor proves that a built-in identity
        // covers every registry fact. Reuse that proof for each camera view.
        let matches = if let Some(id) = P::ID {
            profile.capabilities().profile_id == Some(id)
                && profile.capabilities().typed_support == P::TYPED_SUPPORT
        } else {
            *profile == ProfileSpec::from_compile_time::<P>()?
        };
        if matches {
            Ok(())
        } else {
            Err(profile_mismatch())
        }
    }
}

fn profile_mismatch() -> crate::Error {
    crate::profiles::invalid_profile_fields(
        &["profile"],
        "selected specification does not match the requested compile-time profile",
    )
}

#[cfg(feature = "serde")]
impl<P: CompileTimeProfile> serde::Serialize for TypedProfileSelection<P> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if let Some(selection) = &self.selection {
            return serde::Serialize::serialize(selection, serializer);
        }
        // Infallible typed configuration constructors defer this checked
        // resolution. Serialization reports its error through serde, never
        // substituting a default profile or panicking.
        let spec = self.spec().map_err(serde::ser::Error::custom)?;
        match P::ID {
            Some(id) => serde::Serialize::serialize(&id, serializer),
            None => serde::Serialize::serialize(&spec, serializer),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de, P: CompileTimeProfile> serde::Deserialize<'de> for TypedProfileSelection<P> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let selection = <ProfileSelection as serde::Deserialize>::deserialize(deserializer)?;
        <P as CameraProfile>::select(selection).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "schemars")]
impl<P> schemars::JsonSchema for TypedProfileSelection<P> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        <ProfileSelection as schemars::JsonSchema>::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <ProfileSelection as schemars::JsonSchema>::json_schema(generator)
    }
}

#[cfg(feature = "ts-rs")]
impl<P> ts_rs::TS for TypedProfileSelection<P> {
    type WithoutGenerics = ProfileSelection;
    type OptionInnerType = Self;

    fn name(config: &ts_rs::Config) -> String {
        <ProfileSelection as ts_rs::TS>::name(config)
    }

    fn output_path() -> Option<std::path::PathBuf> {
        <ProfileSelection as ts_rs::TS>::output_path()
    }

    fn inline(config: &ts_rs::Config) -> String {
        <ProfileSelection as ts_rs::TS>::inline(config)
    }

    fn decl(config: &ts_rs::Config) -> String {
        <ProfileSelection as ts_rs::TS>::decl(config)
    }

    fn decl_concrete(config: &ts_rs::Config) -> String {
        <ProfileSelection as ts_rs::TS>::decl_concrete(config)
    }

    fn visit_dependencies(visitor: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        <ProfileSelection as ts_rs::TS>::visit_dependencies(visitor);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod identity_tests {
    use super::*;
    use crate::{capabilities::TypedSupportSet, profiles::PtzOpticsG2};

    struct MismatchedGrants;

    impl CompileTimeProfile for MismatchedGrants {
        const ID: Option<ProfileId> = Some(ProfileId::PtzOpticsG2);
        const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::empty();

        fn spec() -> crate::Result<ProfileSpec> {
            ProfileId::PtzOpticsG2.spec()
        }
    }

    struct FailingSpecification;

    impl CompileTimeProfile for FailingSpecification {
        const ID: Option<ProfileId> = Some(ProfileId::PtzOpticsG2);
        const TYPED_SUPPORT: TypedSupportSet = <PtzOpticsG2 as CompileTimeProfile>::TYPED_SUPPORT;

        fn spec() -> crate::Result<ProfileSpec> {
            Err(crate::profiles::invalid_profile_fields(
                &["profile"],
                "downstream specification failed",
            ))
        }
    }

    #[test]
    fn builtin_view_identity_includes_the_declared_grants() {
        let profile = ProfileId::PtzOpticsG2
            .spec()
            .expect("built-in specification");
        assert!(PtzOpticsG2::validate_spec(&profile).is_ok());
        let error = MismatchedGrants::validate_spec(&profile).expect_err("grant mismatch");
        assert!(error
            .to_string()
            .contains("selected specification does not match"));
        assert!(MismatchedGrants::select(ProfileId::PtzOpticsG2.into()).is_err());
        assert!(PtzOpticsG2::select(ProfileId::SonyFr7.into()).is_err());
    }

    #[test]
    fn builtin_configuration_resolution_still_checks_downstream_specifications() {
        let selection = FailingSpecification::select(ProfileId::PtzOpticsG2.into())
            .expect("matching identity can be selected without rebuilding facts");
        let error = FailingSpecification::selected_spec(&selection)
            .expect_err("configuration must resolve the downstream specification");
        assert!(error
            .to_string()
            .contains("downstream specification failed"));
        let profile = ProfileId::PtzOpticsG2
            .spec()
            .expect("built-in specification");
        let selection = FailingSpecification::select(profile.into()).expect("matching identity");
        assert!(FailingSpecification::selected_spec(&selection).is_err());
    }
}

#[cfg(all(test, feature = "serde"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::profiles::PtzOpticsG2;

    fn custom_spec(name: &str) -> ProfileSpec {
        let mut value = serde_json::to_value(ProfileId::PtzOpticsG2.spec().unwrap()).unwrap();
        value["capabilities"]["profile_id"] = serde_json::Value::Null;
        value["capabilities"]["model_name"] = name.into();
        serde_json::from_value(value).unwrap()
    }

    struct CommunityProfile;

    impl CompileTimeProfile for CommunityProfile {
        const TYPED_SUPPORT: crate::capabilities::TypedSupportSet =
            <PtzOpticsG2 as CompileTimeProfile>::TYPED_SUPPORT;
        const ID: Option<ProfileId> = None;

        fn spec() -> crate::Result<ProfileSpec> {
            Ok(custom_spec("Community reference"))
        }
    }

    struct InvalidDeclaration;

    impl CompileTimeProfile for InvalidDeclaration {
        const TYPED_SUPPORT: crate::capabilities::TypedSupportSet =
            <PtzOpticsG2 as CompileTimeProfile>::TYPED_SUPPORT;
        const ID: Option<ProfileId> = None;

        fn spec() -> crate::Result<ProfileSpec> {
            ProfileId::PtzOpticsG2.spec()
        }
    }

    #[test]
    fn every_builtin_id_and_custom_object_keep_their_persisted_shape() {
        for id in ProfileId::all() {
            let json = serde_json::to_string(id).unwrap();
            let selected: ProfileSelection = serde_json::from_str(&json).unwrap();
            assert!(matches!(selected, ProfileSelection::Builtin(actual) if actual == *id));
            assert_eq!(selected.spec().unwrap(), id.spec().unwrap());
            assert_eq!(serde_json::to_string(&selected).unwrap(), json);
        }
        let spec = custom_spec("Community reference");
        let selected = ProfileSelection::from(spec.clone());
        let json = serde_json::to_value(&selected).unwrap();
        assert!(json.is_object());
        let loaded: ProfileSelection = serde_json::from_value(json.clone()).unwrap();
        assert!(matches!(loaded, ProfileSelection::Custom(_)));
        assert_eq!(loaded.spec().unwrap(), spec);
        assert_eq!(serde_json::to_value(loaded).unwrap(), json);
    }

    #[test]
    fn unknown_ids_name_the_refused_id_and_every_valid_registry_id() {
        let error = serde_json::from_str::<ProfileSelection>("\"unknown_camera\"")
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with(
                "unknown profile id `unknown_camera`: unknown variant `unknown_camera`"
            ),
            "{error}"
        );
        for id in ProfileId::all() {
            let json = serde_json::to_string(id).unwrap();
            assert!(error.contains(json.trim_matches('"')), "{error}");
        }
        for invalid in ["null", "false", "12", "[]", "{\"future\":true}"] {
            assert!(serde_json::from_str::<ProfileSelection>(invalid).is_err());
        }
    }

    #[test]
    fn typed_deserialization_compares_complete_custom_facts_and_declarations() {
        let spec = CommunityProfile::spec().unwrap();
        let json = serde_json::to_string(&spec).unwrap();
        let checked: TypedProfileSelection<CommunityProfile> = serde_json::from_str(&json).unwrap();
        assert_eq!(CommunityProfile::selected_spec(&checked).unwrap(), spec);
        assert_eq!(serde_json::to_string(&checked).unwrap(), json);
        // Both custom specs have ID None and the same typed grants. Their
        // different model fact must still reject the typed configuration.
        let different = serde_json::to_string(&custom_spec("Different community profile")).unwrap();
        let error = serde_json::from_str::<TypedProfileSelection<CommunityProfile>>(&different)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("selected specification does not match"),
            "{error}"
        );
        let error =
            serde_json::from_str::<TypedProfileSelection<InvalidDeclaration>>("\"ptzoptics_g2\"")
                .unwrap_err()
                .to_string();
        assert!(error.contains("CompileTimeProfile::ID"), "{error}");
    }

    #[test]
    fn deferred_typed_selections_resolve_checked_facts_without_profile_type_bounds() {
        let selection = TypedProfileSelection::<PtzOpticsG2>::deferred();
        assert_eq!(
            serde_json::to_string(&selection).unwrap(),
            "\"ptzoptics_g2\""
        );
        assert_eq!(
            PtzOpticsG2::selected_spec(&selection).unwrap(),
            ProfileId::PtzOpticsG2.spec().unwrap()
        );
        assert_eq!(format!("{:?}", selection.clone()), format!("{selection:?}"));
        let custom = TypedProfileSelection::<CommunityProfile>::deferred();
        assert_eq!(
            serde_json::to_value(&custom).unwrap(),
            serde_json::to_value(CommunityProfile::spec().unwrap()).unwrap()
        );
        let invalid = TypedProfileSelection::<InvalidDeclaration>::deferred();
        assert!(InvalidDeclaration::selected_spec(&invalid).is_err());
        assert!(serde_json::to_string(&invalid).is_err());
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn schema_uses_one_of_for_the_builtin_string_and_custom_object() {
        let schema = schemars::schema_for!(ProfileSelection);
        assert_eq!(schema.get("oneOf").unwrap().as_array().unwrap().len(), 2);
        assert!(schema.get("anyOf").is_none());
    }

    #[cfg(feature = "ts-rs")]
    #[test]
    fn typescript_uses_the_canonical_profile_union_for_both_profile_kinds() {
        let config = ts_rs::Config::default();
        let expected = "type ProfileSelection = ProfileId | ProfileSpec;";
        assert_eq!(<ProfileSelection as ts_rs::TS>::decl(&config), expected);
        assert_eq!(
            <TypedProfileSelection<CommunityProfile> as ts_rs::TS>::decl(&config),
            expected
        );
    }
}
