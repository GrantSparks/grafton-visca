//! Profile-correct value domains for typed commands and inquiries (#819).

use grafton_visca::{
    profiles::{
        GenericVisca, NearusBrc300, ProfileSpec, PtzOpticsG2, SonyBrc300, SonyBrcH900, SonyEviH100,
        SonyFr7,
    },
    request::{Inquiry, Request},
    types::{ExposureTime, ShutterSpeed},
    Error,
};

/// A shutter reply decodes to the code the profile's own source table
/// assigns: `05` is 1/30 s and `0F` 1/1000 s on EVI-H100 (R8), BRC-300 (R12),
/// Nearus BRC-300 (R21) and Generic VISCA. The shared Sony table these
/// profiles used before mapped 1/30 s to `00` and 1/1000 s to `05`.
#[test]
fn sony_shutter_replies_decode_to_their_source_table_codes() -> Result<(), Error> {
    for profile in [
        ProfileSpec::from_compile_time::<GenericVisca>()?,
        ProfileSpec::from_compile_time::<SonyEviH100>()?,
        ProfileSpec::from_compile_time::<SonyBrc300>()?,
        ProfileSpec::from_compile_time::<NearusBrc300>()?,
    ] {
        for (reply, denominator) in [(0x05_u8, 30_u32), (0x0F, 1000), (0x15, 10000)] {
            let decoded = grafton_visca::command::exposure::ShutterInquiry
                .decoder_for_profile(&profile)
                .decode(&[0x00, 0x00, reply >> 4, reply & 0x0F])?;
            assert_eq!(decoded.value(), reply, "{}", profile.name());
            assert_eq!(
                Some(decoded),
                profile
                    .capabilities()
                    .shutter_speed_for(
                        ExposureTime::new(1, denominator).expect("nonzero denominator")
                    )
                    .ok(),
                "{} 1/{denominator}",
                profile.name()
            );
        }
    }
    // BRC-H900 currently registers no model-specific shutter-code inventory.
    let brc_h900 = ProfileSpec::from_compile_time::<SonyBrcH900>()?;
    assert!(brc_h900
        .capabilities()
        .exposure
        .as_ref()
        .expect("exposure domain")
        .shutter_speeds
        .is_empty());
    Ok(())
}

/// EVI-H100's table (R8) assigns `00` to 1/1 s, and no other code shares
/// that value, so a `00` shutter reply decodes through the typed inquiry to
/// the 1/1 s code.
#[test]
fn evi_h100_zero_shutter_reply_decodes_to_one_second() -> Result<(), Error> {
    let profile = ProfileSpec::from_compile_time::<SonyEviH100>()?;
    let decoded = grafton_visca::command::exposure::ShutterInquiry
        .decoder_for_profile(&profile)
        .decode(&[0x00, 0x00, 0x00, 0x00])?;
    assert_eq!(decoded.value(), 0x00);
    assert_eq!(
        profile
            .capabilities()
            .shutter_speed_for(ExposureTime::new(1, 1).expect("nonzero denominator"))
            .ok(),
        Some(decoded)
    );
    Ok(())
}

/// The typed shutter command can express every code a built-in profile
/// advertises, including EVI-H100's `00` (1/1 s).
#[test]
fn every_advertised_shutter_code_is_a_typed_shutter_command() {
    for caps in [
        grafton_visca::profiles::ProfileSpec::from_compile_time::<GenericVisca>()
            .expect("validated profile")
            .capabilities()
            .clone(),
        grafton_visca::profiles::ProfileSpec::from_compile_time::<SonyEviH100>()
            .expect("validated profile")
            .capabilities()
            .clone(),
        grafton_visca::profiles::ProfileSpec::from_compile_time::<PtzOpticsG2>()
            .expect("validated profile")
            .capabilities()
            .clone(),
    ] {
        for entry in &caps
            .exposure
            .as_ref()
            .expect("exposure domain")
            .shutter_speeds
        {
            let code = ShutterSpeed::new(entry.value).expect("valid value");
            let request = grafton_visca::command::exposure::SetShutter::new(code);
            assert_eq!(request.speed().value(), entry.value);
        }
    }
}

/// EVI-H100's iris table (R8 p. 44) and Bright table (p. 45) list `00` and
/// `05` upward but not `01`..`04`. Request validation refuses exactly those
/// gaps and admits both endpoints; BRC-300 lists every position.
#[test]
fn evi_h100_iris_and_bright_gaps_are_refused_and_endpoints_admitted() -> Result<(), Error> {
    use grafton_visca::{
        request::Request,
        types::{BrightnessLevel, IrisLevel},
    };

    let iris = |level: u8| {
        grafton_visca::command::exposure::SetIris::new(IrisLevel::new(level).expect("iris value"))
    };
    let bright = |level: u8| {
        grafton_visca::command::exposure::SetBrightness::new(
            BrightnessLevel::new(level).expect("valid value"),
        )
    };

    let evi = ProfileSpec::from_compile_time::<SonyEviH100>()?;
    for admitted in [0x00, 0x05, 0x11] {
        iris(admitted).validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                evi.clone(),
            )
            .expect("target registration"),
        )?;
    }
    for admitted in [0x00, 0x05, 0x1F] {
        bright(admitted).validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                evi.clone(),
            )
            .expect("target registration"),
        )?;
    }
    for gap in 0x01..=0x04 {
        assert!(
            matches!(
                iris(gap).validate_for_target(
                    &grafton_visca::config::TargetRegistration::new(
                        grafton_visca::types::CameraId::CAMERA_1,
                        evi.clone()
                    )
                    .expect("target registration")
                ),
                Err(Error::InvalidParameter { .. })
            ),
            "iris {gap:#04x}"
        );
        assert!(
            matches!(
                bright(gap).validate_for_target(
                    &grafton_visca::config::TargetRegistration::new(
                        grafton_visca::types::CameraId::CAMERA_1,
                        evi.clone()
                    )
                    .expect("target registration")
                ),
                Err(Error::InvalidParameter { .. })
            ),
            "bright {gap:#04x}"
        );
    }
    assert!(matches!(
        iris(0x12).validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                evi.clone()
            )
            .expect("target registration")
        ),
        Err(Error::ParameterOutOfRange { .. })
    ));
    assert!(matches!(
        bright(0x20).validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                evi.clone()
            )
            .expect("target registration")
        ),
        Err(Error::ParameterOutOfRange { .. })
    ));

    // Generic VISCA admits only the iris positions all its Sony sources list.
    let generic = ProfileSpec::from_compile_time::<GenericVisca>()?;
    assert!(iris(0x02)
        .validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                generic.clone()
            )
            .expect("target registration")
        )
        .is_err());
    iris(0x05).validate_for_target(
        &grafton_visca::config::TargetRegistration::new(
            grafton_visca::types::CameraId::CAMERA_1,
            generic.clone(),
        )
        .expect("target registration"),
    )?;

    let brc300 = ProfileSpec::from_compile_time::<SonyBrc300>()?;
    for level in 0x00..=0x11 {
        iris(level).validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                brc300.clone(),
            )
            .expect("target registration"),
        )?;
    }
    for level in 0x00..=0x17 {
        bright(level).validate_for_target(
            &grafton_visca::config::TargetRegistration::new(
                grafton_visca::types::CameraId::CAMERA_1,
                brc300.clone(),
            )
            .expect("target registration"),
        )?;
    }
    Ok(())
}

/// A domain persists with its gaps, and a malformed one is refused.
#[cfg(feature = "serde")]
#[test]
fn capability_domains_round_trip_and_reject_malformed_gaps() -> Result<(), Error> {
    use grafton_visca::capabilities::CapabilityDomain;

    let evi = ProfileSpec::from_compile_time::<SonyEviH100>()?;
    let json = serde_json::to_string(&evi).expect("serialize");
    assert!(json.contains(r#""gaps":[1,2,3,4]"#), "{json}");
    let restored: ProfileSpec = serde_json::from_str(&json).expect("current shape loads");
    assert_eq!(restored, evi);

    for malformed in [
        r#"{"min":0,"max":17,"gaps":[0]}"#,
        r#"{"min":0,"max":17,"gaps":[17]}"#,
        r#"{"min":0,"max":17,"gaps":[4,2]}"#,
        r#"{"min":0,"max":17,"gaps":[2,2]}"#,
        r#"{"min":18,"max":17,"gaps":[]}"#,
    ] {
        assert!(
            serde_json::from_str::<CapabilityDomain<u16>>(malformed).is_err(),
            "{malformed}"
        );
    }
    Ok(())
}

/// Known unverified: the FR7 (R7) and BRC-H900 (R11) shutter tables could not
/// be read, so neither profile advertises a shutter code and every typed
/// shutter position is refused before any I/O.
#[test]
fn fr7_and_brc_h900_advertise_no_shutter_codes() -> Result<(), Error> {
    for profile in [
        ProfileSpec::from_compile_time::<SonyFr7>()?,
        ProfileSpec::from_compile_time::<SonyBrcH900>()?,
    ] {
        let name = profile.name().to_owned();
        assert!(
            profile
                .capabilities()
                .exposure
                .as_ref()
                .expect("exposure domain")
                .shutter_speeds
                .is_empty(),
            "{name} shutter table"
        );
        for code in [0x00, 0x05, 0x0F, 0x15] {
            assert!(
                grafton_visca::command::exposure::SetShutter::new(
                    ShutterSpeed::new(code).expect("valid value")
                )
                .validate_for_target(
                    &grafton_visca::config::TargetRegistration::new(
                        grafton_visca::types::CameraId::CAMERA_1,
                        profile.clone()
                    )
                    .expect("target registration")
                )
                .is_err(),
                "{name} shutter code {code:#04x}"
            );
        }
    }
    Ok(())
}
