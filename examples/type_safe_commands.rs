//! Type-safe profile and capability example.
//!
//! This example does not connect to a camera. It demonstrates the parts of the
//! public API that can be checked before any I/O happens: profile metadata,
//! range validation, protocol envelope selection, and optional capability bounds.
//! It complements the command builder type-state pattern used to guarantee
//! terminator-safe VISCA byte construction around `VISCA_TERMINATOR`.
//!
//! Run with:
//! ```sh
//! cargo run --example type_safe_commands
//! ```

use grafton_visca::{
    capabilities::{
        HasExposureCompensation, HasFocusLock, HasNdFilter, HasPanTiltSpeedMode, HasPushAutoFocus,
        TypedSupportSurface,
    },
    profiles::{CompileTimeProfile, GenericVisca, PtzOpticsG2, SonyFr7},
    types::{GainLevel, Kelvin, UnitInterval},
    wire::WireValue,
    Error,
};

fn main() -> Result<(), Error> {
    print_profile::<PtzOpticsG2>()?;
    print_profile::<SonyFr7>()?;
    print_profile::<GenericVisca>()?;
    demonstrate_value_validation()?;
    demonstrate_optional_capability_bounds()?;
    Ok(())
}

fn print_profile<P: CompileTimeProfile>() -> Result<(), Error> {
    let profile = P::spec()?;
    let caps = profile.capabilities();
    println!("{}", profile.name());
    println!("  TCP default port: {:?}", profile.transports().tcp_port());
    println!("  UDP default port: {:?}", profile.transports().udp_port());
    println!("  Inquiry support: {:?}", caps.inquiry_support);
    if let Some(pan_tilt) = caps.pan_tilt.and_then(|domain| domain.geometry) {
        println!("  Pan range: {:?} VISCA units", pan_tilt.pan_range);
        println!(
            "  Tilt range: {:?} VISCA units",
            pan_tilt.tilt_ranges.range(None)
        );
    }
    if let Some(zoom) = &caps.zoom {
        println!("  Optical zoom max: 0x{:04X}", zoom.optical_max);
        println!("  Digital zoom max: {:?}", zoom.digital_max);
    }
    if let Some(exposure) = &caps.exposure {
        println!("  Exposure modes: {}", exposure.modes.len());
    }
    if let Some(white_balance) = &caps.white_balance {
        println!("  White-balance modes: {}", white_balance.modes.len());
    }
    if let Some(presets) = &caps.presets {
        println!("  Presets: 0..={}", presets.highest);
    }
    println!(
        "  Typed ND filter: {}",
        caps.supports(TypedSupportSurface::NdFilter)
    );
    println!();
    Ok(())
}

fn demonstrate_value_validation() -> Result<(), Error> {
    println!("Validation");
    let half = UnitInterval::new(0.5)?;
    println!("  Checked normalized value: {}", half.value());
    println!(
        "  Normalized domain: {}..={}",
        UnitInterval::MIN.value(),
        UnitInterval::MAX.value()
    );

    let gain = GainLevel::new(5)?;
    println!("  Gain wire value: 0x{:02X}", gain.to_wire());
    let temperature = Kelvin::new(3_200)?;
    println!("  Color temperature: {} K", temperature.value());
    // Value construction validates the union domain. The selected profile
    // validates its narrower supported range during request preparation.

    match UnitInterval::new(1.25) {
        Ok(value) => println!("  Unexpected zoom validation success: {value:?}"),
        Err(error) => println!("  Out-of-range zoom rejected: {error}"),
    }

    println!();
    Ok(())
}

fn demonstrate_optional_capability_bounds() -> Result<(), Error> {
    println!("Optional capability bounds");

    requires_focus_lock::<PtzOpticsG2>()?;
    requires_push_af::<SonyFr7>()?;
    requires_nd_filter::<SonyFr7>()?;
    requires_variable_speed::<SonyFr7>()?;
    requires_exposure_compensation::<PtzOpticsG2>()?;

    // These lines intentionally do not compile if uncommented:
    // requires_nd_filter::<PtzOpticsG2>();
    // requires_variable_speed::<PtzOpticsG2>();
    // requires_push_af::<PtzOpticsG2>();
    // requires_focus_lock::<SonyFr7>();
    // requires_exposure_compensation::<SonyFr7>(); // FR7 uses its own AE level family.
    Ok(())
}

fn requires_focus_lock<P: CompileTimeProfile + HasFocusLock>() -> Result<(), Error> {
    println!("  {} supports focus lock", P::spec()?.name());
    Ok(())
}

fn requires_nd_filter<P: CompileTimeProfile + HasNdFilter>() -> Result<(), Error> {
    println!("  {} supports typed ND filter control", P::spec()?.name());
    Ok(())
}

fn requires_variable_speed<P: CompileTimeProfile + HasPanTiltSpeedMode>() -> Result<(), Error> {
    println!("  {} supports variable speed mode", P::spec()?.name());
    Ok(())
}

fn requires_push_af<P: CompileTimeProfile + HasPushAutoFocus>() -> Result<(), Error> {
    println!("  {} supports Push AF", P::spec()?.name());
    Ok(())
}

fn requires_exposure_compensation<P: CompileTimeProfile + HasExposureCompensation>(
) -> Result<(), Error> {
    println!("  {} supports exposure compensation", P::spec()?.name());
    Ok(())
}
