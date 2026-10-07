//! Profile-correct value domains for typed commands and inquiries (#819).

use grafton_visca::{
    capabilities::Capabilities,
    command::{Shutter, ShutterInquiry},
    profiles::{
        GenericVisca, NearusBRC300, PtzOpticsG2, SonyBRC300, SonyBRCH900, SonyEVIH100, SonyFR7,
    },
    types::ShutterSpeed,
    units::Fraction,
    Error, Inquiry, ProfileSpec, Request,
};

/// A shutter reply decodes to the code the profile's own source table
/// assigns: `05` is 1/30 s and `0F` 1/1000 s on EVI-H100 (R8), BRC-300 (R12),
/// Nearus BRC-300 (R21) and Generic VISCA. The shared Sony table these
/// profiles used before mapped 1/30 s to `00` and 1/1000 s to `05`.
#[test]
fn sony_shutter_replies_decode_to_their_source_table_codes() -> Result<(), Error> {
    for profile in [
        ProfileSpec::from_compile_time::<GenericVisca>()?,
        ProfileSpec::from_compile_time::<SonyEVIH100>()?,
        ProfileSpec::from_compile_time::<SonyBRC300>()?,
        ProfileSpec::from_compile_time::<NearusBRC300>()?,
    ] {
        for (reply, denominator) in [(0x05_u8, 30_u32), (0x0F, 1000), (0x15, 10000)] {
            let decoded = ShutterInquiry.decoder_for_profile(&profile).decode(&[
                0x00,
                0x00,
                reply >> 4,
                reply & 0x0F,
            ])?;
            assert_eq!(decoded.value(), reply, "{}", profile.name());
            assert_eq!(
                Some(decoded),
                profile
                    .capabilities()
                    .shutter_speed_for(Fraction::new(1, denominator).expect("nonzero denominator"))
                    .ok(),
                "{} 1/{denominator}",
                profile.name()
            );
        }
    }
    // BRC-H900's command list could not be read, so it advertises no codes.
    let brc_h900 = ProfileSpec::from_compile_time::<SonyBRCH900>()?;
    assert!(brc_h900.capabilities().shutter_speeds.is_empty());
    Ok(())
}

/// EVI-H100's table (R8) assigns `00` to 1/1 s, and no other code shares
/// that value, so a `00` shutter reply decodes through the typed inquiry to
/// the 1/1 s code.
#[test]
fn evi_h100_zero_shutter_reply_decodes_to_one_second() -> Result<(), Error> {
    let profile = ProfileSpec::from_compile_time::<SonyEVIH100>()?;
    let decoded = ShutterInquiry
        .decoder_for_profile(&profile)
        .decode(&[0x00, 0x00, 0x00, 0x00])?;
    assert_eq!(decoded.value(), 0x00);
    assert_eq!(
        profile
            .capabilities()
            .shutter_speed_for(Fraction::new(1, 1).expect("nonzero denominator"))
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
        Capabilities::from_profile::<GenericVisca>(),
        Capabilities::from_profile::<SonyEVIH100>(),
        Capabilities::from_profile::<PtzOpticsG2>(),
    ] {
        for entry in &caps.shutter_speeds {
            let code = ShutterSpeed::new(entry.value);
            assert!(
                matches!(Shutter::SetSpeed(code), Shutter::SetSpeed(c) if c.value() == entry.value)
            );
        }
    }
}

/// EVI-H100's iris table (R8 p. 44) and Bright table (p. 45) list `00` and
/// `05` upward but not `01`..`04`. Request validation refuses exactly those
/// gaps and admits both endpoints; BRC-300 lists every position.
#[test]
fn evi_h100_iris_and_bright_gaps_are_refused_and_endpoints_admitted() -> Result<(), Error> {
    use grafton_visca::{
        command::Brightness,
        request::builtin::IrisDirect,
        types::{BrightnessLevel, IrisLevel},
        Request,
    };

    let iris = |level: u8| IrisDirect::new(IrisLevel::new(level).expect("iris value"));
    let bright = |level: u8| Brightness::SetLevel(BrightnessLevel::new(level));

    let evi = ProfileSpec::from_compile_time::<SonyEVIH100>()?;
    for admitted in [0x00, 0x05, 0x11] {
        iris(admitted).validate_for_profile(&evi)?;
    }
    for admitted in [0x00, 0x05, 0x1F] {
        bright(admitted).validate_for_profile(&evi)?;
    }
    for gap in 0x01..=0x04 {
        assert!(
            matches!(
                iris(gap).validate_for_profile(&evi),
                Err(Error::InvalidParameter { .. })
            ),
            "iris {gap:#04x}"
        );
        assert!(
            matches!(
                bright(gap).validate_for_profile(&evi),
                Err(Error::InvalidParameter { .. })
            ),
            "bright {gap:#04x}"
        );
    }
    assert!(matches!(
        iris(0x12).validate_for_profile(&evi),
        Err(Error::ParameterOutOfRange { .. })
    ));
    assert!(matches!(
        bright(0x20).validate_for_profile(&evi),
        Err(Error::ParameterOutOfRange { .. })
    ));

    // Generic VISCA admits only the iris positions all its Sony sources list.
    let generic = ProfileSpec::from_compile_time::<GenericVisca>()?;
    assert!(iris(0x02).validate_for_profile(&generic).is_err());
    iris(0x05).validate_for_profile(&generic)?;

    let brc300 = ProfileSpec::from_compile_time::<SonyBRC300>()?;
    for level in 0x00..=0x11 {
        iris(level).validate_for_profile(&brc300)?;
    }
    for level in 0x00..=0x17 {
        bright(level).validate_for_profile(&brc300)?;
    }
    Ok(())
}

/// A domain persists with its gaps, and a malformed one is refused.
#[cfg(feature = "serde")]
#[test]
fn capability_domains_round_trip_and_reject_malformed_gaps() -> Result<(), Error> {
    use grafton_visca::capabilities::CapabilityDomain;

    let evi = ProfileSpec::from_compile_time::<SonyEVIH100>()?;
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
        ProfileSpec::from_compile_time::<SonyFR7>()?,
        ProfileSpec::from_compile_time::<SonyBRCH900>()?,
    ] {
        let name = profile.name().to_owned();
        assert!(
            profile.capabilities().shutter_speeds.is_empty(),
            "{name} shutter table"
        );
        for code in [0x00, 0x05, 0x0F, 0x15] {
            assert!(
                Shutter::SetSpeed(ShutterSpeed::new(code))
                    .validate_for_profile(&profile)
                    .is_err(),
                "{name} shutter code {code:#04x}"
            );
        }
    }
    Ok(())
}
