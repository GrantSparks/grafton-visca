//! Profile-correct value domains for typed commands and inquiries (#819).

use grafton_visca::{
    capabilities::Capabilities,
    command::{Shutter, ShutterInquiry},
    profiles::{GenericVisca, NearusBRC300, PtzOpticsG2, SonyBRC300, SonyBRCH900, SonyEVIH100},
    types::ShutterSpeed,
    units::Fraction,
    Error, Inquiry, ProfileSpec,
};

/// `GENERIC_VISCA_SHUTTER_SPEEDS` maps 1/30 to `0x00`. Before #819 the typed
/// shutter inquiry rejected that reply with `InvalidParameter`.
#[test]
fn generic_visca_zero_shutter_reply_decodes_through_the_typed_inquiry() -> Result<(), Error> {
    for profile in [
        ProfileSpec::from_compile_time::<GenericVisca>()?,
        ProfileSpec::from_compile_time::<SonyBRCH900>()?,
        ProfileSpec::from_compile_time::<SonyEVIH100>()?,
        ProfileSpec::from_compile_time::<SonyBRC300>()?,
        ProfileSpec::from_compile_time::<NearusBRC300>()?,
    ] {
        let decoded = ShutterInquiry
            .decoder_for_profile(&profile)
            .decode(&[0x00, 0x00, 0x00, 0x00])?;
        assert_eq!(decoded.value(), 0x00, "{}", profile.name());
        assert_eq!(
            Some(decoded),
            profile
                .capabilities()
                .shutter_speed_for(Fraction::new(1, 30).expect("nonzero denominator"))
                .ok(),
            "{}",
            profile.name()
        );
    }
    Ok(())
}

/// The typed shutter command can express every code a built-in profile
/// advertises, including the generic `0x00`.
#[test]
fn every_advertised_shutter_code_is_a_typed_shutter_command() {
    for caps in [
        Capabilities::from_profile::<GenericVisca>(),
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
