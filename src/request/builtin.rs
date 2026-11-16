//! Homogeneous built-in typed requests.
//!
//! This module owns one typed request for each closed semantic class while the
//! command enums continue to represent wire values. Individual variants are
//! exposed here with one request class each.

use crate::{
    capabilities::TypedSupportSurface,
    command::{
        encode::WireEncode,
        semantics::{BuiltinCommand, BuiltinRequestContract},
        surface::{row_gate, surface_entry},
        Focus, Iris, NdFilterDensity, NdFilterStep, NdFilterStepCommand, PanTilt, PanTiltDirection,
        PanTiltLimitCorner, PresetCommand, PresetNumber, PushAF, Zoom,
    },
    completion,
    noun_table::noun_table,
    request, AffectedAxes, CameraId, ControlClass, Error, OperationCommand, Request, RetryClass,
    TimeoutClass,
};
use crate::{command::preset_wire::PresetAction, wire::WireValue};
use std::borrow::Cow;

#[cfg(test)]
std::thread_local! {
    static REQUEST_WRITE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) fn reset_request_write_count() {
    REQUEST_WRITE_COUNT.with(|count| count.set(0));
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) fn request_write_count() -> usize {
    REQUEST_WRITE_COUNT.with(std::cell::Cell::get)
}
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
use crate::types::SpeedLevel;
use crate::types::{FocusPosition, IrisLevel, PanSpeed, TiltSpeed, ZoomPosition, ZoomSpeed};
use crate::{
    capabilities::PanTiltWireCodec,
    command::pan_tilt_wire::{MoveSpeeds, PanTiltFraming, PanTiltProfiled},
};
use crate::{units::Degrees, PanTiltCoordinateConversion};

/// Implements the typed request contract of built-in request types from
/// their ledger rows.
///
/// One entry per request type:
///
/// ```text
/// <type> => <BuiltinCommand row>: <Plain | Targeted | AppliedOnly> [(profile_axes)] {
///     size: <MAX_SIZE>
///     [, policy: (<TimeoutClass>, <RetryClass>, <ControlClass>)]
///     [, wire: <closure from &type to its wire value>]
///     [, rows: |<value>| match <value>[.<field>] { <Variant path> => <BuiltinCommand row>, ... }]
/// };
/// ```
///
/// The row is the type's one tie to the semantic ledger: the class is
/// checked against it at compile time, and fixed operation axes, the
/// write-only state effect and the typed capability gate are all read from
/// it. `size` stays explicit because the allocation bound must be a
/// constant; the exact encoded length is measured from the encoder. Without
/// `policy`, a plain request is `(Quick, Standard, Normal)` and an operation
/// `(Movement, Movement, User)`; without `wire`, the type is its own wire
/// encoder.
///
/// `profile_axes` marks an operation whose axes come from the profile, so it
/// implements `OperationCommand` itself.
///
/// `rows` is for a type whose values serve different ledger rows. It
/// generates the inherent `const fn ledger_row(&self)`, an exhaustive match
/// with no repeated pattern. Each pattern is
/// a variant path, optionally with tuple or named fields, so a top-level wildcard or
/// binding cannot absorb a variant added later. The generated request hook
/// gates each value on the row it selects. Every row then classifies like
/// the entry's row. Noun rows with the same class and capability gate can send
/// a value-selecting type; constant `by_value` rows additionally prove their
/// exact selected row at compile time.
macro_rules! builtin_request {
    (@class Plain) => { request::PlainClass };
    (@class Targeted) => { request::OperationClass<completion::Targeted> };
    (@class AppliedOnly) => { request::OperationClass<completion::AppliedOnly> };

    (@policy $class:ident ($timeout:ident, $retry:ident, $control:ident)) => {
        (TimeoutClass::$timeout, RetryClass::$retry, ControlClass::$control)
    };
    (@policy Plain) => {
        (TimeoutClass::Quick, RetryClass::Standard, ControlClass::Normal)
    };
    (@policy $operation:ident) => {
        (TimeoutClass::Movement, RetryClass::Movement, ControlClass::User)
    };

    (@row $this:ident) => { Self::LEDGER_ROW };
    (@row $this:ident, $value:ident) => { $this.ledger_row() };

    (@row_inventory $type:ty, $row:ident, []) => { &[BuiltinCommand::$row] };
    (@row_inventory $type:ty, $row:ident, [$value:ident]) => { <$type>::VALUE_SELECTED_LEDGER_ROWS };

    (@wire $this:tt) => { *$this };
    (@wire $this:tt, $wire:expr) => { ($wire)($this) };

    (@target_wire $this:expr, $target:expr, $buffer:expr) => {
        <Self as Request>::write_into($this, $target.target(), $buffer)
    };
    (@target_wire $this:expr, $target:expr, $buffer:expr, $wire:expr) => {
        ($wire)($this, $target, $buffer)
    };

    (@contract $type:ty, $row:ident [$(profile_axes)?] [$($by_value:ident)?]) => {
        impl BuiltinRequestContract for $type {
            const LEDGER_ROW: BuiltinCommand = BuiltinCommand::$row;
            #[cfg(test)]
            fn selected_ledger_row(&self) -> BuiltinCommand {
                builtin_request!(@row self $(, $by_value)?)
            }
            const LEDGER_ROWS: &'static [BuiltinCommand] = builtin_request!(@row_inventory $type, $row, [$($by_value)?]);
            type State = <crate::command::semantics::LedgerRow<{ BuiltinCommand::$row as usize }> as crate::command::semantics::LedgerState>::State;
        }
    };
    (@contract $type:ty, $row:ident [$flag:ident] $by_value:tt) => {
        compile_error!(concat!(
            "unknown builtin_request! flag `",
            stringify!($flag),
            "`; expected profile_axes"
        ));
    };
    (@rows $type:ty, |$value:ident| ($scrutinee:expr) {
        $($pattern:ident $(::$segment:ident)* $(($($field:pat),*))? $({$($named:tt)*})? => $row:ident),+
    }) => {
        $(const _: () = assert!(
            BuiltinCommand::$row.classification().same_contract(<$type as BuiltinRequestContract>::LEDGER_ROW.classification()),
            "a selected request row changes its class, axes or state-effect contract",
        );)+
        impl $type {
            pub(crate) const VALUE_SELECTED_LEDGER_ROWS: &'static [BuiltinCommand] = &[$(BuiltinCommand::$row),+];
            /// The ledger row this value serves, whose typed capability gate
            /// admits it.
            #[deny(unreachable_patterns)]
            pub(crate) const fn ledger_row(&self) -> BuiltinCommand {
                let $value = self;
                match $scrutinee {
                    $( $pattern $(::$segment)* $(($($field),*))? $({$($named)*})? => BuiltinCommand::$row, )+
                }
            }
        }
    };

    (@operation $type:ty, Plain, profile_axes) => {
        compile_error!(concat!(
            "builtin_request! flag `profile_axes` on Plain request `",
            stringify!($type),
            "`; only an operation has affected axes"
        ));
    };
    (@operation $type:ty, Plain) => {};
    (@operation $type:ty, $class:ident, profile_axes) => {};
    (@operation $type:ty, $class:ident) => {
        impl OperationCommand<completion::$class> for $type {
            fn affected_axes(&self) -> AffectedAxes {
                const { crate::command::semantics::fixed_axes::<$type>() }
            }
        }
    };
    // An unknown flag is reported once, by the `@contract` arm.
    (@operation $type:ty, $class:ident, $flag:ident) => {};

    ($(
        $type:ty => $row:ident: $class:ident $(($flag:ident))? {
            size: $size:expr
            $(, policy: ($timeout:ident, $retry:ident, $control:ident))?
            $(, wire: $wire:expr)?
            $(, target_wire: $target_wire:expr)?
            $(, rows: |$value:ident| match ($scrutinee:expr) {
                $($pattern:ident $(::$segment:ident)* $(($($field:pat),*))? $({$($named:tt)*})? => $value_row:ident),+ $(,)?
            })?
            $(,)?
        };
    )+) => {$(
        impl Request for $type {
            type Class = builtin_request!(@class $class);

            const MAX_SIZE: usize = $size;
            const TIMEOUT_CLASS: TimeoutClass =
                builtin_request!(@policy $class $(($timeout, $retry, $control))?).0;
            const RETRY_CLASS: RetryClass =
                builtin_request!(@policy $class $(($timeout, $retry, $control))?).1;
            const CONTROL_CLASS: ControlClass =
                builtin_request!(@policy $class $(($timeout, $retry, $control))?).2;

            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                #[cfg(test)]
                REQUEST_WRITE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
                WireEncode::write_into(
                    &builtin_request!(@wire self $(, $wire)?),
                    camera_id,
                    buffer,
                )
            }

            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn write_for_target(&self, target: &crate::TargetRegistration, buffer: &mut [u8], _authority: crate::requests::RequestContractAuthority) -> Result<usize, Error> {
                let written = builtin_request!(@target_wire self, target, buffer $(, $target_wire)?)?;
                let row = builtin_request!(@row self $(, $value)?);
                // One bounded byte/mask comparison keeps selector defence in
                // depth without reparsing or walking per-reference packet text.
                if target.profile().capabilities().profile_id.is_some() {
                    if let Some(packet) = row.packet() {
                        packet.validate_frame(&buffer[..written], surface_entry(row).feature())?;
                    }
                }
                Ok(written)
            }

            fn validate_for_target(&self, target: &crate::TargetRegistration) -> Result<(), Error> {
                let row = builtin_request!(@row self $(, $value)?);
                row_gate(row).validate(target.profile(), surface_entry(row).feature())?;
                row.source().validate_for_profile(target.profile(), surface_entry(row).feature())?;
                <Self as BuiltinValidation>::validate_target(self, target)
            }

            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn pan_tilt_max_speed(&self, _authority: crate::requests::RequestContractAuthority) -> Option<u8> {
                <Self as BuiltinValidation>::pan_tilt_max_speed(self)
            }

            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn admission_control_class(
                &self,
                _authority: crate::requests::RequestContractAuthority,
            ) -> Result<ControlClass, Error> {
                Ok(Self::CONTROL_CLASS)
            }

            #[cfg(any(feature = "blocking", feature = "async"))]
            #[allow(private_interfaces)]
            fn applied_state_projection(
                &self,
                _authority: crate::requests::AppliedStateAuthority,
            ) -> Option<crate::runtime::engine::AppliedStateProjection> {
                <<Self as BuiltinRequestContract>::State as BuiltinStateProjection<Self>>::project(self)
            }
        }

        builtin_request!(@contract $type, $row [$($flag)?] [$($value)?]);
        // Evaluated by `cargo check`: fails when the class disagrees with the row.
        const _: () = <$type as BuiltinRequestContract>::CLASS_MATCHES_ROW;
        builtin_request!(@operation $type, $class $(, $flag)?);
        $( builtin_request!(@rows $type, |$value| ($scrutinee) {
            $($pattern $(::$segment)* $(($($field),*))? $({$($named)*})? => $value_row),+
        }); )?
    )+};
}

pub(crate) use super::builtin_contract::BuiltinValidation;
#[cfg(any(feature = "blocking", feature = "async"))]
use super::builtin_contract::{BuiltinStateProjection, BuiltinStateValue};

fn require(supported: bool, feature: &'static str) -> Result<(), Error> {
    if supported {
        Ok(())
    } else {
        Err(Error::FeatureNotSupported { feature })
    }
}

/// Rejects `value` outside the profile's contiguous `range`, reporting the
/// range's bounds through the crate's single out-of-range error.
fn require_in_range<T>(
    parameter: &'static str,
    value: T,
    range: &crate::capabilities::CapabilityRange<T>,
) -> Result<(), Error>
where
    T: Copy + PartialOrd + Into<i32>,
{
    if value >= range.min() && value <= range.max() {
        Ok(())
    } else {
        Err(Error::parameter_out_of_range(
            parameter,
            value.into(),
            range.min().into(),
            range.max().into(),
        ))
    }
}

/// Rejects `value` that the profile's source table does not list: a value
/// outside the bounds through [`require_in_range`], and a gap inside them as
/// an invalid parameter. [`CapabilityDomain`]'s admission rule decides both.
///
/// [`CapabilityDomain`]: crate::capabilities::CapabilityDomain
fn require_in_domain<T>(
    parameter: &'static str,
    value: T,
    domain: &crate::capabilities::CapabilityDomain<T>,
) -> Result<(), Error>
where
    T: Copy + PartialOrd + Into<i32> + std::fmt::LowerHex + 'static,
{
    match domain.admission(value) {
        crate::capabilities::DomainAdmission::Admitted => Ok(()),
        crate::capabilities::DomainAdmission::OutOfBounds => {
            require_in_range(parameter, value, &domain.bounds())
        }
        crate::capabilities::DomainAdmission::Gap => Err(Error::InvalidParameter {
            parameter,
            value: Cow::Owned(format!("{value:#04x}")),
            reason: Cow::Borrowed(crate::capabilities::DOMAIN_GAP),
        }),
    }
}

fn pan_tilt_facts(
    profile: &crate::ProfileSpec,
) -> Result<&crate::capabilities::PanTiltCaps, Error> {
    profile
        .capabilities()
        .pan_tilt
        .as_ref()
        .ok_or(Error::FeatureNotSupported {
            feature: "pan/tilt",
        })
}

fn zoom_facts(profile: &crate::ProfileSpec) -> Result<&crate::capabilities::ZoomCaps, Error> {
    profile
        .capabilities()
        .zoom
        .as_ref()
        .ok_or(Error::FeatureNotSupported { feature: "zoom" })
}

fn focus_facts(profile: &crate::ProfileSpec) -> Result<&crate::capabilities::FocusCaps, Error> {
    profile
        .capabilities()
        .focus
        .as_ref()
        .ok_or(Error::FeatureNotSupported { feature: "focus" })
}

fn exposure_facts(
    profile: &crate::ProfileSpec,
) -> Result<&crate::capabilities::ExposureCaps, Error> {
    profile
        .capabilities()
        .exposure
        .as_ref()
        .ok_or(Error::FeatureNotSupported {
            feature: "exposure",
        })
}

fn white_balance_facts(
    profile: &crate::ProfileSpec,
) -> Result<&crate::capabilities::WhiteBalanceCaps, Error> {
    profile
        .capabilities()
        .white_balance
        .as_ref()
        .ok_or(Error::FeatureNotSupported {
            feature: "white balance",
        })
}

fn presets_facts(profile: &crate::ProfileSpec) -> Result<&crate::capabilities::PresetCaps, Error> {
    profile
        .capabilities()
        .presets
        .as_ref()
        .ok_or(Error::FeatureNotSupported { feature: "presets" })
}

fn validate_pan_tilt_speed(
    profile: &crate::ProfileSpec,
    pan: PanSpeed,
    tilt: TiltSpeed,
) -> Result<(), Error> {
    let facts = pan_tilt_facts(profile)?;
    require_in_range("pan speed", pan.value(), &facts.pan_speed)?;
    require_in_range("tilt speed", tilt.value(), &facts.tilt_speed)
}

/// Validates paired speeds for a position command whose profile may carry a
/// one-speed wire form.
fn validate_pan_tilt_position_speed(
    profile: &crate::ProfileSpec,
    pan: PanSpeed,
    tilt: TiltSpeed,
) -> Result<MoveSpeeds, Error> {
    validate_pan_tilt_speed(profile, pan, tilt)?;
    let conversion = profile
        .pan_tilt_coordinates()
        .ok_or(Error::FeatureNotSupported {
            feature: "pan/tilt coordinate conversion",
        })?;
    PanTiltFraming::for_conversion(conversion).move_speeds(pan, tilt)
}

fn validate_move_speeds(profile: &crate::ProfileSpec, speeds: MoveSpeeds) -> Result<(), Error> {
    let facts = pan_tilt_facts(profile)?;
    let (pan, tilt) = speeds.values();
    require_in_range("pan speed", pan, &facts.pan_speed)?;
    require_in_range("tilt speed", tilt, &facts.tilt_speed)?;
    let current = PanTiltFraming::for_conversion(facts.coordinates)
        .move_speeds(PanSpeed::new(pan)?, TiltSpeed::new(tilt)?)?;
    if current != speeds {
        return Err(Error::InvalidRequest {
            reason: "position speeds belong to another wire grammar".into(),
        });
    }
    Ok(())
}

/// Lowers one coarse position speed through the profile-owned position grammar.
///
/// Standard VISCA keeps [`crate::types::SpeedLevel`]'s asymmetric pan/tilt mapping. The
/// profile still owns the usable maximum for each axis, so a coarse level such
/// as [`crate::types::SpeedLevel::Fastest`] means that profile's fastest valid speed rather
/// than an unconditional `24`/`20` pair. Sony BRC-300 position frames carry
/// one `VV` byte, so both public speed wrappers deliberately receive one value
/// from the intersection of the profile's pan and tilt ranges before the
/// paired-speed validator runs.
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
fn pan_tilt_position_speeds_from_level(
    speed: SpeedLevel,
    profile: &crate::ProfileSpec,
) -> Result<(PanSpeed, TiltSpeed), Error> {
    let facts = pan_tilt_facts(profile)?;
    if facts.coordinates.wire_codec() == PanTiltWireCodec::SonyBrc300 {
        let speed = speed.baseline_speeds().0.clamp(
            facts.pan_speed.min().max(facts.tilt_speed.min()),
            facts.pan_speed.max().min(facts.tilt_speed.max()),
        );
        Ok((PanSpeed::new(speed)?, TiltSpeed::new(speed)?))
    } else {
        Ok((
            PanSpeed::new(
                speed
                    .baseline_speeds()
                    .0
                    .clamp(facts.pan_speed.min(), facts.pan_speed.max()),
            )?,
            TiltSpeed::new(
                speed
                    .baseline_speeds()
                    .1
                    .clamp(facts.tilt_speed.min(), facts.tilt_speed.max()),
            )?,
        ))
    }
}

fn validate_power_state(profile: &crate::ProfileSpec, standby: bool) -> Result<(), Error> {
    if standby {
        require(
            profile
                .capabilities()
                .power
                .as_ref()
                .is_some_and(|domain| domain.standby.is_some()),
            "standby power control",
        )?;
    }
    Ok(())
}

fn validate_exposure_mode(
    profile: &crate::ProfileSpec,
    mode: crate::command::ExposureMode,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();

    require(
        capabilities
            .exposure
            .as_ref()
            .is_some_and(|domain| domain.modes.contains(&mode)),
        crate::command::exposure_wire::SHARED_EXPOSURE_MODE_FEATURE,
    )?;
    Ok(())
}

fn validate_exposure_compensation(
    profile: &crate::ProfileSpec,
    value: Option<i8>,
) -> Result<(), Error> {
    if let Some(value) = value {
        let range = exposure_facts(profile)?.compensation_range.as_ref().ok_or(
            Error::FeatureNotSupported {
                feature: "exposure compensation range",
            },
        )?;
        require_in_range("exposure compensation", value, range)?;
    }
    Ok(())
}

fn validate_shutter(profile: &crate::ProfileSpec, value: Option<u8>) -> Result<(), Error> {
    if let Some(value) = value {
        if !exposure_facts(profile)?
            .shutter_speeds
            .iter()
            .any(|speed| speed.value == value)
        {
            return Err(Error::invalid_parameter(
                "shutter speed",
                Cow::Owned(value.to_string()),
                Cow::Borrowed("code is not in the profile's shutter table"),
            ));
        }
    }
    Ok(())
}

fn validate_brightness(profile: &crate::ProfileSpec, value: Option<u8>) -> Result<(), Error> {
    if let Some(value) = value {
        let range = exposure_facts(profile)?.brightness_range.as_ref().ok_or(
            Error::FeatureNotSupported {
                feature: "brightness range",
            },
        )?;
        require_in_domain("brightness level", value, range)?;
    }
    Ok(())
}

fn validate_gain(profile: &crate::ProfileSpec, value: Option<u8>) -> Result<(), Error> {
    let range = exposure_facts(profile)?
        .gain_range
        .as_ref()
        .ok_or(Error::FeatureNotSupported {
            feature: "shared gain control",
        })?;
    if let Some(value) = value {
        require_in_range("gain level", value, range)?;
    }
    Ok(())
}

/// Checks membership in the profile's white-balance mode inventory after
/// the generated request hook has checked the selected row's gate.
fn validate_white_balance_mode(
    profile: &crate::ProfileSpec,
    command: &crate::command::WhiteBalanceCommand,
) -> Result<(), Error> {
    require(
        white_balance_facts(profile)?.modes.contains(&command.mode),
        "selected white-balance mode",
    )
}

fn validate_tuning(profile: &crate::ProfileSpec, value: i8, red: bool) -> Result<(), Error> {
    let tuning =
        white_balance_facts(profile)?
            .rgb_tuning
            .as_ref()
            .ok_or(Error::FeatureNotSupported {
                feature: "RGB tuning range",
            })?;
    require_in_range(
        "RGB tuning",
        value,
        if red { &tuning.red } else { &tuning.blue },
    )
}

fn validate_color_temperature(
    profile: &crate::ProfileSpec,
    value: Option<crate::types::Kelvin>,
) -> Result<(), Error> {
    if let Some(value) = value {
        let range = white_balance_facts(profile)?
            .color_temp_range
            .as_ref()
            .ok_or(Error::FeatureNotSupported {
                feature: "color-temperature range",
            })?;
        require_in_range("color temperature", value.value(), range)?;
        white_balance_facts(profile)?.color_temperature_wire(value)?;
    }
    Ok(())
}

/// The physical temperature is converted only after the target is selected.
fn write_color_temperature_for_target(
    command: &crate::command::color::ColorTemperature,
    target: &crate::TargetRegistration,
    buffer: &mut [u8],
) -> Result<usize, Error> {
    if let crate::command::color::ColorTemperature::SetTemperature(value) = command {
        let code = white_balance_facts(target.profile())?.color_temperature_wire(*value)?;
        #[cfg(test)]
        REQUEST_WRITE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
        crate::command::bytes::FrameWriter::new(target.target(), buffer)
            .bytes(&crate::command::bytes::constants::color::TEMPERATURE)
            .nibbles::<2>(code)
            .finish()
    } else {
        <crate::command::color::ColorTemperature as Request>::write_into(
            command,
            target.target(),
            buffer,
        )
    }
}

fn validate_rgb_gain(
    profile: &crate::ProfileSpec,
    value: Option<u8>,
    red: bool,
) -> Result<(), Error> {
    if let Some(value) = value {
        let gain =
            white_balance_facts(profile)?
                .rgb_gain
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "RGB gain range",
                })?;
        require_in_range("RGB gain", value, if red { &gain.red } else { &gain.blue })?;
    }
    Ok(())
}

fn validate_image_range(
    feature: &'static str,
    range: Option<&crate::capabilities::CapabilityRange<u8>>,
    value: u8,
) -> Result<(), Error> {
    let range = range.ok_or(Error::FeatureNotSupported { feature })?;
    require_in_range(feature, value, range)?;
    Ok(())
}

/// The profile's motion-sync speed range, which is also its only motion-sync
/// fact: `None` means the camera has no motion sync.
fn motion_sync_speed_range(
    profile: &crate::ProfileSpec,
) -> Result<&crate::capabilities::CapabilityRange<u8>, Error> {
    pan_tilt_facts(profile)?
        .motion_sync_speed_range
        .as_ref()
        .ok_or(Error::FeatureNotSupported {
            feature: "motion sync",
        })
}

fn validate_motion_sync_speed(
    profile: &crate::ProfileSpec,
    speed: crate::types::MotionSyncSpeed,
) -> Result<(), Error> {
    require_in_range(
        "motion sync speed",
        speed.value(),
        motion_sync_speed_range(profile)?,
    )
}

impl BuiltinValidation for crate::command::power_wire::PowerOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_power_state(profile, false)
    }
}

impl BuiltinValidation for crate::command::power_wire::PowerStandby {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_power_state(profile, true)
    }
}

impl BuiltinValidation for crate::command::exposure_wire::ExposureCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_exposure_mode(profile, self.mode)
    }
}

impl BuiltinValidation for crate::command::exposure_wire::ExposureCompensation {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let value = match self {
            Self::SetLevel(level) => Some(level.value()),
            _ => None,
        };
        validate_exposure_compensation(profile, value)
    }
}

impl BuiltinValidation for crate::command::exposure_wire::DynamicRange {}

impl BuiltinValidation for crate::command::exposure_wire::Shutter {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_shutter(
            profile,
            match self {
                Self::SetSpeed(speed) => Some(speed.value()),
                _ => None,
            },
        )
    }
}

impl BuiltinValidation for crate::command::exposure_wire::Brightness {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_brightness(
            profile,
            match self {
                Self::SetLevel(level) => Some(level.value()),
                _ => None,
            },
        )
    }
}

impl BuiltinValidation for crate::command::exposure_wire::AntiFlickerCommand {}

impl BuiltinValidation for crate::command::gain::Gain {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_gain(
            profile,
            match self {
                Self::SetValue(level) => Some(level.value()),
                _ => None,
            },
        )
    }
}

// Every sourced model gives this register the full nibble enforced by
// `GainLimit`. Its domain is independent of the separate gain-level register.
impl BuiltinValidation for crate::command::gain::GainLimitCommand {}

impl BuiltinValidation for crate::command::color::OnePushTriggerCommand {}

impl BuiltinValidation for crate::command::color::RedTuningCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tuning(profile, self.level.value(), true)
    }
}

impl BuiltinValidation for crate::command::color::BlueTuningCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tuning(profile, self.level.value(), false)
    }
}

impl BuiltinValidation for crate::command::color::SaturationCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            "saturation control",
            profile
                .capabilities()
                .image
                .as_ref()
                .and_then(|domain| domain.saturation_range.as_ref()),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::color::HueCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            "hue control",
            profile
                .capabilities()
                .image
                .as_ref()
                .and_then(|domain| domain.hue_range.as_ref()),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::color::ColorTemperature {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_color_temperature(
            profile,
            match self {
                Self::SetTemperature(value) => Some(*value),
                _ => None,
            },
        )
    }
}

impl BuiltinValidation for crate::command::color::RedGain {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_rgb_gain(
            profile,
            match self {
                Self::SetValue(value) => Some(value.value()),
                _ => None,
            },
            true,
        )
    }
}

impl BuiltinValidation for crate::command::color::BlueGain {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_rgb_gain(
            profile,
            match self {
                Self::SetValue(value) => Some(value.value()),
                _ => None,
            },
            false,
        )
    }
}

impl BuiltinValidation for crate::command::image_wire::Sharpness {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        match self {
            Self::SetLevel { value } => validate_image_range(
                "sharpness control",
                profile
                    .capabilities()
                    .image
                    .as_ref()
                    .and_then(|domain| domain.sharpness_range.as_ref()),
                value.value(),
            ),
            _ => Ok(()),
        }
    }
}

impl BuiltinValidation for crate::command::image_wire::Luminance {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            "luminance control",
            profile
                .capabilities()
                .image
                .as_ref()
                .and_then(|domain| domain.luminance_range.as_ref()),
            self.value.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image_wire::Contrast {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            "contrast control",
            profile
                .capabilities()
                .image
                .as_ref()
                .and_then(|domain| domain.contrast_range.as_ref()),
            self.value.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image_wire::GammaCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            "gamma control",
            profile
                .capabilities()
                .image
                .as_ref()
                .and_then(|domain| domain.gamma_range.as_ref()),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image_wire::BacklightCommand {}

impl BuiltinValidation for crate::command::image_wire::NoiseReduction2DModeCommand {}

impl BuiltinValidation for crate::command::image_wire::NoiseReduction2DCommand {}

impl BuiltinValidation for crate::command::image_wire::NoiseReduction3DCommand {}

impl BuiltinValidation for crate::command::image_wire::ImageFlipCombinedCommand {}

impl BuiltinValidation for crate::command::image_wire::PictureEffectCommand {}

impl BuiltinValidation for crate::command::focus_wire::FocusZoneCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();

        if capabilities
            .focus
            .as_ref()
            .is_some_and(|domain| domain.zones.contains(&self.zone))
        {
            Ok(())
        } else {
            Err(Error::InvalidParameter {
                parameter: "focus_zone",
                value: Cow::Owned(format!("{:02X}", self.zone.to_wire())),
                reason: Cow::Borrowed(
                    "the profile has no evidence that the camera accepts this focus-zone value",
                ),
            })
        }
    }
}

impl BuiltinValidation for crate::command::focus_wire::AutoFocusSensitivityCommand {}

impl BuiltinValidation for crate::command::focus_wire::FocusNearLimitCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let value = self.position.value();
        require_in_range("focus near limit", value, &focus_facts(profile)?.range)
    }
}

impl BuiltinValidation for crate::command::white_balance_wire::WhiteBalanceCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_white_balance_mode(profile, self)
    }
}

impl BuiltinValidation for crate::command::white_balance_wire::AWBSensitivityCommand {}

impl BuiltinValidation for crate::command::menu_wire::SetMenuDisplay {}

impl BuiltinValidation for crate::command::menu_wire::MenuNavigate {}

impl BuiltinValidation for crate::command::menu_wire::PerformMenuAction {}

impl BuiltinValidation for crate::command::menu_wire::DirectMenuControl {}

impl BuiltinValidation for crate::command::streaming_wire::UsbAudioCommand {}

impl BuiltinValidation for crate::command::system_wire::SettingsSaveCommand {}

impl BuiltinValidation for crate::command::tally_wire::TallyRedOn {}

impl BuiltinValidation for crate::command::tally_wire::TallyRedOff {}

impl BuiltinValidation for crate::command::tally_wire::TallyGreenOn {}

impl BuiltinValidation for crate::command::tally_wire::TallyGreenOff {}

impl BuiltinValidation for crate::command::motion_sync_wire::SetMotionSyncMode {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        motion_sync_speed_range(profile).map(|_| ())
    }
}

impl BuiltinValidation for crate::command::motion_sync_wire::SetMotionSyncPreset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_motion_sync_speed(profile, self.speed())
    }
}

impl BuiltinValidation for crate::command::system_wire::AddressSetCommand {}

impl BuiltinValidation for crate::command::system_wire::InterfaceClearCommand {}

impl BuiltinValidation for crate::command::system_wire::CommandCancelCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(
            profile.supports_command_cancel(),
            "VISCA socket cancellation",
        )
    }
}

impl BuiltinValidation for ImageFlipCommand {}

impl BuiltinValidation for ImageMirrorCommand {}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedPanTiltPosition {
    pan: i32,
    tilt: i32,
    conversion: PanTiltCoordinateConversion,
    mount: Option<crate::MountPosition>,
}

impl PreparedPanTiltPosition {
    fn for_target(
        pan: Degrees,
        tilt: Degrees,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        let profile = target.profile();
        let conversion = profile
            .pan_tilt_coordinates()
            .ok_or(Error::FeatureNotSupported {
                feature: "pan/tilt coordinate conversion",
            })?;
        let (pan, tilt) = target.convert_pan_tilt_degrees(pan.value(), tilt.value())?;
        Ok(Self {
            pan,
            tilt,
            conversion,
            mount: target.mount(),
        })
    }

    fn validate(self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        if profile.pan_tilt_coordinates() != Some(self.conversion) {
            return Err(Error::InvalidRequest {
                reason: "pan/tilt request was converted for a different profile".into(),
            });
        }
        require_in_range(
            "pan position",
            self.pan,
            &pan_tilt_facts(profile)?.pan_range,
        )?;
        require_in_range(
            "tilt position",
            self.tilt,
            &pan_tilt_facts(profile)?.tilt_ranges.range(self.mount),
        )
    }
}

/// A relative displacement: source scale and codec, with a span bound.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedPanTiltOffset {
    pan: i32,
    tilt: i32,
    conversion: PanTiltCoordinateConversion,
}

impl PreparedPanTiltOffset {
    fn for_target(
        pan: Degrees,
        tilt: Degrees,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        let profile = target.profile();
        let conversion = profile
            .pan_tilt_coordinates()
            .ok_or(Error::FeatureNotSupported {
                feature: "pan/tilt coordinate conversion",
            })?;
        let (pan, tilt) = target.convert_pan_tilt_offset_degrees(pan.value(), tilt.value())?;
        target.validate_pan_tilt_offset(pan, tilt)?;
        Ok(Self {
            pan,
            tilt,
            conversion,
        })
    }

    fn validate(self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        if profile.pan_tilt_coordinates() != Some(self.conversion) {
            return Err(Error::InvalidRequest {
                reason: "pan/tilt offset was converted for a different profile".into(),
            });
        }
        Ok(())
    }
}

/// Return pan/tilt to its home position.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PanTiltHome;

builtin_request! {
    PanTiltHome => PanTiltHome: Targeted {
        size: 5,
        wire: |_: &PanTiltHome| PanTilt::Home,
    };
}

/// Internal value-selected encoder for the canonical boolean setter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpotlightToggle {
    enabled: bool,
}
impl SpotlightToggle {
    pub(crate) const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}
impl WireEncode for SpotlightToggle {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        if self.enabled {
            WireEncode::write_into(
                &crate::command::exposure_wire::SpotlightOn::new(),
                camera_id,
                buffer,
            )
        } else {
            WireEncode::write_into(
                &crate::command::exposure_wire::SpotlightOff::new(),
                camera_id,
                buffer,
            )
        }
    }
}
impl BuiltinValidation for SpotlightToggle {}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for SpotlightToggle {
    fn state_value(&self) -> bool {
        self.enabled
    }
}
builtin_request! {
    SpotlightToggle => SpotlightOn: Plain { size: 6,
        rows: |request| match (request.enabled) { true => SpotlightOn, false => SpotlightOff },
    };
}

/// Internal value-selected encoder for the canonical boolean setter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoSlowShutterToggle {
    enabled: bool,
}
impl AutoSlowShutterToggle {
    pub(crate) const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}
impl WireEncode for AutoSlowShutterToggle {
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        if self.enabled {
            WireEncode::write_into(
                &crate::command::exposure_wire::AutoSlowShutterOn::new(),
                camera_id,
                buffer,
            )
        } else {
            WireEncode::write_into(
                &crate::command::exposure_wire::AutoSlowShutterOff::new(),
                camera_id,
                buffer,
            )
        }
    }
}
impl BuiltinValidation for AutoSlowShutterToggle {}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for AutoSlowShutterToggle {
    fn state_value(&self) -> bool {
        self.enabled
    }
}
builtin_request! {
    AutoSlowShutterToggle => AutoSlowShutterOn: Plain { size: 6,
        rows: |request| match (request.enabled) { true => AutoSlowShutterOn, false => AutoSlowShutterOff },
    };
}

// Built-in command values that are already structurally homogeneous carry
// their request contract directly: every branch represented by the type is a
// configuration, mode, persistence, or output-policy write, so it needs no
// second public noun or branch discriminator. Mixed physical enums (PanTilt,
// Zoom, Focus, Iris, ND, and Preset) are represented by the structurally
// distinct request types below.
builtin_request! {
    crate::command::preset_wire::PresetRecallSpeedCommand => PresetRecallSpeed: Plain { size: 6 };
    crate::command::focus_wire::FocusLockCommand => FocusLock: Plain { size: 6 };
    crate::command::exposure_wire::SpotlightOn => SpotlightOn: Plain { size: 6 };
    crate::command::exposure_wire::SpotlightOff => SpotlightOff: Plain { size: 6 };
    crate::command::exposure_wire::AutoSlowShutterOn => AutoSlowShutterOn: Plain { size: 6 };
    crate::command::exposure_wire::AutoSlowShutterOff => AutoSlowShutterOff: Plain { size: 6 };
    crate::command::nd_filter_wire::NdFilterModeCommand => NdFilterMode: Plain { size: 7 };
    crate::command::nd_filter_wire::AutoNdCommand => NdFilterAutoOn: Plain {
        size: 7,
        rows: |command| match (command.enabled()) {
            true => NdFilterAutoOn,
            false => NdFilterAutoOff,
        },
    };
    crate::command::flip::ImageFreeze => ImageFreezeOn: Plain {
        size: 6,
        rows: |command| match (command.enabled()) {
            true => ImageFreezeOn,
            false => ImageFreezeOff,
        },
    };
    crate::command::zoom_wire::DigitalZoom => DigitalZoom: Plain { size: 6 };
    crate::command::streaming_wire::MulticastStreamingCommand => MulticastStreamingOn: Plain {
        size: 6,
        policy: (Network, Standard, Normal),
        rows: |command| match (command.enabled()) {
            true => MulticastStreamingOn,
            false => MulticastStreamingOff,
        },
    };
    crate::command::streaming_wire::SetNdiQuality => NdiQuality: Plain {
        size: 6,
        policy: (Network, Standard, Normal),
    };
    crate::command::tally_wire::TallyBrightLo => TallyBrightLow: Plain { size: 8 };
    crate::command::tally_wire::TallyBrightHi => TallyBrightHigh: Plain { size: 8 };
    crate::command::variable_speed::SetVariableSpeedMode => VariableSpeedMode: Plain { size: 6 };
    crate::command::tally_wire::TallyOn => TallyOn: Plain { size: 6 };
    crate::command::tally_wire::TallyOff => TallyOff: Plain { size: 6 };
    crate::command::tally_wire::TallyFlash => TallyFlash: Plain { size: 6 };
    crate::command::power_wire::PowerOn => PowerOn: Plain { size: 6 };
    crate::command::power_wire::PowerStandby => PowerStandby: Plain { size: 6 };
    crate::command::exposure_wire::ExposureCommand => ExposureMode: Plain { size: 6 };
    crate::command::exposure_wire::ExposureCompensation => ExposureCompensationOn: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::exposure_wire::ExposureCompensation::On => ExposureCompensationOn,
            crate::command::exposure_wire::ExposureCompensation::Off => ExposureCompensationOff,
            crate::command::exposure_wire::ExposureCompensation::Reset => ExposureCompensationReset,
            crate::command::exposure_wire::ExposureCompensation::Up => ExposureCompensationUp,
            crate::command::exposure_wire::ExposureCompensation::Down => ExposureCompensationDown,
            crate::command::exposure_wire::ExposureCompensation::SetLevel(_) => ExposureCompensationDirect,
        },
    };
    crate::command::exposure_wire::DynamicRange => DynamicRange: Plain { size: 9 };
    crate::command::exposure_wire::Shutter => ShutterReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::exposure_wire::Shutter::Reset => ShutterReset,
            crate::command::exposure_wire::Shutter::Up => ShutterUp,
            crate::command::exposure_wire::Shutter::Down => ShutterDown,
            crate::command::exposure_wire::Shutter::SetSpeed(_) => ShutterDirect,
        },
    };
    crate::command::exposure_wire::Brightness => BrightnessReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::exposure_wire::Brightness::Reset => BrightnessReset,
            crate::command::exposure_wire::Brightness::Up => BrightnessUp,
            crate::command::exposure_wire::Brightness::Down => BrightnessDown,
            crate::command::exposure_wire::Brightness::SetLevel(_) => BrightnessSet,
        },
    };
    crate::command::exposure_wire::AntiFlickerCommand => AntiFlicker: Plain { size: 6 };
    crate::command::gain::Gain => GainReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::gain::Gain::Reset => GainReset,
            crate::command::gain::Gain::Up => GainUp,
            crate::command::gain::Gain::Down => GainDown,
            crate::command::gain::Gain::SetValue(_) => GainDirect,
        },
    };
    crate::command::gain::GainLimitCommand => GainLimit: Plain { size: 6 };
    crate::command::color::OnePushTriggerCommand => OnePushWhiteBalanceTrigger: Plain { size: 6 };
    crate::command::color::RedTuningCommand => RedTuning: Plain { size: 9 };
    crate::command::color::BlueTuningCommand => BlueTuning: Plain { size: 9 };
    crate::command::color::SaturationCommand => Saturation: Plain { size: 9 };
    crate::command::color::HueCommand => Hue: Plain { size: 9 };
    crate::command::color::ColorTemperature => ColorTemperatureReset: Plain {
        size: 7,
        target_wire: write_color_temperature_for_target,
        rows: |command| match (command) {
            crate::command::color::ColorTemperature::Reset => ColorTemperatureReset,
            crate::command::color::ColorTemperature::Up => ColorTemperatureUp,
            crate::command::color::ColorTemperature::Down => ColorTemperatureDown,
            crate::command::color::ColorTemperature::SetTemperature(_) => ColorTemperatureDirect,
        },
    };
    crate::command::color::RedGain => RedGainReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::color::RedGain::Reset => RedGainReset,
            crate::command::color::RedGain::Up => RedGainUp,
            crate::command::color::RedGain::Down => RedGainDown,
            crate::command::color::RedGain::SetValue(_) => RedGainDirect,
        },
    };
    crate::command::color::BlueGain => BlueGainReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::color::BlueGain::Reset => BlueGainReset,
            crate::command::color::BlueGain::Up => BlueGainUp,
            crate::command::color::BlueGain::Down => BlueGainDown,
            crate::command::color::BlueGain::SetValue(_) => BlueGainDirect,
        },
    };
    crate::command::image_wire::Sharpness => SharpnessReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::image_wire::Sharpness::Mode(_) => SharpnessMode,
            crate::command::image_wire::Sharpness::Reset => SharpnessReset,
            crate::command::image_wire::Sharpness::Up => SharpnessUp,
            crate::command::image_wire::Sharpness::Down => SharpnessDown,
            crate::command::image_wire::Sharpness::SetLevel { .. } => SharpnessDirect,
        },
    };
    crate::command::image_wire::Luminance => Luminance: Plain { size: 9 };
    crate::command::image_wire::Contrast => Contrast: Plain { size: 9 };
    crate::command::image_wire::GammaCommand => Gamma: Plain { size: 6 };
    crate::command::image_wire::BacklightCommand => Backlight: Plain { size: 6 };
    crate::command::image_wire::NoiseReduction2DModeCommand => NoiseReduction2dMode: Plain { size: 6 };
    crate::command::image_wire::NoiseReduction2DCommand => NoiseReduction2d: Plain {
        size: 6,
        rows: |command| match (command.setting) {
            crate::command::NoiseReduction2D::Off => NoiseReduction2dOff,
            crate::command::NoiseReduction2D::Level(_) => NoiseReduction2d,
        },
    };
    crate::command::image_wire::NoiseReduction3DCommand => NoiseReduction3d: Plain {
        size: 6,
        rows: |command| match (command.setting) {
            crate::command::NoiseReduction3D::Off => NoiseReduction3dOff,
            crate::command::NoiseReduction3D::Level(_) => NoiseReduction3d,
        },
    };
    crate::command::image_wire::ImageFlipCombinedCommand => ImageFlipBoth: Plain {
        size: 6,
        rows: |command| match ((command.mode as u8) == (crate::types::ImageFlipMode::Both as u8)) {
            true => ImageFlipBoth,
            false => ImageFlipCombined,
        },
    };
    crate::command::image_wire::PictureEffectCommand => PictureEffect: Plain { size: 6 };
    crate::command::focus_wire::FocusZoneCommand => FocusZone: Plain { size: 6 };
    crate::command::focus_wire::AutoFocusSensitivityCommand => FocusAutoSensitivity: Plain { size: 6 };
    crate::command::focus_wire::FocusNearLimitCommand => FocusNearLimit: Plain { size: 9 };
    crate::command::white_balance_wire::WhiteBalanceCommand => WhiteBalanceAuto: Plain {
        size: 6,
        rows: |command| match (command.mode) {
            crate::command::WhiteBalanceMode::Auto => WhiteBalanceAuto,
            crate::command::WhiteBalanceMode::Indoor => WhiteBalanceIndoor,
            crate::command::WhiteBalanceMode::Outdoor => WhiteBalanceOutdoor,
            crate::command::WhiteBalanceMode::OnePush => WhiteBalanceOnePush,
            crate::command::WhiteBalanceMode::Atw => WhiteBalanceAutoTracking,
            crate::command::WhiteBalanceMode::Manual => WhiteBalanceManual,
            crate::command::WhiteBalanceMode::Preset => WhiteBalancePreset,
            crate::command::WhiteBalanceMode::ColorTemperature => WhiteBalanceColorTemperature,
        },
    };
    crate::command::white_balance_wire::AWBSensitivityCommand => AutoWhiteBalanceSensitivity: Plain {
        size: 6,
    };
    crate::command::menu_wire::SetMenuDisplay => MenuDisplay: Plain { size: 6 };
    crate::command::menu_wire::MenuNavigate => MenuNavigate: AppliedOnly { size: 9 };
    crate::command::menu_wire::PerformMenuAction => MenuSelect: Plain {
        size: 6,
        rows: |command| match (command.action) {
            crate::command::menu_wire::MenuAction::Select => MenuSelect,
            crate::command::menu_wire::MenuAction::Cancel => MenuCancel,
        },
    };
    crate::command::menu_wire::DirectMenuControl => DirectMenu: Plain { size: 8 };
    crate::command::streaming_wire::UsbAudioCommand => UsbAudioOn: Plain {
        size: 7,
        rows: |command| match (command.enabled()) {
            true => UsbAudioOn,
            false => UsbAudioOff,
        },
    };
    crate::command::system_wire::SettingsSaveCommand => SettingsSave: Plain { size: 6 };
    crate::command::tally_wire::TallyRedOn => TallyRedOn: Plain { size: 8 };
    crate::command::tally_wire::TallyRedOff => TallyRedOff: Plain { size: 8 };
    crate::command::tally_wire::TallyGreenOn => TallyGreenOn: Plain { size: 8 };
    crate::command::tally_wire::TallyGreenOff => TallyGreenOff: Plain { size: 8 };
    crate::command::motion_sync_wire::SetMotionSyncMode => MotionSyncMode: Plain { size: 6 };
    crate::command::motion_sync_wire::SetMotionSyncPreset => MotionSyncPreset: Plain { size: 6 };
    // These three protocol controls deliberately have crate-private request
    // implementations. Address assignment and interface clear run only while
    // the serial transport handshake owns the wire; socket cancellation is
    // emitted by the operation owner that holds the exact request/socket
    // correlation.
    crate::command::system_wire::AddressSetCommand => AddressSet: Plain { size: 4 };
    crate::command::system_wire::InterfaceClearCommand => InterfaceClear: Plain { size: 5 };
    crate::command::system_wire::CommandCancelCommand => CommandCancel: Plain {
        size: 3,
        policy: (Quick, Never, Urgent),
    };
}

/// Separate vertical-flip command with its own VISCA opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFlipCommand(pub bool);

impl ImageFlipCommand {
    /// Creates a vertical-flip request.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self(enabled)
    }
}

builtin_request! {
    ImageFlipCommand => ImageFlipOff: Plain {
        size: 6,
        wire: |value: &ImageFlipCommand| crate::command::flip::ImageFlip { flip: value.0 },
        rows: |command| match (command.0) {
            true => ImageFlipVertical,
            false => ImageFlipOff,
        },
    };
}

/// Separate horizontal-mirror command with its own VISCA opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageMirrorCommand {
    enabled: bool,
}

impl ImageMirrorCommand {
    /// Creates a horizontal-mirror request.
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

builtin_request! {
    ImageMirrorCommand => ImageFlipHorizontal: Plain {
        size: 6,
        wire: |value: &ImageMirrorCommand| crate::command::flip::HorizontalFlip { on: value.enabled },
        rows: |command| match (command.enabled) {
            true => ImageFlipHorizontal,
            false => ImageFlipHorizontalOff,
        },
    };
}

/// A canonical request's exact protocol-row coverage, generated from the noun table.
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinTypedRequestCoverage {
    pub row: BuiltinCommand,
    pub type_name: &'static str,
    pub branch: &'static str,
    pub state_key: Option<crate::StateKey>,
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
#[derive(Clone, Copy)]
struct TypedRequestFamily {
    rows: &'static [BuiltinCommand],
    type_name: &'static str,
    branch: &'static str,
    state_key: Option<crate::StateKey>,
    state_by_row: &'static [(BuiltinCommand, Option<crate::StateKey>)],
}

/// Validates every declared row rather than interpreting public request names as
/// protocol enum variants. Encoding-selected requests may have distinct state
/// keys; their exact row-to-state map is checked by the generated inventory.
const fn assert_request_rows<T: BuiltinRequestContract>() {
    assert!(
        !T::LEDGER_ROWS.is_empty(),
        "a canonical request declares no protocol rows"
    );
    let mut index = 0;
    while index < T::LEDGER_ROWS.len() {
        let row = T::LEDGER_ROWS[index];
        assert!(
            row.classification().kind() as usize == T::LEDGER_ROW.classification().kind() as usize,
            "a typed request serves a ledger row whose request class differs from its own row"
        );
        assert!(
            row_gate(row).same_as(row_gate(T::LEDGER_ROW)),
            "a typed request serves ledger rows with different capability gates"
        );
        let mut previous = 0;
        while previous < index {
            assert!(
                row as usize != T::LEDGER_ROWS[previous] as usize,
                "a request repeats a declared protocol row"
            );
            previous += 1;
        }
        index += 1;
    }
}

macro_rules! assert_canonical_request {
    (inquiry, $request:ty) => {};
    ($kind:ident, $request:ty) => {
        assert_request_rows::<$request>();
    };
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
macro_rules! typed_request_family {
    (inquiry, $request:ty, $expression:tt) => { None };
    ($kind:ident, $request:ty, $expression:tt) => {
        Some(TypedRequestFamily {
            rows: <$request as BuiltinRequestContract>::LEDGER_ROWS,
            type_name: stringify!($request),
            branch: stringify!($expression),
            state_key: <<$request as BuiltinRequestContract>::State as super::builtin_contract::StateDescriptor>::KEY,
            state_by_row: <<$request as BuiltinRequestContract>::State as super::builtin_contract::StateDescriptor>::BY_ROW,
        })
    };
}

macro_rules! typed_request_inventory {
    (
        $(
            @noun $noun:ident { $($header:tt)* };
            $(
                $(#[$doc:meta])*
                $kind:ident [$key:ident] $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions;
        $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        const _: () = {
            use crate::command;
            $( $( assert_canonical_request!($kind, $ret); )* )*
            $( assert_request_rows::<$exty>(); )*
        };

        #[cfg(all(test, any(feature = "blocking", feature = "async")))]
        const TYPED_REQUEST_FAMILIES: &[Option<TypedRequestFamily>] = {
            use crate::command;
            &[
                $( $( typed_request_family!($kind, $ret, [$($request)*]), )* )*
                $( typed_request_family!(plain, $exty, [$exkind]), )*
            ]
        };
    };
}

noun_table!(typed_request_inventory);

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
const fn typed_request_coverage_count() -> usize {
    let mut count = 0;
    let mut index = 0;
    while index < TYPED_REQUEST_FAMILIES.len() {
        if let Some(family) = TYPED_REQUEST_FAMILIES[index] {
            count += family.rows.len();
        }
        index += 1;
    }
    count
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
const fn build_typed_request_coverage<const N: usize>() -> [BuiltinTypedRequestCoverage; N] {
    let mut coverage = [BuiltinTypedRequestCoverage {
        row: BuiltinCommand::ALL[0],
        type_name: "",
        branch: "",
        state_key: None,
    }; N];
    let mut output = 0;
    let mut family_index = 0;
    while family_index < TYPED_REQUEST_FAMILIES.len() {
        if let Some(family) = TYPED_REQUEST_FAMILIES[family_index] {
            if !family.state_by_row.is_empty() {
                assert!(
                    family.state_by_row.len() == family.rows.len(),
                    "an encoding-selected state map must cover exactly its declared rows"
                );
            }
            let mut row_index = 0;
            while row_index < family.rows.len() {
                let row = family.rows[row_index];
                let mut state_key = family.state_key;
                if !family.state_by_row.is_empty() {
                    let mut found = false;
                    let mut state_index = 0;
                    while state_index < family.state_by_row.len() {
                        let (candidate, key) = family.state_by_row[state_index];
                        if candidate as usize == row as usize {
                            assert!(!found, "an encoding-selected state map repeats a row");
                            found = true;
                            state_key = key;
                        }
                        state_index += 1;
                    }
                    assert!(found, "an encoding-selected state map omits a declared row");
                }
                coverage[output] = BuiltinTypedRequestCoverage {
                    row,
                    type_name: family.type_name,
                    branch: family.branch,
                    state_key,
                };
                output += 1;
                row_index += 1;
            }
        }
        family_index += 1;
    }
    assert!(output == N);
    coverage
}

#[cfg(all(test, any(feature = "blocking", feature = "async")))]
pub(crate) static BUILTIN_TYPED_REQUEST_INVENTORY: &[BuiltinTypedRequestCoverage] =
    &build_typed_request_coverage::<{ typed_request_coverage_count() }>();

/// Reset the pan/tilt mechanism.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PanTiltReset;

builtin_request! {
    PanTiltReset => PanTiltReset: Targeted {
        size: 5,
        wire: |_: &PanTiltReset| PanTilt::Reset,
    };
}

/// Continuous pan/tilt directional drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltDrive {
    direction: PanTiltDirection,
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

impl PanTiltDrive {
    /// Creates a directional drive. STOP has its own structurally distinct type.
    pub fn new(
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<Self, Error> {
        if direction == PanTiltDirection::Stop {
            return Err(Error::InvalidRequest {
                reason: "use PanTiltStop for the stop operation".into(),
            });
        }
        Ok(Self {
            direction,
            pan_speed,
            tilt_speed,
        })
    }
}

builtin_request! {
    PanTiltDrive => PanTiltDrive: AppliedOnly {
        size: 9,
        wire: |value: &PanTiltDrive| PanTilt::Move {
            direction: value.direction,
            pan_speed: value.pan_speed,
            tilt_speed: value.tilt_speed,
        },
    };
}

/// Stop pan/tilt movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltStop {
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

impl PanTiltStop {
    /// Creates a stop request with explicit protocol speed fields.
    #[must_use]
    pub const fn new(pan_speed: PanSpeed, tilt_speed: TiltSpeed) -> Self {
        Self {
            pan_speed,
            tilt_speed,
        }
    }
}

builtin_request! {
    PanTiltStop => PanTiltStop: AppliedOnly {
        size: 9,
        policy: (Quick, Movement, Urgent),
        wire: |value: &PanTiltStop| PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: value.pan_speed,
            tilt_speed: value.tilt_speed,
        },
    };
}

/// Absolute pan/tilt target converted from degrees through one validated profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanTiltAbsolute {
    position: PreparedPanTiltPosition,
    speeds: MoveSpeeds,
}

impl PanTiltAbsolute {
    /// Creates an absolute target request.
    pub fn for_target(
        pan: Degrees,
        tilt: Degrees,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        let profile = target.profile();
        let speeds = validate_pan_tilt_position_speed(profile, pan_speed, tilt_speed)?;
        Ok(Self {
            position: PreparedPanTiltPosition::for_target(pan, tilt, target)?,
            speeds,
        })
    }

    /// Creates an absolute target from one coarse speed through the profile's
    /// position-command grammar.
    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) fn for_target_speed_level(
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        let (pan_speed, tilt_speed) = pan_tilt_position_speeds_from_level(speed, target.profile())?;
        Self::for_target(pan, tilt, pan_speed, tilt_speed, target)
    }
}

builtin_request! {
    PanTiltAbsolute => PanTiltAbsolute: Targeted {
        size: 17,
        wire: |value: &PanTiltAbsolute| PanTiltProfiled::AbsolutePosition {
            framing: PanTiltFraming::for_conversion(value.position.conversion),
            pan: value.position.pan,
            tilt: value.position.tilt,
            speeds: value.speeds,
        },
    };
}

/// Relative pan/tilt offset converted from degrees through one validated profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanTiltRelative {
    offset: PreparedPanTiltOffset,
    speeds: MoveSpeeds,
}

impl PanTiltRelative {
    /// Creates a relative target request.
    pub fn for_target(
        pan: Degrees,
        tilt: Degrees,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        let profile = target.profile();
        let speeds = validate_pan_tilt_position_speed(profile, pan_speed, tilt_speed)?;
        Ok(Self {
            offset: PreparedPanTiltOffset::for_target(pan, tilt, target)?,
            speeds,
        })
    }

    /// Creates a relative target from one coarse speed through the profile's
    /// position-command grammar.
    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub(crate) fn for_target_speed_level(
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        let (pan_speed, tilt_speed) = pan_tilt_position_speeds_from_level(speed, target.profile())?;
        Self::for_target(pan, tilt, pan_speed, tilt_speed, target)
    }
}

builtin_request! {
    PanTiltRelative => PanTiltRelative: Targeted {
        size: 17,
        wire: |value: &PanTiltRelative| PanTiltProfiled::RelativePosition {
            framing: PanTiltFraming::for_conversion(value.offset.conversion),
            pan: value.offset.pan,
            tilt: value.offset.tilt,
            speeds: value.speeds,
        },
    };
}

/// Set one pan/tilt movement-limit corner from profile-converted degree values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanTiltLimitSet {
    corner: PanTiltLimitCorner,
    position: PreparedPanTiltPosition,
}

impl PanTiltLimitSet {
    /// Creates a limit-setting command.
    pub fn for_target(
        corner: PanTiltLimitCorner,
        pan: Degrees,
        tilt: Degrees,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        Ok(Self {
            corner,
            position: PreparedPanTiltPosition::for_target(pan, tilt, target)?,
        })
    }
}

builtin_request! {
    PanTiltLimitSet => PanTiltLimitSet: Plain {
        size: 17,
        wire: |value: &PanTiltLimitSet| PanTiltProfiled::LimitSet {
            framing: PanTiltFraming::for_conversion(value.position.conversion),
            corner: value.corner,
            pan: value.position.pan,
            tilt: value.position.tilt,
        },
    };
}

/// Clear one pan/tilt movement-limit corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanTiltLimitClear {
    corner: PanTiltLimitCorner,
    wire_codec: PanTiltWireCodec,
}

impl PanTiltLimitClear {
    /// Creates a standard-VISCA limit-clear command.
    ///
    /// Use [`Self::for_profile`] when the camera profile owns a different
    /// position-command framing, such as Sony BRC-300.
    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    #[must_use]
    pub const fn new(corner: PanTiltLimitCorner) -> Self {
        Self {
            corner,
            wire_codec: PanTiltWireCodec::StandardVisca,
        }
    }

    /// Creates a limit-clear command bound to one validated profile's framing.
    pub fn for_profile(
        corner: PanTiltLimitCorner,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        let conversion = profile
            .pan_tilt_coordinates()
            .ok_or(Error::FeatureNotSupported {
                feature: "pan/tilt coordinate conversion",
            })?;
        Ok(Self {
            corner,
            wire_codec: conversion.wire_codec(),
        })
    }
}

builtin_request! {
    PanTiltLimitClear => PanTiltLimitClear: Plain {
        size: 17,
        wire: |value: &PanTiltLimitClear| PanTiltProfiled::LimitClear {
            codec: value.wire_codec,
            corner: value.corner,
        },
    };
}

/// Direct zoom target.
///
/// A target built from normalized input retains that input's domain until
/// profile validation. This prevents a combined optical-plus-digital request
/// whose mapped raw value happens to overlap the optical range from bypassing
/// the combined-domain typed-support gate on the erased API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomTarget(ZoomPosition, Option<crate::ZoomDomain>);

impl ZoomTarget {
    /// Creates a direct zoom target.
    #[must_use]
    pub const fn new(position: ZoomPosition) -> Self {
        Self(position, None)
    }

    /// Creates a direct zoom target from a normalized position.
    ///
    /// `0.0` is the wide end and `1.0` the telephoto end of the selected
    /// domain. [`crate::types::ZoomDomain::Optical`] always normalizes across the profile's
    /// documented optical range; [`crate::types::ZoomDomain::OpticalPlusDigital`] requires a
    /// documented digital maximum and never falls back to the optical one.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected domain has no documented maximum for
    /// `profile`, or when the mapped raw value is outside the VISCA zoom range.
    ///
    /// [`crate::types::ZoomDomain::Optical`]: crate::types::ZoomDomain::Optical
    /// [`crate::types::ZoomDomain::OpticalPlusDigital`]: crate::types::ZoomDomain::OpticalPlusDigital
    pub fn from_normalized(
        position: crate::units::UnitInterval,
        domain: crate::ZoomDomain,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        let zoom = zoom_facts(profile)?;
        let optical_max = zoom.optical_max;
        let digital_max = zoom.digital_max;
        let target = crate::inquiry_conversions::zoom_from_normalized(
            position,
            domain,
            optical_max,
            digital_max,
        )?;
        Ok(Self(target, Some(domain)))
    }
}

builtin_request! {
    ZoomTarget => ZoomPosition: Targeted {
        size: 9,
        wire: |value: &ZoomTarget| Zoom::Position(value.0),
    };
}

/// Continuous zoom-drive direction and speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomDrive {
    /// Drive toward telephoto at standard speed.
    Tele,
    /// Drive toward wide angle at standard speed.
    Wide,
    /// Drive toward telephoto at a variable speed.
    TeleVariable(ZoomSpeed),
    /// Drive toward wide angle at a variable speed.
    WideVariable(ZoomSpeed),
}

builtin_request! {
    ZoomDrive => ZoomTele: AppliedOnly {
        size: 6,
        wire: |value: &ZoomDrive| match value {
            ZoomDrive::Tele => Zoom::TeleStd,
            ZoomDrive::Wide => Zoom::WideStd,
            ZoomDrive::TeleVariable(speed) => Zoom::TeleVariable(*speed),
            ZoomDrive::WideVariable(speed) => Zoom::WideVariable(*speed),
        },
        rows: |value| match (value) {
            ZoomDrive::Tele => ZoomTele,
            ZoomDrive::Wide => ZoomWide,
            ZoomDrive::TeleVariable(_) => ZoomTeleVariable,
            ZoomDrive::WideVariable(_) => ZoomWideVariable,
        },
    };
}

/// Stop zoom movement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ZoomStop;

builtin_request! {
    ZoomStop => ZoomStop: AppliedOnly {
        size: 6,
        policy: (Quick, Movement, Urgent),
        wire: |_: &ZoomStop| Zoom::Stop,
    };
}

/// Direct focus target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusTarget(FocusPosition);

impl FocusTarget {
    /// Creates a direct focus target.
    #[must_use]
    pub const fn new(position: FocusPosition) -> Self {
        Self(position)
    }
}

builtin_request! {
    FocusTarget => FocusPosition: Targeted {
        size: 9,
        wire: |value: &FocusTarget| Focus::Position(value.0),
    };
}

/// Move focus to infinity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FocusInfinity;

builtin_request! {
    FocusInfinity => FocusInfinity: Targeted {
        size: 6,
        wire: |_: &FocusInfinity| Focus::Infinity,
    };
}

/// Continuous focus drive direction and speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDrive {
    /// Drive focus farther at standard speed.
    Far,
    /// Drive focus nearer at standard speed.
    Near,
    /// Drive focus farther at a variable speed.
    FarVariable(crate::types::FocusSpeed),
    /// Drive focus nearer at a variable speed.
    NearVariable(crate::types::FocusSpeed),
}

builtin_request! {
    FocusDrive => FocusFar: AppliedOnly {
        size: 6,
        wire: |value: &FocusDrive| match value {
            FocusDrive::Far => Focus::Far,
            FocusDrive::Near => Focus::Near,
            FocusDrive::FarVariable(speed) => Focus::FarWithSpeed(*speed),
            FocusDrive::NearVariable(speed) => Focus::NearWithSpeed(*speed),
        },
        rows: |value| match (value) {
            FocusDrive::Far => FocusFar,
            FocusDrive::Near => FocusNear,
            FocusDrive::FarVariable(_) => FocusFarVariable,
            FocusDrive::NearVariable(_) => FocusNearVariable,
        },
    };
}

/// Stop focus movement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FocusStop;

builtin_request! {
    FocusStop => FocusStop: AppliedOnly {
        size: 6,
        policy: (Quick, Movement, Urgent),
        wire: |_: &FocusStop| Focus::Stop,
    };
}

/// Instantaneous autofocus trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusTrigger {
    /// Perform one-push autofocus.
    OnePush,
    /// Perform vendor snap focus.
    Snap,
}

builtin_request! {
    FocusTrigger => FocusOnePush: AppliedOnly {
        size: 6,
        policy: (Quick, Movement, User),
        wire: |value: &FocusTrigger| match value {
            FocusTrigger::OnePush => Focus::OnePushTrigger,
            FocusTrigger::Snap => Focus::Snap,
        },
        rows: |trigger| match (trigger) {
            FocusTrigger::OnePush => FocusOnePush,
            FocusTrigger::Snap => FocusSnap,
        },
    };
}

/// Focus-mode configuration command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusModeCommand {
    /// Enable automatic focus.
    Auto,
    /// Enable manual focus.
    Manual,
    /// Toggle automatic/manual focus on supported cameras.
    Toggle,
}

builtin_request! {
    FocusModeCommand => FocusAuto: Plain {
        size: 6,
        wire: |value: &FocusModeCommand| match value {
            FocusModeCommand::Auto => Focus::Auto,
            FocusModeCommand::Manual => Focus::Manual,
            FocusModeCommand::Toggle => Focus::Toggle,
        },
        rows: |command| match (command) {
            FocusModeCommand::Auto => FocusAuto,
            FocusModeCommand::Manual => FocusManual,
            FocusModeCommand::Toggle => FocusToggle,
        },
    };
}

/// Reset the iris/aperture to its camera-defined default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IrisReset;

builtin_request! {
    IrisReset => IrisReset: Targeted {
        size: 6,
        wire: |_: &IrisReset| Iris::Reset,
    };
}

/// Increase the iris/aperture by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IrisUp;

builtin_request! {
    IrisUp => IrisUp: Targeted {
        size: 6,
        wire: |_: &IrisUp| Iris::Up,
    };
}

/// Decrease the iris/aperture by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IrisDown;

builtin_request! {
    IrisDown => IrisDown: Targeted {
        size: 6,
        wire: |_: &IrisDown| Iris::Down,
    };
}

/// Set an explicit iris/aperture target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrisDirect(IrisLevel);

impl IrisDirect {
    /// Creates a direct iris target.
    #[must_use]
    pub const fn new(level: IrisLevel) -> Self {
        Self(level)
    }
}

builtin_request! {
    IrisDirect => IrisDirect: Targeted {
        size: 9,
        wire: |value: &IrisDirect| Iris::SetAperture(value.0),
    };
}

/// Set an explicit variable ND-filter target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NdFilterDirect(NdFilterDensity);

impl NdFilterDirect {
    /// Creates a direct ND-filter target.
    #[must_use]
    pub const fn new(value: NdFilterDensity) -> Self {
        Self(value)
    }
}

builtin_request! {
    NdFilterDirect => NdFilterDirect: Targeted {
        size: 10,
        wire: |value: &NdFilterDirect| value.0,
    };
}

/// Increase the ND filter by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NdFilterStepUp;

builtin_request! {
    NdFilterStepUp => NdFilterStepUp: Targeted {
        size: 7,
        wire: |_: &NdFilterStepUp| NdFilterStepCommand::new(NdFilterStep::Up),
    };
}

/// Decrease the ND filter by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NdFilterStepDown;

builtin_request! {
    NdFilterStepDown => NdFilterStepDown: Targeted {
        size: 7,
        wire: |_: &NdFilterStepDown| NdFilterStepCommand::new(NdFilterStep::Down),
    };
}

/// Press Sony's Push-AF control.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PushAfPress;

impl PushAfPress {
    /// Creates a Push-AF press request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

builtin_request! {
    PushAfPress => PushAfPress: AppliedOnly {
        size: 7,
        policy: (Quick, Movement, User),
        wire: |_: &PushAfPress| PushAF::Press,
    };
}

/// Release Sony's Push-AF control.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PushAfRelease;

impl PushAfRelease {
    /// Creates a Push-AF release request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

builtin_request! {
    PushAfRelease => PushAfRelease: AppliedOnly {
        size: 7,
        policy: (Quick, Movement, User),
        wire: |_: &PushAfRelease| PushAF::Release,
    };
}

/// Recall a stored preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetRecall {
    preset: PresetNumber,
    axes: AffectedAxes,
}

impl PresetRecall {
    /// Creates a preset recall operation with profile-selected exact axes.
    pub fn for_profile(preset: PresetNumber, profile: &crate::ProfileSpec) -> Result<Self, Error> {
        let axes = profile
            .preset_recall_axes()
            .ok_or(Error::FeatureNotSupported {
                feature: "preset recall",
            })?;
        Ok(Self { preset, axes })
    }
}

builtin_request! {
    PresetRecall => PresetRecall: Targeted(profile_axes) {
        size: 7,
        policy: (Preset, Preset, User),
        wire: |value: &PresetRecall| PresetCommand {
            action: PresetAction::Recall,
            preset_number: value.preset,
        },
    };
}

impl OperationCommand<completion::Targeted> for PresetRecall {
    fn affected_axes(&self) -> AffectedAxes {
        self.axes
    }
}

/// Store the current state in a preset slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetSet(PresetNumber);

impl PresetSet {
    /// Creates a preset-store command.
    #[must_use]
    pub const fn new(preset: PresetNumber) -> Self {
        Self(preset)
    }
}

builtin_request! {
    PresetSet => PresetSet: Plain {
        size: 7,
        policy: (Preset, Preset, Normal),
        wire: |value: &PresetSet| PresetCommand {
            action: PresetAction::Set,
            preset_number: value.0,
        },
    };
}

/// Clear a stored preset slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetReset(PresetNumber);

impl PresetReset {
    /// Creates a preset-clear command.
    #[must_use]
    pub const fn new(preset: PresetNumber) -> Self {
        Self(preset)
    }
}

builtin_request! {
    PresetReset => PresetReset: Plain {
        size: 7,
        policy: (Preset, Preset, Normal),
        wire: |value: &PresetReset| PresetCommand {
            action: PresetAction::Reset,
            preset_number: value.0,
        },
    };
}

impl BuiltinValidation for PanTiltHome {}

impl BuiltinValidation for PanTiltReset {}

impl BuiltinValidation for PanTiltDrive {
    fn pan_tilt_max_speed(&self) -> Option<u8> {
        Some(self.pan_speed.value().max(self.tilt_speed.value()))
    }

    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_speed(profile, self.pan_speed, self.tilt_speed)
    }
}

impl BuiltinValidation for PanTiltStop {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_speed(profile, self.pan_speed, self.tilt_speed)
    }
}

impl BuiltinValidation for PanTiltAbsolute {
    fn pan_tilt_max_speed(&self) -> Option<u8> {
        Some({
            let (pan, tilt) = self.speeds.values();
            pan.max(tilt)
        })
    }

    fn validate_target(&self, target: &crate::TargetRegistration) -> Result<(), Error> {
        self.validate(target.profile())?;
        if self.position.mount != target.mount() {
            return Err(Error::InvalidRequest {
                reason: "pan/tilt position belongs to a different target mount".into(),
            });
        }
        Ok(())
    }

    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_move_speeds(profile, self.speeds)?;
        self.position.validate(profile)
    }
}

impl BuiltinValidation for PanTiltRelative {
    fn pan_tilt_max_speed(&self) -> Option<u8> {
        Some({
            let (pan, tilt) = self.speeds.values();
            pan.max(tilt)
        })
    }

    fn validate_target(&self, target: &crate::TargetRegistration) -> Result<(), Error> {
        self.validate(target.profile())?;
        target.validate_pan_tilt_offset(self.offset.pan, self.offset.tilt)
    }

    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_move_speeds(profile, self.speeds)?;
        self.offset.validate(profile)
    }
}

impl BuiltinValidation for PanTiltLimitSet {
    fn validate_target(&self, target: &crate::TargetRegistration) -> Result<(), Error> {
        self.validate(target.profile())?;
        if self.position.mount != target.mount() {
            return Err(Error::InvalidRequest {
                reason: "pan/tilt limit belongs to a different target mount".into(),
            });
        }
        Ok(())
    }

    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        self.position.validate(profile)
    }
}

impl BuiltinValidation for PanTiltLimitClear {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        if profile
            .pan_tilt_coordinates()
            .is_none_or(|conversion| conversion.wire_codec() != self.wire_codec)
        {
            return Err(Error::InvalidRequest {
                reason: "pan/tilt limit-clear request was built for a different wire codec".into(),
            });
        }
        Ok(())
    }
}

impl BuiltinValidation for ZoomTarget {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        let zoom = zoom_facts(profile)?;
        let digital = capabilities.supports(TypedSupportSurface::DigitalZoomRange);
        if self.1 == Some(crate::ZoomDomain::OpticalPlusDigital) {
            require(digital, "optical-plus-digital zoom positioning")?;
        }
        let maximum = if digital {
            zoom.digital_max.unwrap_or(zoom.optical_max)
        } else {
            zoom.optical_max
        };
        require_in_range(
            "zoom position",
            self.0.value(),
            &crate::capabilities::CapabilityRange::<u16>::new(0, maximum),
        )
    }
}

impl BuiltinValidation for ZoomDrive {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let speed = match self {
            Self::Tele | Self::Wide => return Ok(()),
            Self::TeleVariable(speed) | Self::WideVariable(speed) => speed.value(),
        };
        let zoom = zoom_facts(profile)?;
        require(zoom.variable, "variable zoom drive")?;
        require_in_range("zoom speed", speed, &zoom.speed)
    }
}

impl BuiltinValidation for ZoomStop {}

impl BuiltinValidation for FocusTarget {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require_in_range(
            "focus position",
            self.0.value(),
            &focus_facts(profile)?.range,
        )
    }
}

impl BuiltinValidation for FocusInfinity {}

impl BuiltinValidation for FocusDrive {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let speed = match self {
            Self::Far | Self::Near => return Ok(()),
            Self::FarVariable(speed) | Self::NearVariable(speed) => speed.value(),
        };
        require_in_range("focus speed", speed, &focus_facts(profile)?.speed)
    }
}

impl BuiltinValidation for FocusStop {}

impl BuiltinValidation for FocusTrigger {}

impl BuiltinValidation for FocusModeCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();

        if matches!(self, Self::Auto | Self::Toggle) {
            require(
                capabilities
                    .focus
                    .as_ref()
                    .is_some_and(|domain| domain.auto_focus),
                "automatic focus",
            )?;
        }
        Ok(())
    }
}

impl BuiltinValidation for IrisReset {}

impl BuiltinValidation for IrisUp {}

impl BuiltinValidation for IrisDown {}

impl BuiltinValidation for IrisDirect {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let range =
            exposure_facts(profile)?
                .iris_range
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "iris range",
                })?;
        require_in_domain("iris level", u16::from(self.0.value()), range)
    }
}

impl BuiltinValidation for NdFilterDirect {}

impl BuiltinValidation for NdFilterStepUp {}

impl BuiltinValidation for NdFilterStepDown {}

impl BuiltinValidation for PushAfPress {}

impl BuiltinValidation for PushAfRelease {}

pub(crate) fn validate_preset(profile: &crate::ProfileSpec, preset: u8) -> Result<(), Error> {
    let facts = presets_facts(profile)?;
    require_in_range(
        "preset number",
        preset,
        &crate::capabilities::CapabilityRange::<u8>::new(0, facts.highest),
    )
}

impl BuiltinValidation for PresetRecall {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.preset.value())?;
        if profile.preset_recall_axes() == Some(self.axes) {
            Ok(())
        } else {
            Err(Error::InvalidRequest {
                reason: "preset recall was prepared for a different profile axis set".into(),
            })
        }
    }
}

impl BuiltinValidation for PresetSet {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.0.value())
    }
}

impl BuiltinValidation for PresetReset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.0.value())
    }
}

impl BuiltinValidation for crate::command::preset_wire::PresetRecallSpeedCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let speed = self.speed.value();
        let range = presets_facts(profile)?.recall_speed_range.as_ref().ok_or(
            Error::FeatureNotSupported {
                feature: "preset recall speed range",
            },
        )?;
        require_in_range("preset recall speed", speed, range)
    }
}

impl BuiltinValidation for crate::command::focus_wire::FocusLockCommand {}

impl BuiltinValidation for crate::command::exposure_wire::SpotlightOn {}

impl BuiltinValidation for crate::command::exposure_wire::SpotlightOff {}

impl BuiltinValidation for crate::command::exposure_wire::AutoSlowShutterOn {}

impl BuiltinValidation for crate::command::exposure_wire::AutoSlowShutterOff {}

impl BuiltinValidation for crate::command::nd_filter_wire::NdFilterModeCommand {}

impl BuiltinValidation for crate::command::nd_filter_wire::AutoNdCommand {}

impl BuiltinValidation for crate::command::flip::ImageFreeze {}

impl BuiltinValidation for crate::command::zoom_wire::DigitalZoom {}

impl BuiltinValidation for crate::command::streaming_wire::MulticastStreamingCommand {}

impl BuiltinValidation for crate::command::streaming_wire::SetNdiQuality {}

impl BuiltinValidation for crate::command::tally_wire::TallyBrightLo {}

impl BuiltinValidation for crate::command::tally_wire::TallyBrightHi {}

impl BuiltinValidation for crate::command::variable_speed::SetVariableSpeedMode {}

impl BuiltinValidation for crate::command::tally_wire::TallyOn {}

impl BuiltinValidation for crate::command::tally_wire::TallyOff {}

impl BuiltinValidation for crate::command::tally_wire::TallyFlash {}

// State keys and verbs come from the ledger; requests supply only typed values.
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::command::ImageFlipMode>
    for crate::command::image_wire::ImageFlipCombinedCommand
{
    fn state_value(&self) -> crate::command::ImageFlipMode {
        self.mode
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::PanTiltLimitUpdate> for PanTiltLimitSet {
    fn state_value(&self) -> crate::PanTiltLimitUpdate {
        crate::PanTiltLimitUpdate::set(
            self.corner,
            crate::camera::PanTiltUnits::new(self.position.pan, self.position.tilt),
        )
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::PanTiltLimitUpdate> for PanTiltLimitClear {
    fn state_value(&self) -> crate::PanTiltLimitUpdate {
        crate::PanTiltLimitUpdate::clear(self.corner)
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::types::PresetSpeed>
    for crate::command::preset_wire::PresetRecallSpeedCommand
{
    fn state_value(&self) -> crate::types::PresetSpeed {
        self.speed
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::focus_wire::FocusLockCommand {
    fn state_value(&self) -> bool {
        self.enabled()
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::exposure_wire::SpotlightOn {
    fn state_value(&self) -> bool {
        true
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::exposure_wire::SpotlightOff {
    fn state_value(&self) -> bool {
        false
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::exposure_wire::AutoSlowShutterOn {
    fn state_value(&self) -> bool {
        true
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::exposure_wire::AutoSlowShutterOff {
    fn state_value(&self) -> bool {
        false
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::command::NdFilterMode>
    for crate::command::nd_filter_wire::NdFilterModeCommand
{
    fn state_value(&self) -> crate::command::NdFilterMode {
        self.mode()
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::nd_filter_wire::AutoNdCommand {
    fn state_value(&self) -> bool {
        self.enabled()
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::flip::ImageFreeze {
    fn state_value(&self) -> bool {
        self.enabled()
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::zoom_wire::DigitalZoom {
    fn state_value(&self) -> bool {
        self.enabled()
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<bool> for crate::command::streaming_wire::MulticastStreamingCommand {
    fn state_value(&self) -> bool {
        self.enabled()
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::types::NdiQuality> for crate::command::streaming_wire::SetNdiQuality {
    fn state_value(&self) -> crate::types::NdiQuality {
        self.quality
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::types::TallyBrightness>
    for crate::command::tally_wire::TallyBrightLo
{
    fn state_value(&self) -> crate::types::TallyBrightness {
        crate::types::TallyBrightness::Low
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::types::TallyBrightness>
    for crate::command::tally_wire::TallyBrightHi
{
    fn state_value(&self) -> crate::types::TallyBrightness {
        crate::types::TallyBrightness::High
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::command::PanTiltSpeedMode>
    for crate::command::variable_speed::SetVariableSpeedMode
{
    fn state_value(&self) -> crate::command::PanTiltSpeedMode {
        self.mode
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::types::TallyMode> for crate::command::tally_wire::TallyOn {
    fn state_value(&self) -> crate::types::TallyMode {
        crate::types::TallyMode::On
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::types::TallyMode> for crate::command::tally_wire::TallyOff {
    fn state_value(&self) -> crate::types::TallyMode {
        crate::types::TallyMode::Off
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::types::TallyMode> for crate::command::tally_wire::TallyFlash {
    fn state_value(&self) -> crate::types::TallyMode {
        crate::types::TallyMode::Flash
    }
}
#[cfg(any(feature = "blocking", feature = "async"))]
impl BuiltinStateValue<crate::types::MotionSyncSpeed>
    for crate::command::motion_sync_wire::SetMotionSyncPreset
{
    fn state_value(&self) -> crate::types::MotionSyncSpeed {
        self.speed()
    }
}

// Built-in request behavior is exercised through shared preparation, which
// exists only with a facade.
#[cfg(all(test, any(feature = "blocking", feature = "async")))]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        command::{
            flip::ImageFreeze, zoom_wire::DigitalZoom, AntiFlickerCommand, AutoNdCommand,
            AutoSlowShutterOff, AutoSlowShutterOn, ExposureCommand, ExposureMode, FocusLockCommand,
            MulticastStreamingCommand, NdFilterMode, NdFilterModeCommand, PanTiltLimitCorner,
            PanTiltSpeedMode, PresetSpeed, SetNdiQuality, SettingsSaveCommand, SpotlightOff,
            SpotlightOn, TallyBrightHi, TallyBrightLo, TallyFlash, TallyOff, TallyOn, TallyRedOn,
            WhiteBalanceMode,
        },
        prepared::{
            test_support::{prepare_builtin_command, prepare_builtin_operation, prepare_command},
            ClassSelection,
        },
        profiles::{
            GenericVisca, NearusBrc300, PtzOptics30x, PtzOpticsG2, PtzOpticsG3, SonyBrc300,
            SonyBrcH900, SonyEviH100, SonyFr7,
        },
        types::{FocusSpeed, NdiQuality},
        OperationalTuning, PlainCommand, ProfileSpec,
    };
    use std::collections::HashSet;

    #[test]
    fn every_zoom_and_focus_drive_variant_selects_its_source_on_all_models() {
        let profiles = [
            ProfileSpec::from_compile_time::<PtzOpticsG2>(),
            ProfileSpec::from_compile_time::<PtzOpticsG3>(),
            ProfileSpec::from_compile_time::<PtzOptics30x>(),
            ProfileSpec::from_compile_time::<SonyFr7>(),
            ProfileSpec::from_compile_time::<SonyBrcH900>(),
            ProfileSpec::from_compile_time::<SonyEviH100>(),
            ProfileSpec::from_compile_time::<SonyBrc300>(),
            ProfileSpec::from_compile_time::<NearusBrc300>(),
            ProfileSpec::from_compile_time::<GenericVisca>(),
        ];
        fn prepared<C: OperationCommand<completion::AppliedOnly> + BuiltinValidation>(
            value: &C,
            target: &crate::TargetRegistration,
            axes: AffectedAxes,
        ) -> Vec<u8> {
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                value,
                target,
                OperationalTuning::new(),
            )
            .expect("source-backed drive must prepare")
            .admit_with(|request, actual_axes, _, _| {
                assert_eq!(actual_axes, axes);
                request.wire().as_bytes().to_vec()
            })
        }
        for profile in profiles {
            let profile = profile.expect("built-in profile");
            let zoom = profile.capabilities().zoom.as_ref().expect("zoom").speed;
            let focus = profile.capabilities().focus.as_ref().expect("focus").speed;
            let target =
                crate::TargetRegistration::new(CameraId::CAMERA_1, profile).expect("target");
            for (value, row, parameter) in [
                (ZoomDrive::Tele, BuiltinCommand::ZoomTele, 0x02),
                (ZoomDrive::Wide, BuiltinCommand::ZoomWide, 0x03),
                (
                    ZoomDrive::TeleVariable(ZoomSpeed::new(zoom.min()).expect("minimum")),
                    BuiltinCommand::ZoomTeleVariable,
                    0x20 | zoom.min(),
                ),
                (
                    ZoomDrive::TeleVariable(ZoomSpeed::new(zoom.max()).expect("maximum")),
                    BuiltinCommand::ZoomTeleVariable,
                    0x20 | zoom.max(),
                ),
                (
                    ZoomDrive::WideVariable(ZoomSpeed::new(zoom.min()).expect("minimum")),
                    BuiltinCommand::ZoomWideVariable,
                    0x30 | zoom.min(),
                ),
                (
                    ZoomDrive::WideVariable(ZoomSpeed::new(zoom.max()).expect("maximum")),
                    BuiltinCommand::ZoomWideVariable,
                    0x30 | zoom.max(),
                ),
            ] {
                assert_eq!(value.ledger_row(), row, "{}", target.profile().name());
                let frame = prepared(&value, &target, AffectedAxes::ZOOM);
                assert_eq!(
                    frame,
                    [0x81, 1, 4, 7, parameter, 0xff],
                    "{} {row:?}",
                    target.profile().name()
                );
                assert!(row
                    .packet()
                    .expect("selected zoom pattern")
                    .validate_frame(&frame, "selected zoom source")
                    .is_ok());
            }
            for (value, row, parameter) in [
                (FocusDrive::Far, BuiltinCommand::FocusFar, 0x02),
                (FocusDrive::Near, BuiltinCommand::FocusNear, 0x03),
                (
                    FocusDrive::FarVariable(FocusSpeed::new(focus.min()).expect("minimum")),
                    BuiltinCommand::FocusFarVariable,
                    0x20 | focus.min(),
                ),
                (
                    FocusDrive::FarVariable(FocusSpeed::new(focus.max()).expect("maximum")),
                    BuiltinCommand::FocusFarVariable,
                    0x20 | focus.max(),
                ),
                (
                    FocusDrive::NearVariable(FocusSpeed::new(focus.min()).expect("minimum")),
                    BuiltinCommand::FocusNearVariable,
                    0x30 | focus.min(),
                ),
                (
                    FocusDrive::NearVariable(FocusSpeed::new(focus.max()).expect("maximum")),
                    BuiltinCommand::FocusNearVariable,
                    0x30 | focus.max(),
                ),
            ] {
                assert_eq!(value.ledger_row(), row, "{}", target.profile().name());
                let frame = prepared(&value, &target, AffectedAxes::FOCUS);
                assert_eq!(
                    frame,
                    [0x81, 1, 4, 8, parameter, 0xff],
                    "{} {row:?}",
                    target.profile().name()
                );
                assert!(row
                    .packet()
                    .expect("selected focus pattern")
                    .validate_frame(&frame, "selected focus source")
                    .is_ok());
            }
            for (wrong, frame) in [
                (BuiltinCommand::ZoomTele, [0x81, 1, 4, 7, 3, 0xff]),
                (
                    BuiltinCommand::ZoomTele,
                    [0x81, 1, 4, 7, 0x20 | zoom.min(), 0xff],
                ),
                (BuiltinCommand::FocusFar, [0x81, 1, 4, 8, 3, 0xff]),
                (
                    BuiltinCommand::FocusFar,
                    [0x81, 1, 4, 8, 0x20 | focus.min(), 0xff],
                ),
                (
                    BuiltinCommand::ZoomTeleVariable,
                    [0x81, 1, 4, 7, 0x30 | zoom.min(), 0xff],
                ),
                (
                    BuiltinCommand::ZoomWideVariable,
                    [0x81, 1, 4, 7, 0x20 | zoom.max(), 0xff],
                ),
                (
                    BuiltinCommand::ZoomTeleVariable,
                    [0x81, 1, 4, 7, 0x02, 0xff],
                ),
                (
                    BuiltinCommand::FocusFarVariable,
                    [0x81, 1, 4, 8, 0x30 | focus.min(), 0xff],
                ),
                (
                    BuiltinCommand::FocusNearVariable,
                    [0x81, 1, 4, 8, 0x20 | focus.max(), 0xff],
                ),
                (
                    BuiltinCommand::FocusFarVariable,
                    [0x81, 1, 4, 8, 0x02, 0xff],
                ),
            ] {
                assert!(
                    wrong
                        .packet()
                        .expect("sourced selector pattern")
                        .validate_frame(&frame, "wrong source key")
                        .is_err(),
                    "{} {wrong:?}",
                    target.profile().name()
                );
            }
        }
    }

    #[test]
    fn selected_source_still_rejects_a_model_without_that_packet() {
        let ptz = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
        )
        .expect("target");
        reset_request_write_count();
        assert!(matches!(
            prepare_builtin_operation::<completion::Targeted, _>(
                &FocusInfinity,
                &ptz,
                OperationalTuning::new()
            ),
            Err(Error::FeatureNotSupported { .. })
        ));
        assert_eq!(
            request_write_count(),
            0,
            "model source denial precedes encoding"
        );
        let sony = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyEviH100>().expect("EVI"),
        )
        .expect("target");
        let bytes = prepare_builtin_operation::<completion::Targeted, _>(
            &FocusInfinity,
            &sony,
            OperationalTuning::new(),
        )
        .expect("EVI infinity source")
        .admit_with(|request, _, _, _| request.wire().as_bytes().to_vec());
        assert_eq!(bytes, [0x81, 1, 4, 0x18, 2, 0xff]);
    }

    fn wire<R: Request>(request: &R) -> Vec<u8> {
        let mut buffer = [0_u8; 32];
        let length = request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("typed request must encode");
        buffer[..length].to_vec()
    }

    #[test]
    fn relative_pan_tilt_uses_reachable_span_instead_of_absolute_limits() {
        let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2");
        let request = PanTiltRelative::for_target(
            Degrees::new(200.0).expect("finite degrees"),
            Degrees::new(-45.0).expect("finite degrees"),
            PanSpeed::new(10).unwrap(),
            TiltSpeed::new(10).unwrap(),
            &crate::TargetRegistration::new(CameraId::CAMERA_1, profile.clone())
                .expect("target registration"),
        )
        .expect("reachable relative offset");
        assert_eq!(&wire(&request)[10..14], [15, 13, 7, 8]);
        request.validate(&profile).expect("offset validation");
        let conversion = profile.pan_tilt_coordinates().unwrap();
        let caps = profile.capabilities().pan_tilt.as_ref().expect("pan/tilt");
        let target =
            crate::TargetRegistration::new(CameraId::CAMERA_1, profile.clone()).expect("target");
        for (pan, tilt) in [
            (caps.pan_range.max() - caps.pan_range.min(), 0),
            (
                0,
                caps.tilt_ranges.range(None).max() - caps.tilt_ranges.range(None).min(),
            ),
        ] {
            target
                .convert_pan_tilt_offset_degrees(
                    conversion.pan_degrees(pan),
                    conversion.tilt_degrees(tilt),
                )
                .expect("full span");
            assert!(matches!(
                target.validate_pan_tilt_offset(pan + 1, tilt + 1),
                Err(Error::InvalidParameter { .. })
            ));
        }
        assert!(matches!(
            target.convert_pan_tilt_offset_degrees(f32::NAN, 0.0),
            Err(Error::InvalidParameter { .. })
        ));
    }

    fn command_wire<C: WireEncode>(command: &C) -> Vec<u8> {
        let mut buffer = [0_u8; 32];
        let length = command
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("command must encode");
        buffer[..length].to_vec()
    }

    /// Copy complete protocol facts while retaining only selections whose
    /// grants the caller preserved in its deliberately altered test profile.
    fn runtime_profile_builder(
        source: &ProfileSpec,
        capabilities: crate::capabilities::Capabilities,
    ) -> crate::profiles::ProfileSpecBuilder {
        let grants = capabilities.typed_support;
        let mut builder = ProfileSpec::builder(capabilities)
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .position_inquiries(source.position_inquiries());
        for encoding in source
            .encodings()
            .iter()
            .filter(|value| grants.contains(value.surface()))
        {
            builder = builder.encoding(encoding);
        }
        builder
    }

    fn assert_profile_range_error(
        result: Result<(), Error>,
        parameter: &str,
        value: i32,
        range: (i32, i32),
    ) {
        match result {
            Err(Error::ParameterOutOfRange {
                parameter: reported,
                value: reported_value,
                min,
                max,
            }) => {
                assert_eq!(reported, parameter);
                assert_eq!((reported_value, min, max), (value, range.0, range.1));
            }
            other => panic!("expected ParameterOutOfRange for {parameter}, got {other:?}"),
        }
    }

    #[test]
    fn profile_range_checks_report_the_profiles_inclusive_bounds() {
        let g2 = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let capabilities = g2.capabilities();

        let gain = capabilities
            .exposure
            .as_ref()
            .expect("exposure")
            .gain_range
            .expect("gain")
            .as_inclusive();
        let above_gain = *gain.end() + 1;
        assert_profile_range_error(
            validate_gain(&g2, Some(above_gain)),
            "gain level",
            i32::from(above_gain),
            (i32::from(*gain.start()), i32::from(*gain.end())),
        );

        let above_presets = capabilities.presets.as_ref().expect("presets").highest + 1;
        assert_profile_range_error(
            validate_preset(&g2, above_presets),
            "preset number",
            i32::from(above_presets),
            (
                0,
                i32::from(capabilities.presets.as_ref().expect("presets").highest),
            ),
        );

        // The G2 tilt speed range is narrower than the syntactic `TiltSpeed`
        // domain, so the profile check is the one that rejects.
        let tilt = capabilities
            .pan_tilt
            .as_ref()
            .expect("pan/tilt")
            .tilt_speed
            .as_inclusive();
        let above_tilt = *tilt.end() + 1;
        assert_profile_range_error(
            validate_pan_tilt_speed(
                &g2,
                PanSpeed::new(
                    capabilities
                        .pan_tilt
                        .as_ref()
                        .expect("pan/tilt")
                        .pan_speed
                        .min(),
                )
                .expect("pan speed"),
                TiltSpeed::new(above_tilt).expect("syntactic tilt speed"),
            ),
            "tilt speed",
            i32::from(above_tilt),
            (i32::from(*tilt.start()), i32::from(*tilt.end())),
        );
    }

    #[test]
    fn usb_audio_direct_validation_follows_the_registry_capability_fact() {
        let g2 = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let g3 = ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile");
        let thirty_x = ProfileSpec::from_compile_time::<PtzOptics30x>().expect("30X profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");

        for profile in [&g2, &thirty_x] {
            assert!(prepare_builtin_command(
                &crate::command::UsbAudioCommand::new(true),
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    profile.clone()
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .is_ok());
        }

        for profile in [&g3, &fr7, &generic] {
            assert!(prepare_builtin_command(
                &crate::command::UsbAudioCommand::new(true),
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    profile.clone()
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .is_err());
        }
    }

    #[test]
    fn vendor_command_validation_follows_the_static_typed_surface() {
        let g2 = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let g3 = ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile");
        let thirty_x = ProfileSpec::from_compile_time::<PtzOptics30x>().expect("30X profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBrcH900>().expect("H900 profile");
        let evi = ProfileSpec::from_compile_time::<SonyEviH100>().expect("EVI profile");
        let brc300 = ProfileSpec::from_compile_time::<SonyBrc300>().expect("BRC300 profile");
        let nearus = ProfileSpec::from_compile_time::<NearusBrc300>().expect("Nearus profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        let profiles = [
            &g2, &g3, &thirty_x, &fr7, &h900, &evi, &brc300, &nearus, &generic,
        ];

        // `prepare_builtin_command` admits plain requests; an operation names
        // its completion kind and goes through `prepare_builtin_operation`.
        macro_rules! follows_static_surface {
            ($surface:expr, $request:expr) => {
                follows_static_surface!(@prepare [prepare_builtin_command] $surface, $request)
            };
            ($kind:ty: $surface:expr, $request:expr) => {
                follows_static_surface!(
                    @prepare [prepare_builtin_operation::<$kind, _>] $surface, $request
                )
            };
            (@prepare [$($prepare:tt)+] $surface:expr, $request:expr) => {
                for profile in profiles {
                    let request = $request;
                    assert_eq!(
                        $($prepare)+(
                            &request,
                            &crate::TargetRegistration::from_profile(crate::target::TargetSlot::CAMERA_1, profile.clone()),
                            OperationalTuning::new(),
                        )
                        .is_ok(),
                        profile.capabilities().supports($surface),
                        "{request:?} runtime validation must match {:?} for {}",
                        $surface,
                        profile.capabilities().model_name,
                    );
                }
            };
        }

        follows_static_surface!(
            TypedSupportSurface::AntiFlicker,
            AntiFlickerCommand::new(crate::command::AntiFlickerMode::Hz50)
        );
        follows_static_surface!(
            TypedSupportSurface::SettingsSave,
            SettingsSaveCommand::new()
        );
        for profile in profiles {
            let target = crate::TargetRegistration::from_profile(
                crate::target::TargetSlot::CAMERA_1,
                profile.clone(),
            );
            let result = crate::command::presets::SetRecallSpeed::for_target(
                &target,
                PresetSpeed::new(12).expect("valid recall speed"),
            )
            .and_then(|request| {
                prepare_builtin_command(&request, &target, OperationalTuning::new()).map(drop)
            });
            assert_eq!(
                result.is_ok(),
                profile
                    .capabilities()
                    .supports(TypedSupportSurface::PresetRecallSpeed),
                "canonical recall speed uses the selected encoding for {}: {result:?}",
                profile.name()
            );
        }
        follows_static_surface!(TypedSupportSurface::Spotlight, SpotlightOn::new());
        follows_static_surface!(TypedSupportSurface::Spotlight, SpotlightOff::new());
        follows_static_surface!(
            TypedSupportSurface::AutoSlowShutter,
            AutoSlowShutterOn::new()
        );
        follows_static_surface!(
            TypedSupportSurface::AutoSlowShutter,
            AutoSlowShutterOff::new()
        );
        follows_static_surface!(
            TypedSupportSurface::MulticastStreaming,
            MulticastStreamingCommand::new(true)
        );
        follows_static_surface!(
            TypedSupportSurface::MulticastStreaming,
            MulticastStreamingCommand::new(false)
        );
        follows_static_surface!(
            TypedSupportSurface::NdiQuality,
            SetNdiQuality::new(NdiQuality::High)
        );
        follows_static_surface!(
            completion::AppliedOnly: TypedSupportSurface::OnePushFocus,
            FocusTrigger::OnePush
        );
        follows_static_surface!(
            completion::AppliedOnly: TypedSupportSurface::SnapFocus,
            FocusTrigger::Snap
        );
    }

    /// Modes are a profile value inventory. Optional white-balance action
    /// grants do not redefine which mode values the camera accepts.
    #[test]
    fn white_balance_mode_families_admit_only_sourced_profile_values() {
        use crate::command::white_balance::SetMode;
        for model in crate::profiles::ProfileId::all() {
            let profile = model.spec().expect("built-in profile");
            let target = crate::TargetRegistration::new(CameraId::CAMERA_1, profile.clone())
                .expect("target");
            let modes = &profile
                .capabilities()
                .white_balance
                .as_ref()
                .expect("white-balance domain")
                .modes;
            let mut admitted = 0;
            for code in u8::MIN..=u8::MAX {
                let Ok(mode) = WhiteBalanceMode::from_wire(code) else {
                    continue;
                };
                let request = SetMode::new(mode);
                let result = request.validate_for_target(&target);
                assert_eq!(
                    result.is_ok(),
                    modes.contains(&mode),
                    "{model:?} {mode:?}: {result:?}"
                );
                if result.is_err() {
                    continue;
                }
                admitted += 1;
                let row = request
                    .selected_ledger_row_for_target(&target)
                    .expect("selected mode row");
                assert!(<SetMode as BuiltinRequestContract>::LEDGER_ROWS.contains(&row));
                assert!(
                    row.source().covers_model(Some(*model)),
                    "{model:?} {mode:?} lacks its selected source"
                );
                let mut buffer = [0; 16];
                let written = crate::requests::write_for_target(&request, &target, &mut buffer)
                    .expect("source-backed mode frame");
                row.packet()
                    .expect("sourced mode packet")
                    .validate_frame(&buffer[..written], "admitted white-balance mode")
                    .expect("selected source packet");
            }
            assert!(admitted > 0, "{model:?} mode proof must not be vacuous");
        }
    }

    #[test]
    fn profile_aware_speed_level_lowering_keeps_standard_pairs_and_mirrors_brc300() {
        let standard = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let g3 = ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile");
        let thirty_x = ProfileSpec::from_compile_time::<PtzOptics30x>().expect("30X profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBrcH900>().expect("H900 profile");
        let evi = ProfileSpec::from_compile_time::<SonyEviH100>().expect("EVI profile");
        let brc300 = ProfileSpec::from_compile_time::<SonyBrc300>().expect("BRC-300 profile");
        let nearus = ProfileSpec::from_compile_time::<NearusBrc300>().expect("Nearus profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");

        let standard_absolute = PanTiltAbsolute::for_target_speed_level(
            Degrees::new(10.0).expect("finite degrees"),
            Degrees::new(-5.0).expect("finite degrees"),
            SpeedLevel::Medium,
            &crate::TargetRegistration::new(CameraId::CAMERA_1, standard.clone())
                .expect("target registration"),
        )
        .expect("standard coarse absolute position");
        assert_eq!(
            wire(&standard_absolute),
            vec![
                0x81, 0x01, 0x06, 0x02, 0x0C, 0x0A, 0x00, 0x00, 0x09, 0x00, 0x0F, 0x0F, 0x0B, 0x08,
                0xFF,
            ]
        );

        let brc300_absolute = PanTiltAbsolute::for_target_speed_level(
            Degrees::new(45.0).expect("finite degrees"),
            Degrees::new(15.0).expect("finite degrees"),
            SpeedLevel::Fastest,
            &crate::TargetRegistration::new(CameraId::CAMERA_1, brc300.clone())
                .expect("target registration"),
        )
        .expect("BRC-300 coarse absolute position");
        assert_eq!(
            wire(&brc300_absolute),
            vec![
                0x81, 0x01, 0x06, 0x02, 0x18, 0x00, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
                0x00, 0xFF,
            ]
        );

        let brc300_relative = PanTiltRelative::for_target_speed_level(
            Degrees::new(45.0).expect("finite degrees"),
            Degrees::new(15.0).expect("finite degrees"),
            SpeedLevel::Fastest,
            &crate::TargetRegistration::new(CameraId::CAMERA_1, brc300.clone())
                .expect("target registration"),
        )
        .expect("BRC-300 coarse relative position");
        assert_eq!(
            wire(&brc300_relative),
            vec![
                0x81, 0x01, 0x06, 0x03, 0x18, 0x00, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
                0x00, 0xFF,
            ]
        );

        for profile in [
            &standard, &g3, &thirty_x, &fr7, &h900, &evi, &brc300, &nearus, &generic,
        ] {
            PanTiltAbsolute::for_target_speed_level(
                Degrees::new(0.0).expect("finite degrees"),
                Degrees::new(0.0).expect("finite degrees"),
                SpeedLevel::Fastest,
                &crate::TargetRegistration::new(CameraId::CAMERA_1, profile.clone())
                    .expect("target registration"),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{} must lower SpeedLevel::Fastest to a valid absolute request: {error}",
                    profile.capabilities().model_name
                )
            });
            PanTiltRelative::for_target_speed_level(
                Degrees::new(0.0).expect("finite degrees"),
                Degrees::new(0.0).expect("finite degrees"),
                SpeedLevel::Fastest,
                &crate::TargetRegistration::new(CameraId::CAMERA_1, profile.clone())
                    .expect("target registration"),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{} must lower SpeedLevel::Fastest to a valid relative request: {error}",
                    profile.capabilities().model_name
                )
            });
        }

        assert_eq!(
            pan_tilt_position_speeds_from_level(SpeedLevel::Fastest, &evi)
                .expect("EVI-H100 fastest speed"),
            (
                // R8: pan `01`..`18`, tilt `01`..`17`; `Fastest` tilt is 0x14.
                PanSpeed::new(0x18).expect("EVI pan maximum"),
                TiltSpeed::new(0x14).expect("fastest coarse tilt speed"),
            )
        );
        assert_eq!(
            pan_tilt_position_speeds_from_level(SpeedLevel::Fastest, &nearus)
                .expect("Nearus fastest speed"),
            (
                PanSpeed::new(0x18).expect("Nearus one-speed maximum (R21 `01`..`18h`)"),
                TiltSpeed::new(0x18).expect("Nearus one-speed maximum (R21 `01`..`18h`)"),
            )
        );
    }

    /// Every preset and typed direct-menu value encodes to one terminated frame.
    /// Preset 255 exercises a final data byte equal to the terminator (#683).
    #[test]
    fn preset_and_direct_menu_value_domains_stay_terminated() {
        use crate::command::menu_wire::DirectMenuControl;

        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");

        for number in 0..=u8::MAX {
            let preset = PresetNumber::new(number).expect("preset number in range");

            // Plain requests: PresetSet / PresetReset.
            for (label, request_bytes, action) in [
                ("PresetSet", wire(&PresetSet::new(preset)), 0x01u8),
                ("PresetReset", wire(&PresetReset::new(preset)), 0x00u8),
            ] {
                assert_eq!(request_bytes.len(), 7, "{label} {number} length");
                assert_eq!(
                    request_bytes,
                    vec![0x81, 0x01, 0x04, 0x3F, action, number, 0xFF],
                    "{label} {number} wire bytes"
                );
            }

            // Targeted operation: PresetRecall (needs the profile for its axes).
            let recall = PresetRecall::for_profile(preset, &fr7).expect("FR7 preset recall");
            let recall_bytes = wire(&recall);
            assert_eq!(
                recall_bytes.len(),
                7,
                "PresetRecall {number} terminated length"
            );
            assert_eq!(
                recall_bytes,
                vec![0x81, 0x01, 0x04, 0x3F, 0x02, number, 0xFF],
                "PresetRecall {number} wire bytes"
            );
        }

        // Every documented direct-menu target and button writes exactly once.
        for target in [
            crate::command::DirectMenuTarget::NdFilter,
            crate::command::DirectMenuTarget::Exit,
        ] {
            for button in [
                crate::command::MenuButton::Release,
                crate::command::MenuButton::Press,
            ] {
                let command = DirectMenuControl::new(target, button);
                let bytes = wire(&command);
                assert_eq!(
                    bytes,
                    [
                        0x81,
                        1,
                        0x7E,
                        4,
                        0x72,
                        target.to_wire(),
                        button.to_wire(),
                        0xFF
                    ]
                );
            }
        }
    }

    /// FR7 admits its documented highest preset and refuses larger slots
    /// before encoding, while retaining its distinct direct-menu command.
    #[test]
    fn fr7_preset_boundary_and_direct_menu_prepare_with_source_backed_facts() {
        use crate::command::menu_wire::DirectMenuControl;

        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
        let registration = crate::TargetRegistration::new(CameraId::CAMERA_1, fr7.clone())
            .expect("target registration");
        let highest = PresetNumber::new(99).expect("R7 highest preset");
        prepare_builtin_command(
            &PresetSet::new(highest),
            &registration,
            OperationalTuning::new(),
        )
        .expect("R7 highest preset set");
        prepare_builtin_command(
            &PresetReset::new(highest),
            &registration,
            OperationalTuning::new(),
        )
        .expect("R7 highest preset reset");
        prepare_builtin_operation::<completion::Targeted, _>(
            &PresetRecall::for_profile(highest, &fr7).expect("FR7 preset axes"),
            &registration,
            OperationalTuning::new(),
        )
        .expect("R7 highest preset recall");

        for value in [100, 255] {
            let preset = PresetNumber::new(value).expect("global wire-value type");
            reset_request_write_count();
            let attempts = [
                prepare_builtin_command(
                    &PresetSet::new(preset),
                    &registration,
                    OperationalTuning::new(),
                )
                .map(drop),
                prepare_builtin_command(
                    &PresetReset::new(preset),
                    &registration,
                    OperationalTuning::new(),
                )
                .map(drop),
                prepare_builtin_operation::<completion::Targeted, _>(
                    &PresetRecall::for_profile(preset, &fr7).expect("FR7 preset axes"),
                    &registration,
                    OperationalTuning::new(),
                )
                .map(drop),
            ];
            for result in attempts {
                assert!(
                    matches!(result, Err(Error::ParameterOutOfRange { parameter: "preset number", value: actual, min: 0, max: 99 }) if actual == i32::from(value))
                );
            }
            assert_eq!(
                request_write_count(),
                0,
                "out-of-profile presets never encode"
            );
        }
        prepare_builtin_command(
            &DirectMenuControl::new(
                crate::command::DirectMenuTarget::Exit,
                crate::command::MenuButton::Release,
            ),
            &registration,
            OperationalTuning::new(),
        )
        .expect("documented FR7 direct menu control");
    }

    /// Guard for #683: the fix must not double-terminate the complete-command
    /// constants that are loaded through `from_prefix` already carrying a
    /// terminator. Their frames must be byte-identical to before the fix.
    #[test]
    fn pre_terminated_constants_are_not_double_terminated() {
        assert_eq!(wire(&PanTiltHome), vec![0x81, 0x01, 0x06, 0x04, 0xFF]);
        assert_eq!(wire(&PanTiltReset), vec![0x81, 0x01, 0x06, 0x05, 0xFF]);
        assert_eq!(wire(&ZoomStop), vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]);
        assert_eq!(
            wire(&ZoomDrive::Tele),
            vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]
        );
        assert_eq!(
            wire(&ZoomDrive::Wide),
            vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]
        );
    }

    fn prepared_state<C>(
        command: &C,
        profile: &ProfileSpec,
    ) -> crate::runtime::engine::AppliedStateProjection
    where
        C: PlainCommand + ?Sized,
    {
        prepare_command(
            command,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                profile.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("state command must prepare")
        .admit_with(|request, _timeout| match request {
            crate::runtime::engine::RuntimeRequest::Command {
                applied_state: Some(state),
                ..
            } => state,
            crate::runtime::engine::RuntimeRequest::Command {
                applied_state: None,
                ..
            } => panic!("state command prepared without an applied-state projection"),
            crate::runtime::engine::RuntimeRequest::Inquiry { .. } => {
                panic!("plain state command prepared as an inquiry")
            }
        })
    }

    fn assert_state_row<C>(
        name: &str,
        command: &C,
        profile: &ProfileSpec,
        semantic: BuiltinCommand,
        expected: crate::runtime::engine::AppliedStateProjection,
    ) where
        C: PlainCommand + ?Sized,
    {
        let actual = prepared_state(command, profile);
        assert_eq!(actual, expected, "{name} {semantic:?} applied-state value");
    }

    // Some command rows deliberately have no built-in profile permission yet
    // (for example image-freeze and the unevidenced tally families).  Their
    // closed projection is still part of the semantic ledger, so verify that
    // mapping independently of profile validation.  The runtime rejection of
    // those rows remains covered by the profile-gating tests below.
    fn assert_semantic_state_row<C>(
        name: &str,
        command: &C,
        semantic: BuiltinCommand,
        expected: crate::runtime::engine::AppliedStateProjection,
    ) where
        C: Request + ?Sized,
    {
        let actual = command
            .applied_state_projection(crate::requests::AppliedStateAuthority::new())
            .expect("state command must expose an applied-state projection");
        assert_eq!(actual, expected, "{name} {semantic:?} applied-state value");
    }

    #[test]
    fn every_write_only_state_command_has_an_exact_closed_projection() {
        use crate::command::semantics::BuiltinCommand as B;
        use crate::StateKey as S;

        // Coverage comes from the closed ledger and generated request inventory,
        // not the value examples below. A newly stateful row participates here
        // automatically; its generated hook must also type-check in every build.
        for row in BuiltinCommand::ALL {
            let expected = row
                .classification()
                .state_effect()
                .map(|effect| effect.state());
            let entries = BUILTIN_TYPED_REQUEST_INVENTORY
                .iter()
                .filter(|entry| entry.row == *row);
            let mut covered = false;
            for entry in entries {
                covered = true;
                assert_eq!(entry.state_key, expected, "{row:?} generated state key");
            }
            assert!(covered, "{row:?} has no generated request projection");
        }

        let ptz = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZ profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
        let evi = ProfileSpec::from_compile_time::<SonyEviH100>().expect("EVI profile");

        macro_rules! row {
            ($name:literal, $command:expr, $profile:expr, $semantic:expr, $expected:expr) => {{
                let command = $command;
                assert_state_row($name, &command, $profile, $semantic, $expected);
            }};
        }

        macro_rules! semantic_row {
            ($name:literal, $command:expr, $semantic:expr, $expected:expr) => {{
                let command = $command;
                assert_semantic_state_row($name, &command, $semantic, $expected);
            }};
        }

        // Limits carry their typed corner and optional converted position.
        row!(
            "pan/tilt limit set",
            PanTiltLimitSet::for_target(
                PanTiltLimitCorner::DownLeft,
                Degrees::new(0.0).expect("finite degrees"),
                Degrees::new(0.0).expect("finite degrees"),
                &crate::TargetRegistration::new(CameraId::CAMERA_1, ptz.clone())
                    .expect("target registration"),
            )
            .expect("limit set"),
            &ptz,
            B::PanTiltLimitSet,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::PanTiltLimit(
                crate::PanTiltLimitUpdate::set(
                    PanTiltLimitCorner::DownLeft,
                    crate::camera::PanTiltUnits::new(0, 0)
                )
            ))
        );
        row!(
            "pan/tilt limit clear",
            PanTiltLimitClear::new(PanTiltLimitCorner::UpRight),
            &ptz,
            B::PanTiltLimitClear,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::PanTiltLimit(
                crate::PanTiltLimitUpdate::clear(PanTiltLimitCorner::UpRight)
            ))
        );

        row!(
            "preset recall speed",
            crate::command::PresetRecallSpeedCommand {
                speed: PresetSpeed::new(12).expect("preset speed"),
            },
            &ptz,
            B::PresetRecallSpeed,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::PresetsRecallSpeed(PresetSpeed::new(12).expect("preset speed"))
            )
        );
        row!(
            "focus lock on",
            FocusLockCommand::new(true),
            &ptz,
            B::FocusLock,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::FocusLock(true))
        );
        row!(
            "focus lock off",
            FocusLockCommand::new(false),
            &ptz,
            B::FocusLock,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::FocusLock(
                false
            ))
        );

        row!(
            "spotlight on",
            SpotlightOn::new(),
            &fr7,
            B::SpotlightOn,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::ExposureSpotlight(true)
            )
        );
        row!(
            "spotlight off",
            SpotlightOff::new(),
            &fr7,
            B::SpotlightOff,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::ExposureSpotlight(false)
            )
        );
        row!(
            "auto slow shutter on",
            AutoSlowShutterOn::new(),
            &evi,
            B::AutoSlowShutterOn,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::ExposureAutoSlowShutter(true)
            )
        );
        row!(
            "auto slow shutter off",
            AutoSlowShutterOff::new(),
            &evi,
            B::AutoSlowShutterOff,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::ExposureAutoSlowShutter(false)
            )
        );

        row!(
            "ND filter preset mode",
            NdFilterModeCommand::new(NdFilterMode::Preset),
            &fr7,
            B::NdFilterMode,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::NdFilterMode(
                NdFilterMode::Preset
            ))
        );
        row!(
            "ND filter variable mode",
            NdFilterModeCommand::new(NdFilterMode::Variable),
            &fr7,
            B::NdFilterMode,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::NdFilterMode(
                NdFilterMode::Variable
            ))
        );
        row!(
            "automatic ND on",
            AutoNdCommand::new(true),
            &fr7,
            B::NdFilterAutoOn,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::NdFilterAuto(
                true
            ))
        );
        row!(
            "automatic ND off",
            AutoNdCommand::new(false),
            &fr7,
            B::NdFilterAutoOff,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::NdFilterAuto(
                false
            ))
        );

        semantic_row!(
            "image freeze on",
            ImageFreeze::on(),
            B::ImageFreezeOn,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::ImageFreeze(
                true
            ))
        );
        semantic_row!(
            "image freeze off",
            ImageFreeze::off(),
            B::ImageFreezeOff,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::ImageFreeze(
                false
            ))
        );
        row!(
            "digital zoom on",
            DigitalZoom::new(true),
            &evi,
            B::DigitalZoom,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::ZoomDigitalZoom(true)
            )
        );
        row!(
            "digital zoom off",
            DigitalZoom::new(false),
            &evi,
            B::DigitalZoom,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::ZoomDigitalZoom(false)
            )
        );

        row!(
            "multicast on",
            MulticastStreamingCommand::new(true),
            &ptz,
            B::MulticastStreamingOn,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::StreamingMulticast(true)
            )
        );
        row!(
            "multicast off",
            MulticastStreamingCommand::new(false),
            &ptz,
            B::MulticastStreamingOff,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::StreamingMulticast(false)
            )
        );
        for (name, quality) in [
            ("high", NdiQuality::High),
            ("medium", NdiQuality::Medium),
            ("low", NdiQuality::Low),
            ("off", NdiQuality::Off),
        ] {
            let command = SetNdiQuality::new(quality);
            assert_state_row(
                &format!("NDI quality {name}"),
                &command,
                &ptz,
                B::NdiQuality,
                crate::runtime::engine::AppliedStateProjection::set(
                    crate::KnownState::StreamingNdiQuality(quality),
                ),
            );
        }

        semantic_row!(
            "tally brightness low",
            TallyBrightLo::new(),
            B::TallyBrightLow,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::TallyBrightness(crate::types::TallyBrightness::Low)
            )
        );
        semantic_row!(
            "tally brightness high",
            TallyBrightHi::new(),
            B::TallyBrightHigh,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::TallyBrightness(crate::types::TallyBrightness::High)
            )
        );
        row!(
            "variable speed standard",
            crate::command::SetVariableSpeedMode::new(PanTiltSpeedMode::Standard24),
            &fr7,
            B::VariableSpeedMode,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::PanTiltSpeedMode(PanTiltSpeedMode::Standard24)
            )
        );
        row!(
            "variable speed fine",
            crate::command::SetVariableSpeedMode::new(PanTiltSpeedMode::Fine50),
            &fr7,
            B::VariableSpeedMode,
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::PanTiltSpeedMode(PanTiltSpeedMode::Fine50)
            )
        );

        // PTZOptics tally-mode opcodes retain their closed state projections
        // even though no built-in profile currently opts into their dedicated
        // typed-support surface.
        semantic_row!(
            "tally on",
            TallyOn::new(),
            B::TallyOn,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::TallyMode(
                crate::types::TallyMode::On
            ))
        );
        semantic_row!(
            "tally off",
            TallyOff::new(),
            B::TallyOff,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::TallyMode(
                crate::types::TallyMode::Off
            ))
        );
        semantic_row!(
            "tally flash",
            TallyFlash::new(),
            B::TallyFlash,
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::TallyMode(
                crate::types::TallyMode::Flash
            ))
        );

        // The combined-flip opcode carries both axes, so it records the pair
        // as `[horizontal, vertical]`. The single-axis opcodes move one axis
        // and leave the other unknown, so they invalidate the same key.
        for (name, mode) in [
            ("combined flip off", crate::command::ImageFlipMode::Off),
            (
                "combined flip horizontal",
                crate::command::ImageFlipMode::Horizontal,
            ),
            (
                "combined flip vertical",
                crate::command::ImageFlipMode::Vertical,
            ),
            ("combined flip both", crate::command::ImageFlipMode::Both),
        ] {
            assert_state_row(
                name,
                &crate::command::ImageFlipCombinedCommand::new(mode),
                &ptz,
                B::ImageFlipCombined,
                crate::runtime::engine::AppliedStateProjection::set(
                    crate::KnownState::ImageFlipMode(mode),
                ),
            );
        }
        row!(
            "separate vertical flip",
            ImageFlipCommand::new(true),
            &ptz,
            B::ImageFlipVertical,
            crate::runtime::engine::AppliedStateProjection::invalidate(S::ImageFlipMode)
        );
        row!(
            "separate horizontal mirror",
            ImageMirrorCommand::new(true),
            &ptz,
            B::ImageFlipHorizontal,
            crate::runtime::engine::AppliedStateProjection::invalidate(S::ImageFlipMode)
        );
        row!(
            "separate horizontal mirror off",
            ImageMirrorCommand::new(false),
            &ptz,
            B::ImageFlipHorizontalOff,
            crate::runtime::engine::AppliedStateProjection::invalidate(S::ImageFlipMode)
        );
    }

    #[test]
    fn unsupported_write_only_state_commands_fail_before_encoding() {
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");

        reset_request_write_count();
        assert!(prepare_builtin_command(
            &crate::command::PresetRecallSpeedCommand {
                speed: PresetSpeed::new(12).expect("preset speed"),
            },
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &FocusLockCommand::new(true),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &SpotlightOn::new(),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &AutoSlowShutterOn::new(),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &NdFilterModeCommand::new(NdFilterMode::Variable),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &DigitalZoom::new(true),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &MulticastStreamingCommand::new(true),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &SetNdiQuality::new(NdiQuality::High),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyBrightLo::new(),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &crate::command::SetVariableSpeedMode::new(PanTiltSpeedMode::Standard24),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyOn::new(),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyFlash::new(),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn write_only_state_requests_use_their_exact_plain_policies() {
        assert_policy::<PanTiltLimitSet>(
            17,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<PanTiltLimitClear>(
            17,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<crate::command::PresetRecallSpeedCommand>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<FocusLockCommand>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<SpotlightOn>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<SpotlightOff>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<AutoSlowShutterOn>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<AutoSlowShutterOff>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<NdFilterModeCommand>(
            7,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<AutoNdCommand>(
            7,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<ImageFreeze>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<DigitalZoom>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<MulticastStreamingCommand>(
            6,
            TimeoutClass::Network,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<SetNdiQuality>(
            6,
            TimeoutClass::Network,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyBrightLo>(
            8,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyBrightHi>(
            8,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<crate::command::SetVariableSpeedMode>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyOn>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyOff>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<TallyFlash>(
            6,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<PresetSet>(
            7,
            TimeoutClass::Preset,
            RetryClass::Preset,
            ControlClass::Normal,
        );
        assert_policy::<PresetReset>(
            7,
            TimeoutClass::Preset,
            RetryClass::Preset,
            ControlClass::Normal,
        );
    }

    fn assert_targeted<R>()
    where
        R: Request<Class = request::OperationClass<completion::Targeted>>
            + OperationCommand<completion::Targeted>,
    {
        let _ = core::marker::PhantomData::<R>;
    }

    fn assert_applied_only<R>()
    where
        R: Request<Class = request::OperationClass<completion::AppliedOnly>>
            + OperationCommand<completion::AppliedOnly>,
    {
        let _ = core::marker::PhantomData::<R>;
    }

    fn assert_policy<R: Request>(
        max_size: usize,
        timeout: TimeoutClass,
        retry: RetryClass,
        control: ControlClass,
    ) {
        assert_eq!(R::MAX_SIZE, max_size);
        assert_eq!(R::TIMEOUT_CLASS, timeout);
        assert_eq!(R::RETRY_CLASS, retry);
        assert_eq!(R::CONTROL_CLASS, control);
    }

    fn assert_exact_request_size<R: Request>(request: &R) {
        let mut buffer = [0_u8; 32];
        let written = request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("request must encode");
        assert!(written <= R::MAX_SIZE);
    }

    fn assert_exact_declared_wire_size<R: Request>(request: &R, expected: usize) {
        assert_eq!(R::MAX_SIZE, expected);
        let mut buffer = vec![0_u8; R::MAX_SIZE];
        let written = request
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .expect("request must fit its declared maximum size");
        assert_eq!(written, expected);
    }

    #[test]
    fn tally_requests_declare_their_terminated_wire_lengths() {
        assert_exact_declared_wire_size(&TallyRedOn::new(), 8);
        assert_exact_declared_wire_size(&crate::command::TallyRedOff::new(), 8);
        assert_exact_declared_wire_size(&crate::command::TallyGreenOn::new(), 8);
        assert_exact_declared_wire_size(&crate::command::TallyGreenOff::new(), 8);
    }

    /// #814: `ColorTemperature` declared a maximum one byte longer than its
    /// longest frame, `SetTemperature`.
    #[test]
    fn color_temperature_declares_its_longest_frame_as_max_size() {
        assert_exact_declared_wire_size(
            &crate::command::ColorTemperature::SetTemperature(
                crate::types::Kelvin::new(5700).expect("valid color temperature"),
            ),
            7,
        );
    }

    #[test]
    fn color_temperature_preparation_uses_the_selected_profiles_step_table() {
        use crate::command::ColorTemperature;
        use crate::types::Kelvin;

        let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "Custom temperature table".into();
        capabilities
            .white_balance
            .as_mut()
            .expect("white-balance domain")
            .color_temperature_steps = vec![2500, 3000, 5700, 8000].into();
        let profile = runtime_profile_builder(&source, capabilities)
            .build()
            .expect("runtime temperature profile");
        let target = crate::TargetRegistration::new(CameraId::CAMERA_1, profile)
            .expect("temperature target");
        let direct = ColorTemperature::SetTemperature(Kelvin::new(5700).expect("physical Kelvin"));
        direct
            .validate_for_target(&target)
            .expect("documented temperature step");
        let mut buffer = [0; ColorTemperature::MAX_SIZE];
        let written = crate::requests::write_for_target(&direct, &target, &mut buffer)
            .expect("profile-selected encoding");
        assert_eq!(&buffer[..written], &[0x81, 0x01, 0x04, 0x20, 0, 2, 0xFF]);

        reset_request_write_count();
        let absent = ColorTemperature::SetTemperature(Kelvin::new(5800).expect("physical Kelvin"));
        assert!(prepare_builtin_command(&absent, &target, OperationalTuning::new()).is_err());
        assert_eq!(
            request_write_count(),
            0,
            "an absent step must fail before encoding"
        );
    }

    #[test]
    fn plain_command_values_report_exact_wire_sizes() {
        assert_exact_request_size(&crate::command::ExposureCompensation::On);
        assert_exact_request_size(&crate::command::ExposureCompensation::SetLevel(
            crate::types::ExposureCompensationLevel::new(0).expect("valid compensation"),
        ));
        assert_exact_request_size(&crate::command::Shutter::Reset);
        assert_exact_request_size(&crate::command::Shutter::SetSpeed(
            crate::types::ShutterSpeed::new(1).expect("shutter code"),
        ));
        assert_exact_request_size(&crate::command::Brightness::Reset);
        assert_exact_request_size(&crate::command::Brightness::SetLevel(
            crate::types::BrightnessLevel::new(1).expect("brightness code"),
        ));
        assert_exact_request_size(&crate::command::Gain::Reset);
        assert_exact_request_size(&crate::command::Gain::SetValue(
            crate::types::GainLevel::new(1).expect("valid gain"),
        ));
        assert_exact_request_size(&crate::command::ColorTemperature::Reset);
        assert_exact_request_size(&crate::command::ColorTemperature::SetTemperature(
            crate::types::Kelvin::new(5700).expect("valid color temperature"),
        ));
        assert_exact_request_size(&crate::command::RedGain::Reset);
        assert_exact_request_size(&crate::command::RedGain::SetValue(
            crate::types::RgbGain::new(1).expect("valid red gain"),
        ));
        assert_exact_request_size(&crate::command::BlueGain::Reset);
        assert_exact_request_size(&crate::command::BlueGain::SetValue(
            crate::types::RgbGain::new(1).expect("valid blue gain"),
        ));
        assert_exact_request_size(&crate::command::Sharpness::Reset);
        assert_exact_request_size(&crate::command::Sharpness::SetLevel {
            value: crate::types::SharpnessLevel::new(1).expect("valid sharpness"),
        });
    }

    #[test]
    fn focus_mode_commands_do_not_project_write_only_state() {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        for command in [
            FocusModeCommand::Auto,
            FocusModeCommand::Manual,
            FocusModeCommand::Toggle,
        ] {
            let request = prepare_builtin_command(
                &command,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    profile.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .expect("focus mode preparation")
            .admit_with(|request, _timeout| request);
            assert!(matches!(
                request,
                crate::runtime::engine::RuntimeRequest::Command {
                    applied_state: None,
                    ..
                }
            ));
        }
    }

    #[test]
    fn source_admission_uses_the_actual_variant_of_a_homogeneous_command() {
        use crate::command::image_wire::{Sharpness, SharpnessMode};
        let fr7 = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7"),
        )
        .expect("target");
        let direct = Sharpness::SetLevel {
            value: crate::types::SharpnessLevel::new(7).expect("detail"),
        };
        assert_eq!(direct.ledger_row(), BuiltinCommand::SharpnessDirect);
        assert!(direct.validate_for_target(&fr7).is_ok());
        let mut buffer = [0; 16];
        let written = crate::requests::write_for_target(&direct, &fr7, &mut buffer)
            .expect("source-backed FR7 detail");
        assert_eq!(
            &buffer[..written],
            &[0x81, 0x01, 0x04, 0x42, 0, 0, 0, 7, 0xff]
        );
        let mode = Sharpness::Mode(SharpnessMode::Manual);
        assert_eq!(mode.ledger_row(), BuiltinCommand::SharpnessMode);
        assert!(matches!(
            mode.validate_for_target(&fr7),
            Err(Error::FeatureNotSupported { .. })
        ));
        let ptz = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
        )
        .expect("target");
        for (command, row) in [
            (
                crate::command::ExposureCompensation::On,
                BuiltinCommand::ExposureCompensationOn,
            ),
            (
                crate::command::ExposureCompensation::Off,
                BuiltinCommand::ExposureCompensationOff,
            ),
            (
                crate::command::ExposureCompensation::Reset,
                BuiltinCommand::ExposureCompensationReset,
            ),
            (
                crate::command::ExposureCompensation::Up,
                BuiltinCommand::ExposureCompensationUp,
            ),
            (
                crate::command::ExposureCompensation::Down,
                BuiltinCommand::ExposureCompensationDown,
            ),
        ] {
            assert_eq!(command.ledger_row(), row);
            command
                .validate_for_target(&ptz)
                .expect("source-backed compensation variant");
            crate::requests::write_for_target(&command, &ptz, &mut buffer)
                .expect("source-backed variant encoding");
        }
    }

    #[test]
    fn typed_request_inventory_covers_exactly_every_ledger_row() {
        let ledger_rows: HashSet<_> = BuiltinCommand::ALL.iter().copied().collect();
        let inventory_rows: HashSet<_> = BUILTIN_TYPED_REQUEST_INVENTORY
            .iter()
            .map(|entry| {
                assert!(
                    !entry.branch.is_empty(),
                    "{} has no command branch",
                    entry.type_name
                );
                if !entry.row.is_plain() {
                    assert!(
                        entry.row.completion().is_some(),
                        "operation row has no completion class: {:?}",
                        entry.row
                    );
                }
                entry.row
            })
            .collect();
        assert_eq!(inventory_rows, ledger_rows);
        // Several canonical methods may share a packet (for example explicit
        // and normalized zoom positioning). The surface index checks their
        // complete gates and lifecycle contracts at compile time.
        assert!(BUILTIN_TYPED_REQUEST_INVENTORY.len() >= ledger_rows.len());
    }

    #[test]
    fn physical_requests_have_closed_classes_and_exact_axes() {
        assert_targeted::<IrisReset>();
        assert_targeted::<IrisUp>();
        assert_targeted::<IrisDown>();
        assert_targeted::<IrisDirect>();
        assert_targeted::<NdFilterDirect>();
        assert_targeted::<NdFilterStepUp>();
        assert_targeted::<NdFilterStepDown>();
        assert_applied_only::<PushAfPress>();
        assert_applied_only::<PushAfRelease>();

        assert_eq!(IrisReset.affected_axes(), AffectedAxes::IRIS);
        assert_eq!(IrisUp.affected_axes(), AffectedAxes::IRIS);
        assert_eq!(IrisDown.affected_axes(), AffectedAxes::IRIS);
        assert_eq!(
            IrisDirect::new(IrisLevel::new(4).unwrap()).affected_axes(),
            AffectedAxes::IRIS
        );
        assert_eq!(
            NdFilterDirect::new(NdFilterDensity::new(4).unwrap()).affected_axes(),
            AffectedAxes::ND_FILTER
        );
        assert_eq!(NdFilterStepUp.affected_axes(), AffectedAxes::ND_FILTER);
        assert_eq!(NdFilterStepDown.affected_axes(), AffectedAxes::ND_FILTER);
        assert_eq!(PushAfPress.affected_axes(), AffectedAxes::FOCUS);
        assert_eq!(PushAfRelease.affected_axes(), AffectedAxes::FOCUS);

        assert_policy::<IrisReset>(
            6,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<IrisUp>(
            6,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<IrisDown>(
            6,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<IrisDirect>(
            9,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<NdFilterDirect>(
            10,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<NdFilterStepUp>(
            7,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<NdFilterStepDown>(
            7,
            TimeoutClass::Movement,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<PushAfPress>(
            7,
            TimeoutClass::Quick,
            RetryClass::Movement,
            ControlClass::User,
        );
        assert_policy::<PushAfRelease>(
            7,
            TimeoutClass::Quick,
            RetryClass::Movement,
            ControlClass::User,
        );
    }

    #[test]
    fn physical_requests_match_command_wire_encoders() {
        assert_eq!(wire(&IrisReset), command_wire(&Iris::Reset));
        assert_eq!(wire(&IrisUp), command_wire(&Iris::Up));
        assert_eq!(wire(&IrisDown), command_wire(&Iris::Down));

        let iris = IrisLevel::new(4).unwrap();
        assert_eq!(
            wire(&IrisDirect::new(iris)),
            command_wire(&Iris::SetAperture(iris))
        );

        let nd = NdFilterDensity::new(4).unwrap();
        assert_eq!(wire(&NdFilterDirect::new(nd)), command_wire(&nd));
        assert_eq!(
            wire(&NdFilterStepUp),
            command_wire(&NdFilterStepCommand::new(NdFilterStep::Up))
        );
        assert_eq!(
            wire(&NdFilterStepDown),
            command_wire(&NdFilterStepCommand::new(NdFilterStep::Down))
        );
        assert_eq!(wire(&PushAfPress), command_wire(&PushAF::Press));
        assert_eq!(wire(&PushAfRelease), command_wire(&PushAF::Release));
    }

    /// A normalized combined-domain target and an explicit target can encode
    /// to the same raw position, but they do not have the same permission
    /// contract. The explicit target is governed by its numeric range;
    /// normalized combined-domain input also passes runtime typed
    /// digital-range validation against a documented digital maximum.
    #[test]
    fn normalized_combined_zoom_retains_its_permission_beyond_raw_overlap() {
        let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "Documented Digital Range Without Permission".into();
        // These are explicit caller-owned runtime facts, not a digital-range
        // claim about any built-in model. Permission remains independently absent.
        capabilities.zoom.as_mut().expect("zoom domain").digital_max = Some(0x7000);
        capabilities.typed_support =
            crate::capabilities::TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
        let profile = runtime_profile_builder(&source, capabilities)
            .position_inquiries(AffectedAxes::MOVEMENT)
            .build()
            .expect("runtime partial profile");

        let midpoint = crate::units::UnitInterval::new(0.5).expect("unit interval midpoint");
        let normalized =
            ZoomTarget::from_normalized(midpoint, crate::ZoomDomain::OpticalPlusDigital, &profile)
                .expect("documented digital maximum maps the midpoint");
        // This custom combined midpoint is 0x3800, still inside its 0x4000
        // optical range. Keep that exact raw value as the direct control.
        let explicit = ZoomTarget::new(ZoomPosition::new(0x3800).expect("optical position"));
        assert_eq!(wire(&normalized), wire(&explicit));
        assert_ne!(
            normalized, explicit,
            "normalization provenance changes profile admissibility"
        );

        prepare_builtin_operation::<completion::Targeted, _>(
            &explicit,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                profile.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("explicit optical position remains governed by its numeric range");

        let error = prepare_builtin_operation::<completion::Targeted, _>(
            &normalized,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                profile.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect_err("combined normalization requires typed digital-range permission");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "optical-plus-digital zoom positioning"
            }
        ));
    }

    #[test]
    fn profile_validation_rejects_unsupported_physical_requests_before_encoding() {
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        let nd = NdFilterDirect::new(NdFilterDensity::new(4).unwrap());

        reset_request_write_count();
        for result in [
            prepare_builtin_operation::<completion::Targeted, _>(
                &IrisReset,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    fr7.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &IrisUp,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    fr7.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &IrisDown,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    fr7.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        let iris_direct = IrisDirect::new(IrisLevel::new(4).unwrap());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &iris_direct,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        for result in [
            prepare_builtin_operation::<completion::Targeted, _>(
                &nd,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    generic.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &NdFilterStepUp,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    generic.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &NdFilterStepDown,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    generic.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        for result in [
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                &PushAfPress,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    generic.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                &PushAfRelease,
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    generic.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn noise_reduction_family_grants_admit_or_refuse_both_readback_and_control() {
        use crate::command::image::{
            NoiseReduction2dInquiry, NoiseReduction3dInquiry, SetNoiseReduction2d,
            SetNoiseReduction3d,
        };
        let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let supported = crate::TargetRegistration::new(CameraId::CAMERA_1, source.clone())
            .expect("supported target");
        for validation in [
            NoiseReduction2dInquiry.validate_for_target(&supported),
            NoiseReduction3dInquiry.validate_for_target(&supported),
        ] {
            validation.expect("a family grant admits readback");
        }
        let two_d = SetNoiseReduction2d::new(crate::types::NoiseReduction2D::Off);
        let three_d = SetNoiseReduction3d::new(crate::types::NoiseReduction3D::Off);
        for (bytes, expected) in [
            (
                prepare_builtin_command(&two_d, &supported, OperationalTuning::new())
                    .expect("2D control grant")
                    .admit_with(|request, _| request.wire().as_bytes().to_vec()),
                [0x81, 1, 4, 0x53, 0, 0xff],
            ),
            (
                prepare_builtin_command(&three_d, &supported, OperationalTuning::new())
                    .expect("3D control grant")
                    .admit_with(|request, _| request.wire().as_bytes().to_vec()),
                [0x81, 1, 4, 0x54, 0, 0xff],
            ),
        ] {
            assert_eq!(bytes, expected);
        }
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "NR without family grants".into();
        capabilities.typed_support = capabilities
            .typed_support
            .without(TypedSupportSurface::NoiseReduction2D)
            .without(TypedSupportSurface::NoiseReduction2DMode)
            .without(TypedSupportSurface::NoiseReduction3D);
        let profile = runtime_profile_builder(&source, capabilities)
            .build()
            .expect("withheld family grants");
        let registration =
            crate::TargetRegistration::new(CameraId::CAMERA_1, profile).expect("target");
        for validation in [
            NoiseReduction2dInquiry.validate_for_target(&registration),
            NoiseReduction3dInquiry.validate_for_target(&registration),
        ] {
            assert!(matches!(validation, Err(Error::FeatureNotSupported { .. })));
        }
        reset_request_write_count();
        for result in [
            prepare_builtin_command(&two_d, &registration, OperationalTuning::new()),
            prepare_builtin_command(&three_d, &registration, OperationalTuning::new()),
        ] {
            assert!(matches!(result, Err(Error::FeatureNotSupported { .. })));
        }
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn profile_validation_limits_typed_iris_to_supported_profiles() {
        let ptz = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZ profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBrcH900>().expect("H900 profile");
        let iris = IrisDirect::new(IrisLevel::new(4).unwrap());

        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                ptz.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &iris,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                ptz.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &IrisDirect::new(IrisLevel::new(0x19).unwrap()),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                h900.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_ok());

        reset_request_write_count();
        // BRC-H900's sourced maximum is 0x19, not the global type's 0x1E.
        let beyond_h900 = prepare_builtin_operation::<completion::Targeted, _>(
            &IrisDirect::new(IrisLevel::new(0x1A).unwrap()),
            &crate::TargetRegistration::new(CameraId::CAMERA_1, h900.clone()).expect("H900 target"),
            OperationalTuning::new(),
        );
        assert!(matches!(
            beyond_h900,
            Err(Error::ParameterOutOfRange {
                parameter: "iris level",
                min: 0,
                max: 25,
                ..
            })
        ));
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &iris,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &NdFilterDirect::new(NdFilterDensity::new(4).unwrap()),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &NdFilterStepUp,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::AppliedOnly, _>(
            &PushAfPress,
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_ok());
    }

    #[test]
    fn gain_validation_uses_each_profiles_sourced_domain() {
        let h900 = ProfileSpec::from_compile_time::<SonyBrcH900>().expect("H900 profile");
        let evi = ProfileSpec::from_compile_time::<SonyEviH100>().expect("EVI profile");
        for (profile, value) in [(&h900, 0x19), (&evi, 0x0F)] {
            let gain = crate::command::Gain::SetValue(
                crate::types::GainLevel::new(value).expect("sourced gain code"),
            );
            prepare_builtin_command(
                &gain,
                &crate::TargetRegistration::new(CameraId::CAMERA_1, profile.clone())
                    .expect("target"),
                OperationalTuning::new(),
            )
            .expect("documented upper gain boundary prepares");
        }

        let gain = crate::command::Gain::SetValue(
            crate::types::GainLevel::new(0x0C).expect("representable gain"),
        );
        let fr7 = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile"),
        )
        .expect("FR7 target");
        let ptz = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"),
        )
        .expect("G2 target");
        reset_request_write_count();
        let absent = prepare_builtin_command(&gain, &fr7, OperationalTuning::new())
            .expect_err("FR7 has no shared gain family");
        assert!(matches!(absent, Error::FeatureNotSupported { .. }));
        let out_of_range = prepare_builtin_command(&gain, &ptz, OperationalTuning::new())
            .expect_err("G2's shared gain maximum is 7");
        assert!(matches!(
            out_of_range,
            Error::ParameterOutOfRange {
                value: 12,
                min: 0,
                max: 7,
                ..
            }
        ));
        assert_eq!(
            request_write_count(),
            0,
            "unsupported gain requests never encode"
        );
    }

    #[test]
    fn gain_limit_prepares_every_sourced_nibble_independently_of_gain_levels() {
        use crate::{command::gain::GainLimitCommand, Inquiry};

        let decoded = crate::command::GainLimitInquiry
            .decoder()
            .decode(&[0x0F])
            .expect("gain-limit reply 90 50 0F FF");
        assert_eq!(decoded.value(), 0x0F);

        for profile in [
            ProfileSpec::from_compile_time::<PtzOpticsG2>(),
            ProfileSpec::from_compile_time::<PtzOpticsG3>(),
            ProfileSpec::from_compile_time::<PtzOptics30x>(),
        ] {
            let profile = profile.expect("sourced PTZOptics profile");
            let target = crate::TargetRegistration::new(CameraId::CAMERA_1, profile.clone())
                .expect("target");
            for value in 0x00..=0x0F {
                let limit = crate::types::GainLimit::new(value).expect("full nibble");
                let frame = prepare_builtin_command(
                    &GainLimitCommand::new(limit),
                    &target,
                    OperationalTuning::new(),
                )
                .expect("every sourced gain-limit value prepares")
                .admit_with(|request, _| request.wire().as_bytes().to_vec());
                assert_eq!(frame, [0x81, 1, 4, 0x2C, value, 0xFF]);
            }
            prepare_builtin_command(
                &GainLimitCommand::new(decoded),
                &target,
                OperationalTuning::new(),
            )
            .expect("a decoded limit can be written back");

            let mut capabilities = profile.capabilities().clone();
            capabilities.profile_id = None;
            capabilities.exposure.as_mut().expect("exposure").gain_range = None;
            let without_gain_levels = runtime_profile_builder(&profile, capabilities)
                .build()
                .expect("gain-limit support needs no gain-level range");
            let target = crate::TargetRegistration::new(CameraId::CAMERA_1, without_gain_levels)
                .expect("target without gain levels");
            prepare_builtin_command(
                &GainLimitCommand::new(decoded),
                &target,
                OperationalTuning::new(),
            )
            .expect("gain-limit admission does not read gain_range");
        }
    }

    #[test]
    fn unsourced_gain_limits_are_refused_before_encoding() {
        let command = crate::command::gain::GainLimitCommand::new(
            crate::types::GainLimit::new(0x0F).expect("full nibble"),
        );
        for profile in [
            ProfileSpec::from_compile_time::<SonyFr7>(),
            ProfileSpec::from_compile_time::<SonyBrcH900>(),
            ProfileSpec::from_compile_time::<SonyEviH100>(),
            ProfileSpec::from_compile_time::<SonyBrc300>(),
            ProfileSpec::from_compile_time::<NearusBrc300>(),
            ProfileSpec::from_compile_time::<GenericVisca>(),
        ] {
            let target = crate::TargetRegistration::new(
                CameraId::CAMERA_1,
                profile.expect("unsourced built-in profile"),
            )
            .expect("target");
            reset_request_write_count();
            assert!(matches!(
                prepare_builtin_command(&command, &target, OperationalTuning::new()),
                Err(Error::FeatureNotSupported { .. })
            ));
            assert_eq!(request_write_count(), 0, "source denial precedes encoding");
        }
    }

    #[test]
    fn fr7_rejects_every_shared_ae_mode_before_encoding() {
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");

        reset_request_write_count();
        for mode in [
            ExposureMode::Auto,
            ExposureMode::Manual,
            ExposureMode::Shutter,
            ExposureMode::Iris,
            ExposureMode::Bright,
        ] {
            let error = prepare_builtin_command(
                &ExposureCommand::new(mode),
                &crate::TargetRegistration::new(
                    (crate::target::TargetSlot::CAMERA_1).camera(),
                    fr7.clone(),
                )
                .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .expect_err("FR7 does not document the shared AE-mode command family");
            assert!(matches!(
                error,
                Error::FeatureNotSupported {
                    feature: "shared exposure-mode family"
                }
            ));
        }
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn shared_ae_modes_require_typed_support_beyond_nonempty_inventory() {
        let source = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "Exposure inventory without typed permission".into();
        capabilities.typed_support = capabilities
            .typed_support
            .without(TypedSupportSurface::ExposureMode);
        let generic = runtime_profile_builder(&source, capabilities)
            .build()
            .expect("runtime profile without exposure-mode permission");
        assert!(!generic
            .capabilities()
            .exposure
            .as_ref()
            .expect("exposure")
            .modes
            .is_empty());
        assert!(!generic
            .capabilities()
            .supports(TypedSupportSurface::ExposureMode));

        reset_request_write_count();
        let error = prepare_builtin_command(
            &ExposureCommand::new(ExposureMode::Auto),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                generic.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect_err("discovery metadata alone must not admit shared AE-mode control");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "shared exposure-mode family"
            }
        ));
        assert_eq!(request_write_count(), 0);
    }

    /// Sony FR7 red/green tally and the unevidenced PTZOptics/brightness
    /// candidates occupy independent typed-support surfaces.
    #[test]
    fn tally_families_do_not_inherit_each_others_support() {
        let ptz = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZ profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7 profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBrcH900>().expect("H900 profile");

        macro_rules! admits {
            ($profile:expr, $command:expr) => {
                prepare_builtin_command(
                    &$command,
                    &crate::TargetRegistration::new(
                        (crate::target::TargetSlot::CAMERA_1).camera(),
                        ($profile).clone(),
                    )
                    .expect("valid target registration"),
                    OperationalTuning::new(),
                )
                .is_ok()
            };
        }

        assert!(admits!(&fr7, TallyRedOn::new()));
        assert!(admits!(&fr7, crate::command::TallyGreenOn::new()));
        assert!(!admits!(&fr7, TallyOn::new()));
        assert!(!admits!(&fr7, TallyOff::new()));
        assert!(!admits!(&fr7, TallyFlash::new()));
        assert!(!admits!(&fr7, TallyBrightLo::new()));
        assert!(!admits!(&fr7, TallyBrightHi::new()));

        // Neither the PTZOptics candidate documentation nor generic metadata
        // opts a built-in profile into either typed family.
        for profile in [&ptz, &generic, &h900] {
            assert!(!admits!(profile, TallyOn::new()));
            assert!(!admits!(profile, TallyOff::new()));
            assert!(!admits!(profile, TallyFlash::new()));
            assert!(!admits!(profile, TallyRedOn::new()));
        }
        assert!(!admits!(&h900, crate::command::TallyRedOff::new()));
        assert!(!admits!(&h900, crate::command::TallyGreenOn::new()));
        assert!(!admits!(&h900, crate::command::TallyGreenOff::new()));
        assert!(!admits!(&ptz, TallyBrightHi::new()));
    }
}

// Source-backed Sony FR7 request contracts (R7 v4).
builtin_request! {
    crate::command::Fr7AeLevel => Fr7AeLevelUp: Plain { size: 6,
        rows: |value| match (value) {
            crate::command::Fr7AeLevel::Up => Fr7AeLevelUp,
            crate::command::Fr7AeLevel::Down => Fr7AeLevelDown,
        },
    };
    crate::command::Fr7AutoGain => Fr7AutoGain: Plain { size: 7,
    };
    crate::command::Fr7AutoShutter => Fr7AutoShutter: Plain { size: 6,
    };
    crate::command::Fr7AutoIris => Fr7AutoIris: Plain { size: 6,
    };
    crate::command::Fr7IrisUp => Fr7IrisUp: AppliedOnly { size: 9,
    };
    crate::command::Fr7IrisDown => Fr7IrisDown: AppliedOnly { size: 9,
    };
    crate::command::Fr7WhiteBalanceSet => Fr7WhiteBalanceSet: Plain { size: 6,
    };
    crate::command::Fr7PresetWhite => Fr7PresetWhiteUp: Plain { size: 10,
        rows: |value| match (value) {
            crate::command::Fr7PresetWhite::Up(_) => Fr7PresetWhiteUp,
            crate::command::Fr7PresetWhite::Down(_) => Fr7PresetWhiteDown,
            crate::command::Fr7PresetWhite::Direct(_) => Fr7PresetWhiteDirect,
        },
    };
    crate::command::Fr7MemoryColorTemperature => Fr7MemoryColorTemperatureUp: Plain { size: 10,
        rows: |value| match (value) {
            crate::command::Fr7MemoryColorTemperature::Up(_) => Fr7MemoryColorTemperatureUp,
            crate::command::Fr7MemoryColorTemperature::Down(_) => Fr7MemoryColorTemperatureDown,
            crate::command::Fr7MemoryColorTemperature::Direct(_) => Fr7MemoryColorTemperatureDirect,
        },
    };
    crate::command::Fr7MemoryRedGain => Fr7MemoryRedGainUp: Plain { size: 11,
        rows: |value| match (value) {
            crate::command::Fr7MemoryRedGain::Up(_) => Fr7MemoryRedGainUp,
            crate::command::Fr7MemoryRedGain::Down(_) => Fr7MemoryRedGainDown,
            crate::command::Fr7MemoryRedGain::Direct(_) => Fr7MemoryRedGainDirect,
        },
    };
    crate::command::Fr7MemoryBlueGain => Fr7MemoryBlueGainUp: Plain { size: 11,
        rows: |value| match (value) {
            crate::command::Fr7MemoryBlueGain::Up(_) => Fr7MemoryBlueGainUp,
            crate::command::Fr7MemoryBlueGain::Down(_) => Fr7MemoryBlueGainDown,
            crate::command::Fr7MemoryBlueGain::Direct(_) => Fr7MemoryBlueGainDirect,
        },
    };
    crate::command::Fr7MemoryTint => Fr7MemoryTintUp: Plain { size: 10,
        rows: |value| match (value) {
            crate::command::Fr7MemoryTint::Up(_) => Fr7MemoryTintUp,
            crate::command::Fr7MemoryTint::Down(_) => Fr7MemoryTintDown,
            crate::command::Fr7MemoryTint::Direct(_) => Fr7MemoryTintDirect,
        },
    };
    crate::command::Fr7AtwColorTemperatureOffset => Fr7AtwColorTemperatureOffsetUp: Plain { size: 10,
        rows: |value| match (value) {
            crate::command::Fr7AtwColorTemperatureOffset::Up(_) => Fr7AtwColorTemperatureOffsetUp,
            crate::command::Fr7AtwColorTemperatureOffset::Down(_) => Fr7AtwColorTemperatureOffsetDown,
            crate::command::Fr7AtwColorTemperatureOffset::Direct(_) => Fr7AtwColorTemperatureOffsetDirect,
        },
    };
    crate::command::Fr7AtwTintOffset => Fr7AtwTintOffsetUp: Plain { size: 10,
        rows: |value| match (value) {
            crate::command::Fr7AtwTintOffset::Up(_) => Fr7AtwTintOffsetUp,
            crate::command::Fr7AtwTintOffset::Down(_) => Fr7AtwTintOffsetDown,
            crate::command::Fr7AtwTintOffset::Direct(_) => Fr7AtwTintOffsetDirect,
        },
    };
    crate::command::Fr7MasterBlack => Fr7MasterBlackUp: Plain { size: 9,
        rows: |value| match (value) {
            crate::command::Fr7MasterBlack::Up(_) => Fr7MasterBlackUp,
            crate::command::Fr7MasterBlack::Down(_) => Fr7MasterBlackDown,
            crate::command::Fr7MasterBlack::Direct(_) => Fr7MasterBlackDirect,
        },
    };
    crate::command::Fr7RedBlack => Fr7RedBlackUp: Plain { size: 10,
        rows: |value| match (value) {
            crate::command::Fr7RedBlack::Up(_) => Fr7RedBlackUp,
            crate::command::Fr7RedBlack::Down(_) => Fr7RedBlackDown,
            crate::command::Fr7RedBlack::Direct(_) => Fr7RedBlackDirect,
        },
    };
    crate::command::Fr7BlueBlack => Fr7BlueBlackUp: Plain { size: 10,
        rows: |value| match (value) {
            crate::command::Fr7BlueBlack::Up(_) => Fr7BlueBlackUp,
            crate::command::Fr7BlueBlack::Down(_) => Fr7BlueBlackDown,
            crate::command::Fr7BlueBlack::Direct(_) => Fr7BlueBlackDirect,
        },
    };
    crate::command::Fr7DetailSetting => Fr7DetailSetting: Plain { size: 7,
    };
    crate::command::Fr7KneeSetting => Fr7KneeSetting: Plain { size: 7,
    };
    crate::command::Fr7SetKneeMode => Fr7SetKneeMode: Plain { size: 7,
    };
    crate::command::Fr7SetKneeSlope => Fr7SetKneeSlope: Plain { size: 8,
    };
    crate::command::Fr7SetKneePoint => Fr7SetKneePoint: Plain { size: 8,
    };
    crate::command::Fr7ZoomHighResolution => Fr7ZoomHighResolutionTele: AppliedOnly { size: 11,
        rows: |value| match (value) {
            crate::command::Fr7ZoomHighResolution::Tele(_) => Fr7ZoomHighResolutionTele,
            crate::command::Fr7ZoomHighResolution::Wide(_) => Fr7ZoomHighResolutionWide,
        },
    };
    crate::command::Fr7ZoomHighResolutionStop => Fr7ZoomHighResolutionStop: AppliedOnly { size: 11,
        policy: (Quick, Movement, Urgent),
    };
    crate::command::Fr7SetPresetSpeedSelection => Fr7SetPresetSpeedSelection: Plain { size: 7,
    };
    crate::command::Fr7SetCommonPresetSpeed => Fr7SetCommonPresetSpeed: Plain { size: 8,
    };
    crate::command::Fr7SetSeparatePresetMode => Fr7SetSeparatePresetMode: Plain { size: 8,
    };
    crate::command::Fr7SetSeparatePresetSpeed => Fr7SetSeparatePresetSpeed: Plain { size: 8,
    };
    crate::command::Fr7SetSeparatePresetDuration => Fr7SetSeparatePresetDuration: Plain { size: 10,
    };
}

impl BuiltinValidation for crate::command::Fr7AeLevel {}
impl BuiltinValidation for crate::command::Fr7AutoGain {}
impl BuiltinValidation for crate::command::Fr7AutoShutter {}
impl BuiltinValidation for crate::command::Fr7AutoIris {}
impl BuiltinValidation for crate::command::Fr7IrisUp {}
impl BuiltinValidation for crate::command::Fr7IrisDown {}
impl BuiltinValidation for crate::command::Fr7WhiteBalanceSet {}
impl BuiltinValidation for crate::command::Fr7PresetWhite {}
impl BuiltinValidation for crate::command::Fr7MemoryColorTemperature {}
impl BuiltinValidation for crate::command::Fr7MemoryRedGain {}
impl BuiltinValidation for crate::command::Fr7MemoryBlueGain {}
impl BuiltinValidation for crate::command::Fr7MemoryTint {}
impl BuiltinValidation for crate::command::Fr7AtwColorTemperatureOffset {}
impl BuiltinValidation for crate::command::Fr7AtwTintOffset {}
impl BuiltinValidation for crate::command::Fr7MasterBlack {}
impl BuiltinValidation for crate::command::Fr7RedBlack {}
impl BuiltinValidation for crate::command::Fr7BlueBlack {}
impl BuiltinValidation for crate::command::Fr7DetailSetting {}
impl BuiltinValidation for crate::command::Fr7KneeSetting {}
impl BuiltinValidation for crate::command::Fr7SetKneeMode {}
impl BuiltinValidation for crate::command::Fr7SetKneeSlope {}
impl BuiltinValidation for crate::command::Fr7SetKneePoint {}
impl BuiltinValidation for crate::command::Fr7ZoomHighResolution {}
impl BuiltinValidation for crate::command::Fr7ZoomHighResolutionStop {}
impl BuiltinValidation for crate::command::Fr7SetPresetSpeedSelection {}
impl BuiltinValidation for crate::command::Fr7SetCommonPresetSpeed {}
impl BuiltinValidation for crate::command::Fr7SetSeparatePresetMode {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.preset().value())
    }
}
impl BuiltinValidation for crate::command::Fr7SetSeparatePresetSpeed {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.preset().value())
    }
}
impl BuiltinValidation for crate::command::Fr7SetSeparatePresetDuration {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.preset().value())
    }
}
