//! Profile-selected encoding families, generated from the typed vocabulary.

macro_rules! define_profile_encodings {
    ($($registry:tt)*) => {
        crate::__macro_support::__capability_encodings! { $($registry)* }
    };
}

super::typed_support_registry::typed_support_registry!(define_profile_encodings);

impl PresetRecallSpeedEncoding {
    /// Inclusive recall-speed values represented by this packet dialect.
    #[must_use]
    pub const fn speed_range(self) -> super::CapabilityRange<u8> {
        match self {
            Self::PtzOptics => super::CapabilityRange::<u8>::from_registry(1, 0x18),
            Self::SonyFr7 => super::CapabilityRange::<u8>::from_registry(1, 50),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{TypedSupportSet, TypedSupportSurface};

    #[test]
    fn selections_have_exactly_their_grant_and_named_encoding() {
        const ENCODINGS: ProfileEncodings = ProfileEncodings::EMPTY.with_encoding(
            EncodingSelection::PresetRecallSpeed(PresetRecallSpeedEncoding::PtzOptics),
        );
        const LOOKUP: EncodingSelection =
            crate::capabilities::encoding_selection!(PresetRecallSpeed(PtzOptics));
        assert_eq!(LOOKUP.surface(), TypedSupportSurface::PresetRecallSpeed);
        assert_eq!(ProfileEncodings::default(), ProfileEncodings::EMPTY);
        assert_eq!(
            ENCODINGS.preset_recall_speed(),
            Some(PresetRecallSpeedEncoding::PtzOptics)
        );
        assert_eq!(ENCODINGS.one_push_white_balance(), None);
        assert!(ENCODINGS
            .validate(TypedSupportSet::from_surface(
                TypedSupportSurface::PresetRecallSpeed
            ))
            .is_ok());
        assert!(ENCODINGS.validate(TypedSupportSet::EMPTY).is_err());
        assert!(ProfileEncodings::EMPTY
            .validate(TypedSupportSet::from_surface(
                TypedSupportSurface::PresetRecallSpeed
            ))
            .is_err());
        assert!(ProfileEncodings::EMPTY
            .validate(TypedSupportSet::EMPTY)
            .is_ok());
        let selection = EncodingSelection::from(OnePushWhiteBalanceEncoding::SonyFr7);
        assert!(ProfileEncodings::EMPTY
            .validate(TypedSupportSet::from_surface(
                TypedSupportSurface::OnePushWhiteBalance
            ))
            .is_err());
        assert!(ProfileEncodings::EMPTY
            .with_encoding(selection)
            .validate(TypedSupportSet::EMPTY)
            .is_err());
        assert_eq!(
            selection.surface(),
            TypedSupportSurface::OnePushWhiteBalance
        );
        assert_eq!(
            ProfileEncodings::EMPTY
                .with_encoding(selection)
                .iter()
                .collect::<Vec<_>>(),
            vec![selection]
        );
    }

    #[test]
    fn changing_one_family_preserves_the_other_selection() {
        let encodings = ProfileEncodings::EMPTY
            .with_encoding(PresetRecallSpeedEncoding::PtzOptics.into())
            .with_encoding(OnePushWhiteBalanceEncoding::Standard.into())
            .with_encoding(PresetRecallSpeedEncoding::SonyFr7.into());
        assert_eq!(
            encodings.preset_recall_speed(),
            Some(PresetRecallSpeedEncoding::SonyFr7)
        );
        assert_eq!(
            encodings.one_push_white_balance(),
            Some(OnePushWhiteBalanceEncoding::Standard)
        );
        assert_eq!(encodings.iter().count(), 2);
        assert!(encodings
            .validate(TypedSupportSet::from_surfaces(&[
                TypedSupportSurface::PresetRecallSpeed,
                TypedSupportSurface::OnePushWhiteBalance
            ]))
            .is_ok());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_pins_surface_keys_and_encoding_tags() -> Result<(), serde_json::Error> {
        assert_eq!(
            serde_json::to_value(ProfileEncodings::EMPTY)?,
            serde_json::json!({})
        );
        let encodings =
            ProfileEncodings::EMPTY.with_encoding(PresetRecallSpeedEncoding::PtzOptics.into());
        assert_eq!(
            serde_json::to_value(encodings)?,
            serde_json::json!({"preset_recall_speed":"ptzoptics"})
        );
        assert_eq!(
            serde_json::from_value::<ProfileEncodings>(
                serde_json::json!({"preset_recall_speed":"ptzoptics"})
            )?,
            encodings
        );
        assert_eq!(
            serde_json::to_value(OnePushWhiteBalanceEncoding::Standard)?,
            "standard"
        );
        assert_eq!(
            serde_json::to_value(OnePushWhiteBalanceEncoding::SonyFr7)?,
            "sony_fr7"
        );
        assert_eq!(
            serde_json::to_value(PresetRecallSpeedEncoding::SonyFr7)?,
            "sony_fr7"
        );
        for invalid in [
            r#"{"preset_recall_speed":"ptz_optics"}"#,
            r#"{"preset_recall_speed":"unknown"}"#,
            r#"{"preset_recall_speed":"standard"}"#,
            r#"{"unknown":"standard"}"#,
            r#"{"preset_recall_speed":"ptzoptics","preset_recall_speed":"sony_fr7"}"#,
        ] {
            assert!(
                serde_json::from_str::<ProfileEncodings>(invalid).is_err(),
                "accepted {invalid}"
            );
        }
        Ok(())
    }

    #[cfg(all(feature = "schemars", feature = "ts-rs"))]
    #[test]
    fn schemas_and_bindings_use_the_same_encoding_tags() {
        let schema = schemars::schema_for!(PresetRecallSpeedEncoding);
        let choices = schema
            .get("enum")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .or_else(|| {
                schema
                    .get("oneOf")
                    .and_then(serde_json::Value::as_array)
                    .map(|variants| {
                        variants
                            .iter()
                            .filter_map(|variant| variant.get("const").cloned())
                            .collect()
                    })
            });
        assert_eq!(
            choices,
            Some(vec![
                serde_json::json!("ptzoptics"),
                serde_json::json!("sony_fr7")
            ])
        );
        let bindings = <PresetRecallSpeedEncoding as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(bindings.contains("\"ptzoptics\""));
        assert!(bindings.contains("\"sony_fr7\""));
        let map = <ProfileEncodings as ts_rs::TS>::decl(&ts_rs::Config::default());
        assert!(map.contains("preset_recall_speed"));
        assert!(map.contains("one_push_white_balance"));
    }
}
