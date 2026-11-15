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
        Flip, Focus, Iris, NdFilterStep, NdFilterStepCommand, NdFilterValue, PanTilt,
        PanTiltDirection, PanTiltLimitCorner, PresetAction, PresetCommand, PresetNumber, PushAF,
        Zoom,
    },
    completion,
    noun_table::noun_table,
    request, AffectedAxes, CameraId, ControlClass, Error, OperationCommand, Request, RetryClass,
    TimeoutClass,
};
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
#[cfg(any(feature = "blocking", feature = "async"))]
use crate::types::SpeedLevel;
use crate::types::{FocusPosition, IrisLevel, PanSpeed, TiltSpeed, ZoomPosition, ZoomSpeed};
use crate::{
    capabilities::PanTiltWireCodec,
    command::pan_tilt::{MoveSpeeds, PanTiltFraming, PanTiltProfiled},
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
    (@class Plain) => { request::Plain };
    (@class Targeted) => { request::Operation<completion::Targeted> };
    (@class AppliedOnly) => { request::Operation<completion::AppliedOnly> };

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

    (@wire $this:tt) => { *$this };
    (@wire $this:tt, $wire:expr) => { ($wire)($this) };

    (@contract $type:ty, $row:ident [$(profile_axes)?] [$($by_value:ident)?]) => {
        impl BuiltinRequestContract for $type {
            const LEDGER_ROW: BuiltinCommand = BuiltinCommand::$row;
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
                let written = <Self as Request>::write_into(self, target.target(), buffer)?;
                let row = builtin_request!(@row self $(, $value)?);
                row.source().validate_frame(target.profile(), &buffer[..written], surface_entry(row).feature())?;
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
                <Self as BuiltinValidation>::applied_state(self)
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

pub(crate) trait BuiltinValidation {
    /// Checks only value-dependent facts after the generated row gate.
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }

    fn validate_target(&self, target: &crate::TargetRegistration) -> Result<(), Error> {
        self.validate(target.profile())
    }

    fn pan_tilt_max_speed(&self) -> Option<u8> {
        None
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        None
    }
}

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
        return Err(Error::InvalidRequest(
            "position speeds belong to another wire grammar".into(),
        ));
    }
    Ok(())
}

/// Lowers one coarse position speed through the profile-owned position grammar.
///
/// Standard VISCA keeps [`SpeedLevel`]'s asymmetric pan/tilt mapping. The
/// profile still owns the usable maximum for each axis, so a coarse level such
/// as [`SpeedLevel::Fastest`] means that profile's fastest valid speed rather
/// than an unconditional `24`/`20` pair. Sony BRC-300 position frames carry
/// one `VV` byte, so both public speed wrappers deliberately receive one value
/// from the intersection of the profile's pan and tilt ranges before the
/// paired-speed validator runs.
#[cfg(any(feature = "blocking", feature = "async"))]
fn pan_tilt_position_speeds_from_level(
    speed: SpeedLevel,
    profile: &crate::ProfileSpec,
) -> Result<(PanSpeed, TiltSpeed), Error> {
    let facts = pan_tilt_facts(profile)?;
    if facts.coordinates.wire_codec() == PanTiltWireCodec::SonyBrc300 {
        let speed = speed.to_pan_speed().clamp(
            facts.pan_speed.min().max(facts.tilt_speed.min()),
            facts.pan_speed.max().min(facts.tilt_speed.max()),
        );
        Ok((PanSpeed::new(speed)?, TiltSpeed::new(speed)?))
    } else {
        Ok((
            PanSpeed::new(
                speed
                    .to_pan_speed()
                    .clamp(facts.pan_speed.min(), facts.pan_speed.max()),
            )?,
            TiltSpeed::new(
                speed
                    .to_tilt_speed()
                    .clamp(facts.tilt_speed.min(), facts.tilt_speed.max()),
            )?,
        ))
    }
}

fn validate_power_state(profile: &crate::ProfileSpec, standby: bool) -> Result<(), Error> {
    if standby {
        require(
            profile.capabilities().supports_standby(),
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
        capabilities.supports_exposure_mode(mode),
        crate::command::exposure::SHARED_EXPOSURE_MODE_FEATURE,
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
    value: Option<crate::types::ColorTemp>,
) -> Result<(), Error> {
    if let Some(value) = value {
        let range = white_balance_facts(profile)?
            .color_temp_range
            .as_ref()
            .ok_or(Error::FeatureNotSupported {
                feature: "color-temperature range",
            })?;
        require_in_range("color temperature", value.to_kelvin(), range)?;
    }
    Ok(())
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

impl BuiltinValidation for crate::command::power::PowerOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_power_state(profile, false)
    }
}

impl BuiltinValidation for crate::command::power::PowerStandby {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_power_state(profile, true)
    }
}

impl BuiltinValidation for crate::command::exposure::ExposureCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_exposure_mode(profile, self.mode)
    }
}

impl BuiltinValidation for crate::command::exposure::ExposureCompensation {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let value = match self {
            Self::SetLevel(level) => Some(level.value()),
            _ => None,
        };
        validate_exposure_compensation(profile, value)
    }
}

impl BuiltinValidation for crate::command::exposure::DynamicRange {}

impl BuiltinValidation for crate::command::exposure::Shutter {
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

impl BuiltinValidation for crate::command::exposure::Brightness {
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

impl BuiltinValidation for crate::command::exposure::AntiFlickerCommand {}

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

impl BuiltinValidation for crate::command::gain::GainLimitCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_gain(profile, Some(self.limit.value()))
    }
}

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

impl BuiltinValidation for crate::command::image::Sharpness {
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

impl BuiltinValidation for crate::command::image::Luminance {
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

impl BuiltinValidation for crate::command::image::Contrast {
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

impl BuiltinValidation for crate::command::image::GammaCommand {
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

impl BuiltinValidation for crate::command::image::BacklightCommand {}

impl BuiltinValidation for crate::command::image::NoiseReduction2DModeCommand {}

impl BuiltinValidation for crate::command::image::NoiseReduction2D {}

impl BuiltinValidation for crate::command::image::NoiseReduction3D {}

impl BuiltinValidation for crate::command::image::ImageFlipCombinedCommand {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // The combined opcode carries both axes in one parameter byte, so a
        // successful application establishes the complete pair.
        known_projection(crate::KnownState::Flip(self.mode.into()))
    }
}

impl BuiltinValidation for crate::command::image::PictureEffectCommand {}

impl BuiltinValidation for crate::command::focus::FocusZoneCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();

        if capabilities.supports_focus_zone(self.zone) {
            Ok(())
        } else {
            Err(Error::InvalidParameter {
                parameter: "focus_zone",
                value: Cow::Owned(format!("{:02X}", u8::from(self.zone))),
                reason: Cow::Borrowed(
                    "the profile has no evidence that the camera accepts this focus-zone value",
                ),
            })
        }
    }
}

impl BuiltinValidation for crate::command::focus::AutoFocusSensitivityCommand {}

impl BuiltinValidation for crate::command::focus::FocusNearLimitCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let value = self.position.value();
        require_in_range("focus near limit", value, &focus_facts(profile)?.range)
    }
}

impl BuiltinValidation for crate::command::white_balance::WhiteBalanceCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_white_balance_mode(profile, self)
    }
}

impl BuiltinValidation for crate::command::white_balance::AWBSensitivityCommand {}

impl BuiltinValidation for crate::command::menu::SetMenuDisplay {}

impl BuiltinValidation for crate::command::menu::MenuNavigate {}

impl BuiltinValidation for crate::command::menu::PerformMenuAction {}

impl BuiltinValidation for crate::command::menu::DirectMenuControl {}

impl BuiltinValidation for crate::command::streaming::UsbAudio {}

impl BuiltinValidation for crate::command::system::SettingsSaveCommand {}

impl BuiltinValidation for crate::command::tally::TallyRedOn {}

impl BuiltinValidation for crate::command::tally::TallyRedOff {}

impl BuiltinValidation for crate::command::tally::TallyGreenOn {}

impl BuiltinValidation for crate::command::tally::TallyGreenOff {}

impl BuiltinValidation for crate::command::motion_sync::SetMotionSyncMode {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        motion_sync_speed_range(profile).map(|_| ())
    }
}

impl BuiltinValidation for crate::command::motion_sync::SetMotionSyncPreset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_motion_sync_speed(profile, self.speed())
    }
}

impl BuiltinValidation for crate::command::system::AddressSetCommand {}

impl BuiltinValidation for crate::command::system::InterfaceClearCommand {}

impl BuiltinValidation for crate::command::system::CommandCancelCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(
            profile.supports_command_cancel(),
            "VISCA socket cancellation",
        )
    }
}

impl BuiltinValidation for ImageFlipCommand {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // This opcode moves only the vertical axis. The horizontal axis keeps
        // whatever value it had, which this request does not know, so the
        // complete pair stops being known.
        invalidated_projection::<Self>()
    }
}

impl BuiltinValidation for ImageMirrorCommand {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // The mirror opcode is the horizontal twin of `ImageFlipCommand` and
        // leaves the vertical axis unknown for the same reason.
        invalidated_projection::<Self>()
    }
}

#[cfg(any(feature = "blocking", feature = "async"))]
fn known_projection(
    value: crate::KnownState,
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    Some(crate::runtime::engine::AppliedStateProjection::set(value))
}

#[cfg(any(feature = "blocking", feature = "async"))]
fn invalidated_projection<T: BuiltinRequestContract>(
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    use crate::command::semantics::AppliedStateEffectRequirement;
    let state = const {
        match crate::command::semantics::state_effect::<T>() {
            AppliedStateEffectRequirement::Invalidate(state) => state,
            _ => panic!("an invalidating request must declare an invalidation"),
        }
    };
    Some(crate::runtime::engine::AppliedStateProjection::invalidate(
        state,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedPanTiltPosition {
    pan: i32,
    tilt: i32,
    conversion: PanTiltCoordinateConversion,
    mount: Option<crate::MountPosition>,
}

impl PreparedPanTiltPosition {
    fn for_target(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        let profile = target.profile();
        let conversion = profile
            .pan_tilt_coordinates()
            .ok_or(Error::FeatureNotSupported {
                feature: "pan/tilt coordinate conversion",
            })?;
        let (pan, tilt) = target.convert_pan_tilt_degrees(pan.0, tilt.0)?;
        Ok(Self {
            pan,
            tilt,
            conversion,
            mount: target.mount(),
        })
    }

    fn validate(self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        if profile.pan_tilt_coordinates() != Some(self.conversion) {
            return Err(Error::InvalidRequest(
                "pan/tilt request was converted for a different profile".into(),
            ));
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
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        target: &crate::TargetRegistration,
    ) -> Result<Self, Error> {
        let profile = target.profile();
        let conversion = profile
            .pan_tilt_coordinates()
            .ok_or(Error::FeatureNotSupported {
                feature: "pan/tilt coordinate conversion",
            })?;
        let (pan, tilt) = target.convert_pan_tilt_offset_degrees(pan.0, tilt.0)?;
        target.validate_pan_tilt_offset(pan, tilt)?;
        Ok(Self {
            pan,
            tilt,
            conversion,
        })
    }

    fn validate(self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        if profile.pan_tilt_coordinates() != Some(self.conversion) {
            return Err(Error::InvalidRequest(
                "pan/tilt offset was converted for a different profile".into(),
            ));
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

// Built-in command values that are already structurally homogeneous carry
// their request contract directly: every branch represented by the type is a
// configuration, mode, persistence, or output-policy write, so it needs no
// second public noun or branch discriminator. Mixed physical enums (PanTilt,
// Zoom, Focus, Iris, ND, and Preset) are represented by the structurally
// distinct request types below.
builtin_request! {
    crate::command::preset::PresetRecallSpeedCommand => PresetRecallSpeed: Plain { size: 6 };
    crate::command::focus::FocusLock => FocusLock: Plain { size: 6 };
    crate::command::exposure::SpotlightOn => SpotlightOn: Plain { size: 6 };
    crate::command::exposure::SpotlightOff => SpotlightOff: Plain { size: 6 };
    crate::command::exposure::AutoSlowShutterOn => AutoSlowShutterOn: Plain { size: 6 };
    crate::command::exposure::AutoSlowShutterOff => AutoSlowShutterOff: Plain { size: 6 };
    crate::command::nd_filter::NdFilterModeCommand => NdFilterMode: Plain { size: 7 };
    crate::command::nd_filter::AutoNdCommand => NdFilterAutoOn: Plain {
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
    crate::command::zoom::DigitalZoom => DigitalZoom: Plain { size: 6 };
    crate::command::streaming::MulticastStreaming => MulticastStreamingOn: Plain {
        size: 6,
        policy: (Network, Standard, Normal),
        rows: |command| match (command) {
            crate::command::streaming::MulticastStreaming::On => MulticastStreamingOn,
            crate::command::streaming::MulticastStreaming::Off => MulticastStreamingOff,
        },
    };
    crate::command::streaming::SetNdiQuality => NdiQuality: Plain {
        size: 6,
        policy: (Network, Standard, Normal),
    };
    crate::command::tally::TallyBrightLo => TallyBrightLow: Plain { size: 8 };
    crate::command::tally::TallyBrightHi => TallyBrightHigh: Plain { size: 8 };
    crate::command::variable_speed::SetVariableSpeedMode => VariableSpeedMode: Plain { size: 6 };
    crate::command::tally::TallyOn => TallyOn: Plain { size: 6 };
    crate::command::tally::TallyOff => TallyOff: Plain { size: 6 };
    crate::command::tally::TallyFlash => TallyFlash: Plain { size: 6 };
    crate::command::power::PowerOn => PowerOn: Plain { size: 6 };
    crate::command::power::PowerStandby => PowerStandby: Plain { size: 6 };
    crate::command::exposure::ExposureCommand => ExposureMode: Plain { size: 6 };
    crate::command::exposure::ExposureCompensation => ExposureCompensationOn: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::exposure::ExposureCompensation::On => ExposureCompensationOn,
            crate::command::exposure::ExposureCompensation::Off => ExposureCompensationOff,
            crate::command::exposure::ExposureCompensation::Reset => ExposureCompensationReset,
            crate::command::exposure::ExposureCompensation::Up => ExposureCompensationUp,
            crate::command::exposure::ExposureCompensation::Down => ExposureCompensationDown,
            crate::command::exposure::ExposureCompensation::SetLevel(_) => ExposureCompensationDirect,
        },
    };
    crate::command::exposure::DynamicRange => DynamicRange: Plain { size: 9 };
    crate::command::exposure::Shutter => ShutterReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::exposure::Shutter::Reset => ShutterReset,
            crate::command::exposure::Shutter::Up => ShutterUp,
            crate::command::exposure::Shutter::Down => ShutterDown,
            crate::command::exposure::Shutter::SetSpeed(_) => ShutterDirect,
        },
    };
    crate::command::exposure::Brightness => BrightnessReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::exposure::Brightness::Reset => BrightnessReset,
            crate::command::exposure::Brightness::Up => BrightnessUp,
            crate::command::exposure::Brightness::Down => BrightnessDown,
            crate::command::exposure::Brightness::SetLevel(_) => BrightnessSet,
        },
    };
    crate::command::exposure::AntiFlickerCommand => AntiFlicker: Plain { size: 6 };
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
    crate::command::image::Sharpness => SharpnessReset: Plain {
        size: 9,
        rows: |command| match (command) {
            crate::command::image::Sharpness::Mode(_) => SharpnessMode,
            crate::command::image::Sharpness::Reset => SharpnessReset,
            crate::command::image::Sharpness::Up => SharpnessUp,
            crate::command::image::Sharpness::Down => SharpnessDown,
            crate::command::image::Sharpness::SetLevel { .. } => SharpnessDirect,
        },
    };
    crate::command::image::Luminance => Luminance: Plain { size: 9 };
    crate::command::image::Contrast => Contrast: Plain { size: 9 };
    crate::command::image::GammaCommand => Gamma: Plain { size: 6 };
    crate::command::image::BacklightCommand => Backlight: Plain { size: 6 };
    crate::command::image::NoiseReduction2DModeCommand => NoiseReduction2dMode: Plain { size: 6 };
    crate::command::image::NoiseReduction2D => NoiseReduction2d: Plain {
        size: 6,
        rows: |command| match (command) {
            crate::command::NoiseReduction2D::Off => NoiseReduction2dOff,
            crate::command::NoiseReduction2D::Level(_) => NoiseReduction2d,
        },
    };
    crate::command::image::NoiseReduction3D => NoiseReduction3d: Plain {
        size: 6,
        rows: |command| match (command) {
            crate::command::NoiseReduction3D::Off => NoiseReduction3dOff,
            crate::command::NoiseReduction3D::Level(_) => NoiseReduction3d,
        },
    };
    crate::command::image::ImageFlipCombinedCommand => ImageFlipBoth: Plain { size: 6 };
    crate::command::image::PictureEffectCommand => PictureEffect: Plain { size: 6 };
    crate::command::focus::FocusZoneCommand => FocusZone: Plain { size: 6 };
    crate::command::focus::AutoFocusSensitivityCommand => FocusAutoSensitivity: Plain { size: 6 };
    crate::command::focus::FocusNearLimitCommand => FocusNearLimit: Plain { size: 9 };
    crate::command::white_balance::WhiteBalanceCommand => WhiteBalanceAuto: Plain {
        size: 6,
        rows: |command| match (command.mode) {
            crate::command::WhiteBalanceMode::Auto => WhiteBalanceAuto,
            crate::command::WhiteBalanceMode::Indoor => WhiteBalanceIndoor,
            crate::command::WhiteBalanceMode::Outdoor => WhiteBalanceOutdoor,
            crate::command::WhiteBalanceMode::OnePush => WhiteBalanceOnePush,
            crate::command::WhiteBalanceMode::ATW => WhiteBalanceAutoTracking,
            crate::command::WhiteBalanceMode::Manual => WhiteBalanceManual,
            crate::command::WhiteBalanceMode::Preset => WhiteBalancePreset,
            crate::command::WhiteBalanceMode::ColorTemperature => WhiteBalanceColorTemperature,
        },
    };
    crate::command::white_balance::AWBSensitivityCommand => AutoWhiteBalanceSensitivity: Plain {
        size: 6,
    };
    crate::command::menu::SetMenuDisplay => MenuDisplay: Plain { size: 6 };
    crate::command::menu::MenuNavigate => MenuNavigate: AppliedOnly { size: 9 };
    crate::command::menu::PerformMenuAction => MenuSelect: Plain {
        size: 6,
        rows: |command| match (command.action) {
            crate::command::menu::MenuAction::Select => MenuSelect,
            crate::command::menu::MenuAction::Cancel => MenuCancel,
        },
    };
    crate::command::menu::DirectMenuControl => DirectMenu: Plain { size: 8 };
    crate::command::streaming::UsbAudio => UsbAudioOn: Plain {
        size: 7,
        rows: |command| match (command) {
            crate::command::streaming::UsbAudio::On => UsbAudioOn,
            crate::command::streaming::UsbAudio::Off => UsbAudioOff,
        },
    };
    crate::command::system::SettingsSaveCommand => SettingsSave: Plain { size: 6 };
    crate::command::tally::TallyRedOn => TallyRedOn: Plain { size: 8 };
    crate::command::tally::TallyRedOff => TallyRedOff: Plain { size: 8 };
    crate::command::tally::TallyGreenOn => TallyGreenOn: Plain { size: 8 };
    crate::command::tally::TallyGreenOff => TallyGreenOff: Plain { size: 8 };
    crate::command::motion_sync::SetMotionSyncMode => MotionSyncMode: Plain { size: 6 };
    crate::command::motion_sync::SetMotionSyncPreset => MotionSyncPreset: Plain { size: 6 };
    // These three protocol controls deliberately have crate-private request
    // implementations. Address assignment and interface clear run only while
    // the serial transport handshake owns the wire; socket cancellation is
    // emitted by the operation owner that holds the exact request/socket
    // correlation.
    crate::command::system::AddressSetCommand => AddressSet: Plain { size: 4 };
    crate::command::system::InterfaceClearCommand => InterfaceClear: Plain { size: 5 };
    crate::command::system::CommandCancelCommand => CommandCancel: Plain {
        size: 3,
        policy: (Quick, Never, Urgent),
    };
}

/// Separate vertical-flip command with its own VISCA opcode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFlipCommand(pub Flip);

impl ImageFlipCommand {
    /// Creates a vertical-flip request.
    #[must_use]
    pub const fn new(flip: Flip) -> Self {
        Self(flip)
    }
}

builtin_request! {
    ImageFlipCommand => ImageFlipOff: Plain {
        size: 6,
        wire: |value: &ImageFlipCommand| crate::command::flip::ImageFlip { flip: value.0 },
        rows: |command| match (command.0) {
            Flip::On => ImageFlipVertical,
            Flip::Off => ImageFlipOff,
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

    /// Returns whether mirroring is enabled.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
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

/// One semantic row tied to a concrete typed request implementation.
///
/// Each inventory entry is an inline const that asserts, while the `#[used]`
/// inventory is initialized, that its request type may serve its row. The
/// test-only metadata then gives the runtime audit a readable row; that audit
/// lives with the preparation-backed request tests.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinTypedRequestCoverage {
    /// Authoritative semantic ledger row.
    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub row: BuiltinCommand,
    /// Stable source type name used by diagnostics and audits.
    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub type_name: &'static str,
    /// Command branch/value family represented by the typed request.
    #[cfg(all(test, any(feature = "blocking", feature = "async")))]
    pub branch: &'static str,
}

/// The one construction site of [`BuiltinTypedRequestCoverage`].
///
/// The inline const asserts at compile time that `$ty` serves `$row`. A
/// `by_value` request supplies a constant value or a constant witness for an
/// argument-bearing constructor. Its `ledger_row` must select `$row`; generated
/// row tests also check that the evaluated request selects the same row. Other
/// request forms are checked on their type alone.
macro_rules! typed_request_coverage {
    ($row:ident, $ty:ty, [by_value [$witness:expr] $request:expr]) => {
        typed_request_coverage!(@entry $row, $ty, stringify!([by_value [$witness] $request]),
            assert_value_selects_row::<$ty>(BuiltinCommand::$row, ($witness).ledger_row()))
    };
    ($row:ident, $ty:ty, [by_value $($value:tt)*]) => {
        typed_request_coverage!(@entry $row, $ty, stringify!([by_value $($value)*]),
            assert_value_selects_row::<$ty>(BuiltinCommand::$row, ($($value)*).ledger_row()))
    };
    ($row:ident, $ty:ty, $request:tt) => {
        typed_request_coverage!(@entry $row, $ty, stringify! $request,
            assert_row_served_by::<$ty>(BuiltinCommand::$row))
    };
    (@exception $row:ident, $ty:ty, $kind:ident) => {
        typed_request_coverage!(@entry $row, $ty, stringify!($kind),
            assert_row_served_by::<$ty>(BuiltinCommand::$row))
    };
    (@entry $row:ident, $ty:ty, $branch:expr, $check:expr) => {
        const {
            $check;
            BuiltinTypedRequestCoverage {
                #[cfg(all(test, any(feature = "blocking", feature = "async")))]
                row: BuiltinCommand::$row,
                #[cfg(all(test, any(feature = "blocking", feature = "async")))]
                type_name: stringify!($ty),
                #[cfg(all(test, any(feature = "blocking", feature = "async")))]
                branch: $branch,
            }
        }
    };
}

/// Fails const evaluation unless `T` may serve `row` through a request form
/// that is not a constant: the row must classify exactly like `T`'s own
/// ledger row (class, axes and state effect) and carry the same typed
/// capability gate. Value-dependent gates are still checked by the request.
const fn assert_row_served_by<T: BuiltinRequestContract>(row: BuiltinCommand) {
    assert!(
        row.classification()
            .same_contract(T::LEDGER_ROW.classification()),
        "a typed request serves a ledger row whose classification differs from its own row",
    );
    assert!(
        row_gate(row).same_as(row_gate(T::LEDGER_ROW)),
        "a typed request serves ledger rows with different capability gates",
    );
}

/// Fails const evaluation unless the constant request of a `by_value` noun
/// row selects that row: `selected` (the value's `ledger_row`) must be `row`,
/// and `row` must classify exactly like `T`'s own ledger row.
const fn assert_value_selects_row<T: BuiltinRequestContract>(
    row: BuiltinCommand,
    selected: BuiltinCommand,
) {
    assert!(
        row.classification()
            .same_contract(T::LEDGER_ROW.classification()),
        "a typed request serves a ledger row whose classification differs from its own row",
    );
    assert!(
        selected as usize == row as usize,
        "a `by_value` noun row sends a value whose ledger row is a different row",
    );
}

/// Generates the typed-request inventory from the noun table.
///
/// Every noun row that names a command contributes one entry for its request
/// type, and every protocol exception one entry for its request type, so the
/// inventory has no hand-written row. Rows are repeated when one homogeneous
/// type serves several protocol branches. Inquiry and convenience rows (`[]`)
/// contribute nothing: their request types are checked by the facades'
/// `execute`/`submit` bounds. The first arm normalizes each row to its
/// command, request type and bracketed request tokens; the second emits the
/// entries.
macro_rules! typed_request_inventory {
    (@entries
        $( [$($command:ident)?] $ret:ty, $request:tt; )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exty:ty; )*
    ) => {
        /// Compile-time coverage of every semantic built-in row by its typed
        /// request; see `typed_request_coverage!`.
        #[used]
        pub(crate) static BUILTIN_TYPED_REQUEST_INVENTORY: &[BuiltinTypedRequestCoverage] = {
            use crate::{command, request::builtin, types};
            &[
                $( $( typed_request_coverage!($command, $ret, $request), )? )*
                $( typed_request_coverage!(@exception $excommand, $exty, $exkind), )*
            ]
        };
    };

    (
        $(
            @noun $noun:ident { $($header:tt)* };
            $(
                $(#[$doc:meta])*
                $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions;
        $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        typed_request_inventory!(@entries
            $( $( [$($command)?] $ret, [$($request)*]; )* )*
            @exceptions; $( $exkind [$excommand] $exty; )*);
    };
}

noun_table!(typed_request_inventory);

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
            return Err(Error::InvalidRequest(
                "use PanTiltStop for the stop operation".into(),
            ));
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
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
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
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn for_target_speed_level(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
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
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
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
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn for_target_speed_level(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
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
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
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
    /// domain. [`ZoomDomain::Optical`] always normalizes across the profile's
    /// documented optical range; [`ZoomDomain::OpticalPlusDigital`] requires a
    /// documented digital maximum and never falls back to the optical one.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected domain has no documented maximum for
    /// `profile`, or when the mapped raw value is outside the VISCA zoom range.
    ///
    /// [`ZoomDomain::Optical`]: crate::ZoomDomain::Optical
    /// [`ZoomDomain::OpticalPlusDigital`]: crate::ZoomDomain::OpticalPlusDigital
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

impl IrisReset {
    /// Creates an iris-reset request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

builtin_request! {
    IrisReset => IrisReset: Targeted {
        size: 6,
        wire: |_: &IrisReset| Iris::Reset,
    };
}

/// Increase the iris/aperture by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IrisUp;

impl IrisUp {
    /// Creates an iris-step-up request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

builtin_request! {
    IrisUp => IrisUp: Targeted {
        size: 6,
        wire: |_: &IrisUp| Iris::Up,
    };
}

/// Decrease the iris/aperture by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IrisDown;

impl IrisDown {
    /// Creates an iris-step-down request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

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

    /// Returns the requested iris level.
    #[must_use]
    pub const fn level(self) -> IrisLevel {
        self.0
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
pub struct NdFilterDirect(NdFilterValue);

impl NdFilterDirect {
    /// Creates a direct ND-filter target.
    #[must_use]
    pub const fn new(value: NdFilterValue) -> Self {
        Self(value)
    }

    /// Returns the requested ND-filter value.
    #[must_use]
    pub const fn value(self) -> NdFilterValue {
        self.0
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

impl NdFilterStepUp {
    /// Creates an ND-filter-step-up request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

builtin_request! {
    NdFilterStepUp => NdFilterStepUp: Targeted {
        size: 7,
        wire: |_: &NdFilterStepUp| NdFilterStepCommand::new(NdFilterStep::Up),
    };
}

/// Decrease the ND filter by one camera-defined step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NdFilterStepDown;

impl NdFilterStepDown {
    /// Creates an ND-filter-step-down request.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

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
            return Err(Error::InvalidRequest(
                "pan/tilt position belongs to a different target mount".into(),
            ));
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
            return Err(Error::InvalidRequest(
                "pan/tilt limit belongs to a different target mount".into(),
            ));
        }
        Ok(())
    }

    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        self.position.validate(profile)
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::PanTiltLimits(
            crate::PanTiltLimitUpdate::set(
                self.corner,
                crate::camera::PanTiltPosition::new(self.position.pan, self.position.tilt),
            ),
        ))
    }
}

impl BuiltinValidation for PanTiltLimitClear {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        if profile
            .pan_tilt_coordinates()
            .is_none_or(|conversion| conversion.wire_codec() != self.wire_codec)
        {
            return Err(Error::InvalidRequest(
                "pan/tilt limit-clear request was built for a different wire codec".into(),
            ));
        }
        Ok(())
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::PanTiltLimits(
            crate::PanTiltLimitUpdate::clear(self.corner),
        ))
    }
}

impl BuiltinValidation for ZoomTarget {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        let zoom = zoom_facts(profile)?;
        let digital = capabilities.permits_typed(TypedSupportSurface::DigitalZoomRange);
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
            require(capabilities.has_auto_focus(), "automatic focus")?;
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

fn validate_preset(profile: &crate::ProfileSpec, preset: PresetNumber) -> Result<(), Error> {
    let facts = presets_facts(profile)?;
    require_in_range(
        "preset number",
        preset.value(),
        &crate::capabilities::CapabilityRange::<u8>::new(0, facts.highest),
    )
}

impl BuiltinValidation for PresetRecall {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.preset)?;
        if profile.preset_recall_axes() == Some(self.axes) {
            Ok(())
        } else {
            Err(Error::InvalidRequest(
                "preset recall was prepared for a different profile axis set".into(),
            ))
        }
    }
}

impl BuiltinValidation for PresetSet {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.0)
    }
}

impl BuiltinValidation for PresetReset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_preset(profile, self.0)
    }
}

impl BuiltinValidation for crate::command::preset::PresetRecallSpeedCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let speed = self.speed.value();
        let range =
            presets_facts(profile)?
                .speed_range
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "preset recall speed range",
                })?;
        require_in_range("preset recall speed", speed, range)
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::PresetRecallSpeed(self.speed.value()))
    }
}

impl BuiltinValidation for crate::command::focus::FocusLock {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::FocusLockMode(matches!(self, Self::On)))
    }
}

impl BuiltinValidation for crate::command::exposure::SpotlightOn {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::Spotlight(true))
    }
}

impl BuiltinValidation for crate::command::exposure::SpotlightOff {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::Spotlight(false))
    }
}

impl BuiltinValidation for crate::command::exposure::AutoSlowShutterOn {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::AutoSlowShutter(true))
    }
}

impl BuiltinValidation for crate::command::exposure::AutoSlowShutterOff {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::AutoSlowShutter(false))
    }
}

impl BuiltinValidation for crate::command::nd_filter::NdFilterModeCommand {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::NdFilterMode(self.mode()))
    }
}

impl BuiltinValidation for crate::command::nd_filter::AutoNdCommand {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::AutoNdFilter(self.enabled()))
    }
}

impl BuiltinValidation for crate::command::flip::ImageFreeze {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::ImageFreeze(self.enabled()))
    }
}

impl BuiltinValidation for crate::command::zoom::DigitalZoom {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::DigitalZoomMode(self.enabled()))
    }
}

impl BuiltinValidation for crate::command::streaming::MulticastStreaming {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::MulticastStreaming(matches!(
            self,
            Self::On
        )))
    }
}

impl BuiltinValidation for crate::command::streaming::SetNdiQuality {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::NdiQuality(self.quality))
    }
}

impl BuiltinValidation for crate::command::tally::TallyBrightLo {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::TallyBrightness(false))
    }
}

impl BuiltinValidation for crate::command::tally::TallyBrightHi {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::TallyBrightness(true))
    }
}

impl BuiltinValidation for crate::command::variable_speed::SetVariableSpeedMode {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::VariableSpeedMode(self.mode))
    }
}

impl BuiltinValidation for crate::command::tally::TallyOn {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::TallyMode(true))
    }
}

impl BuiltinValidation for crate::command::tally::TallyOff {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        known_projection(crate::KnownState::TallyMode(false))
    }
}

impl BuiltinValidation for crate::command::tally::TallyFlash {
    #[cfg(any(feature = "blocking", feature = "async"))]
    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        invalidated_projection::<Self>()
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
            AntiFlickerCommand, AutoNdCommand, AutoSlowShutterOff, AutoSlowShutterOn, DigitalZoom,
            ExposureCommand, ExposureMode, FocusLock, ImageFreeze, MulticastStreaming,
            NdFilterMode, NdFilterModeCommand, PanTiltLimitCorner, PresetRecallSpeed,
            SetNdiQuality, SettingsSaveCommand, SpotlightOff, SpotlightOn, TallyBrightHi,
            TallyBrightLo, TallyFlash, TallyOff, TallyOn, TallyRedOn, VariableSpeedMode,
            WhiteBalanceCommand, WhiteBalanceMode,
        },
        prepared::{
            test_support::{prepare_builtin_command, prepare_builtin_operation, prepare_command},
            ClassSelection,
        },
        profiles::{
            GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
            SonyBRCH900, SonyEVIH100, SonyFR7,
        },
        types::NdiQuality,
        OperationalTuning, PlainCommand, ProfileSpec,
    };
    use std::collections::HashSet;

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
            Degrees(200.0),
            Degrees(-45.0),
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
            validate_preset(&g2, PresetNumber::new(above_presets).expect("preset")),
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
        let thirty_x = ProfileSpec::from_compile_time::<PtzOptics30X>().expect("30X profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");

        for profile in [&g2, &thirty_x] {
            assert!(prepare_builtin_command(
                &crate::command::UsbAudio::On,
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
                &crate::command::UsbAudio::On,
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
        let thirty_x = ProfileSpec::from_compile_time::<PtzOptics30X>().expect("30X profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBRCH900>().expect("H900 profile");
        let evi = ProfileSpec::from_compile_time::<SonyEVIH100>().expect("EVI profile");
        let brc300 = ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC300 profile");
        let nearus = ProfileSpec::from_compile_time::<NearusBRC300>().expect("Nearus profile");
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
                        profile.capabilities().permits_typed($surface),
                        "{request:?} runtime validation must match {:?} for {}",
                        $surface,
                        profile.capabilities().model_name,
                    );
                }
            };
        }

        follows_static_surface!(
            TypedSupportSurface::PtzOpticsAntiFlicker,
            AntiFlickerCommand::new(crate::command::AntiFlickerMode::Hz50)
        );
        follows_static_surface!(
            TypedSupportSurface::PtzOpticsSettingsSave,
            SettingsSaveCommand::new()
        );
        follows_static_surface!(
            TypedSupportSurface::PtzOpticsPresetRecallSpeed,
            crate::command::PresetRecallSpeedCommand::new(
                PresetRecallSpeed::new(12).expect("valid recall speed")
            )
        );
        follows_static_surface!(TypedSupportSurface::SonySpotlight, SpotlightOn::new());
        follows_static_surface!(TypedSupportSurface::SonySpotlight, SpotlightOff::new());
        follows_static_surface!(
            TypedSupportSurface::SonyAutoSlowShutter,
            AutoSlowShutterOn::new()
        );
        follows_static_surface!(
            TypedSupportSurface::SonyAutoSlowShutter,
            AutoSlowShutterOff::new()
        );
        follows_static_surface!(
            TypedSupportSurface::PtzOpticsMulticastStreaming,
            MulticastStreaming::On
        );
        follows_static_surface!(
            TypedSupportSurface::PtzOpticsMulticastStreaming,
            MulticastStreaming::Off
        );
        follows_static_surface!(
            TypedSupportSurface::PtzOpticsNdiQuality,
            SetNdiQuality::new(NdiQuality::High)
        );
        follows_static_surface!(
            completion::AppliedOnly: TypedSupportSurface::OnePushFocus,
            FocusTrigger::OnePush
        );
        follows_static_surface!(
            completion::AppliedOnly: TypedSupportSurface::PtzOpticsSnapFocus,
            FocusTrigger::Snap
        );
        follows_static_surface!(
            TypedSupportSurface::OnePushWhiteBalance,
            WhiteBalanceCommand::new(WhiteBalanceMode::OnePush)
        );
        follows_static_surface!(
            TypedSupportSurface::AutoTrackingWhiteBalance,
            WhiteBalanceCommand::new(WhiteBalanceMode::ATW)
        );
        follows_static_surface!(
            TypedSupportSurface::ColorTemperature,
            WhiteBalanceCommand::new(WhiteBalanceMode::ColorTemperature)
        );
    }

    /// Each white-balance mode is gated by the typed gate of the ledger row
    /// it selects. A built-in profile's mode list already rejects every mode
    /// whose typed gate it denies, so this runtime profile admits every mode
    /// and withdraws one row gate at a time.
    #[test]
    fn white_balance_modes_follow_the_typed_gate_of_their_own_row() {
        const MODES: [WhiteBalanceMode; 7] = [
            WhiteBalanceMode::Auto,
            WhiteBalanceMode::Indoor,
            WhiteBalanceMode::Outdoor,
            WhiteBalanceMode::OnePush,
            WhiteBalanceMode::ATW,
            WhiteBalanceMode::Manual,
            WhiteBalanceMode::ColorTemperature,
        ];
        let gated = [
            (
                WhiteBalanceMode::OnePush,
                TypedSupportSurface::OnePushWhiteBalance,
                "one-push white balance",
            ),
            (
                WhiteBalanceMode::ATW,
                TypedSupportSurface::AutoTrackingWhiteBalance,
                "auto-tracking white balance",
            ),
            (
                WhiteBalanceMode::ColorTemperature,
                TypedSupportSurface::ColorTemperature,
                "color-temperature white balance",
            ),
        ];

        let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "Every white-balance mode".into();
        capabilities
            .white_balance
            .as_mut()
            .expect("white balance")
            .modes = MODES.to_vec();
        capabilities
            .white_balance
            .as_mut()
            .expect("white balance")
            .one_push = true;
        capabilities
            .white_balance
            .as_mut()
            .expect("white balance")
            .color_temp_range = Some(crate::capabilities::CapabilityRange::<u16>::new(2500, 8000));
        capabilities.typed_support =
            capabilities
                .typed_support
                .union(crate::capabilities::TypedSupportSet::from_surfaces(&[
                    TypedSupportSurface::OnePushWhiteBalance,
                    TypedSupportSurface::AutoTrackingWhiteBalance,
                    TypedSupportSurface::ColorTemperature,
                ]));
        let profile = |typed_support| {
            let mut capabilities = capabilities.clone();
            capabilities.typed_support = typed_support;
            ProfileSpec::builder(capabilities)
                .transports(source.transports())
                .envelope(source.envelope())
                .timing(source.timing())
                .maximum_command_sockets(source.maximum_command_sockets())
                .supports_operation_complete(source.supports_operation_complete())
                .supports_command_cancel(source.supports_command_cancel())
                .position_inquiries(source.position_inquiries())
                .build()
                .expect("runtime profile admitting every white-balance mode")
        };

        let full = profile(capabilities.typed_support);
        for mode in MODES {
            assert!(
                WhiteBalanceCommand::new(mode)
                    .validate_for_target(&crate::TargetRegistration::from_profile(
                        crate::target::TargetSlot::CAMERA_1,
                        full.clone()
                    ))
                    .is_ok(),
                "{mode:?} must be admitted when every row gate is permitted"
            );
        }
        for (denied, surface, feature) in gated {
            let profile = profile(capabilities.typed_support.without(surface));
            for mode in MODES {
                let result = WhiteBalanceCommand::new(mode).validate_for_target(
                    &crate::TargetRegistration::from_profile(
                        crate::target::TargetSlot::CAMERA_1,
                        profile.clone(),
                    ),
                );
                if mode == denied {
                    assert!(
                        matches!(result, Err(Error::FeatureNotSupported { .. })),
                        "{mode:?} without {surface:?} must report {feature:?}, got {result:?}"
                    );
                } else {
                    assert!(
                        result.is_ok(),
                        "{mode:?} must not depend on {surface:?}, got {result:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn profile_aware_speed_level_lowering_keeps_standard_pairs_and_mirrors_brc300() {
        let standard = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let g3 = ProfileSpec::from_compile_time::<PtzOpticsG3>().expect("G3 profile");
        let thirty_x = ProfileSpec::from_compile_time::<PtzOptics30X>().expect("30X profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBRCH900>().expect("H900 profile");
        let evi = ProfileSpec::from_compile_time::<SonyEVIH100>().expect("EVI profile");
        let brc300 = ProfileSpec::from_compile_time::<SonyBRC300>().expect("BRC-300 profile");
        let nearus = ProfileSpec::from_compile_time::<NearusBRC300>().expect("Nearus profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");

        let standard_absolute = PanTiltAbsolute::for_target_speed_level(
            Degrees(10.0),
            Degrees(-5.0),
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
            Degrees(45.0),
            Degrees(15.0),
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
            Degrees(45.0),
            Degrees(15.0),
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
                Degrees(0.0),
                Degrees(0.0),
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
                Degrees(0.0),
                Degrees(0.0),
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
        use crate::command::menu::DirectMenuControl;

        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");

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
                        u8::from(target),
                        u8::from(button),
                        0xFF
                    ]
                );
            }
        }
    }

    /// Preset 255 and a documented direct-menu button prepare on Sony FR7.
    #[test]
    fn preset_255_and_direct_menu_prepare_on_fr7() {
        use crate::command::menu::DirectMenuControl;

        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let preset = PresetNumber::new(255).expect("255 is a valid preset number");

        prepare_builtin_command(
            &PresetSet::new(preset),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("preset 255 set must prepare end-to-end on FR7");

        prepare_builtin_command(
            &PresetReset::new(preset),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("preset 255 reset must prepare end-to-end on FR7");

        prepare_builtin_operation::<completion::Targeted, _>(
            &PresetRecall::for_profile(preset, &fr7).expect("FR7 preset recall"),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("preset 255 recall must prepare end-to-end on FR7");

        prepare_builtin_command(
            &DirectMenuControl::new(
                crate::command::DirectMenuTarget::Exit,
                crate::command::MenuButton::Release,
            ),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                fr7.clone(),
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("documented direct menu control must prepare end-to-end on FR7");
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
        requirement: crate::command::semantics::AppliedStateEffectRequirement,
        expected: crate::runtime::engine::AppliedStateProjection,
    ) where
        C: PlainCommand + ?Sized,
    {
        assert_eq!(
            semantic.classification(),
            crate::command::semantics::BuiltinRequestClass::Plain {
                state_effect: Some(requirement),
            },
            "{name} classification"
        );
        let actual = prepared_state(command, profile);
        assert_eq!(actual, expected, "{name} applied-state projection");
        assert_eq!(actual.key(), requirement.state(), "{name} ledger key");
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
        requirement: crate::command::semantics::AppliedStateEffectRequirement,
        expected: crate::runtime::engine::AppliedStateProjection,
    ) where
        C: BuiltinValidation + ?Sized,
    {
        assert_eq!(
            semantic.classification(),
            crate::command::semantics::BuiltinRequestClass::Plain {
                state_effect: Some(requirement),
            },
            "{name} classification"
        );
        let actual = BuiltinValidation::applied_state(command)
            .expect("state command must expose an applied-state projection");
        assert_eq!(actual, expected, "{name} applied-state projection");
        assert_eq!(actual.key(), requirement.state(), "{name} ledger key");
    }

    #[test]
    fn every_write_only_state_command_has_an_exact_closed_projection() {
        use crate::command::semantics::{
            AppliedStateEffectRequirement::{Clear, Invalidate, Set},
            BuiltinCommand as B,
        };
        use crate::StateKey as S;

        let ptz = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZ profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let evi = ProfileSpec::from_compile_time::<SonyEVIH100>().expect("EVI profile");

        macro_rules! row {
            ($name:literal, $command:expr, $profile:expr, $semantic:expr, $requirement:expr, $expected:expr) => {{
                let command = $command;
                assert_state_row(
                    $name,
                    &command,
                    $profile,
                    $semantic,
                    $requirement,
                    $expected,
                );
            }};
        }

        macro_rules! semantic_row {
            ($name:literal, $command:expr, $semantic:expr, $requirement:expr, $expected:expr) => {{
                let command = $command;
                assert_semantic_state_row($name, &command, $semantic, $requirement, $expected);
            }};
        }

        // Limits carry their typed corner and optional converted position.
        row!(
            "pan/tilt limit set",
            PanTiltLimitSet::for_target(
                PanTiltLimitCorner::DownLeft,
                Degrees(0.0),
                Degrees(0.0),
                &crate::TargetRegistration::new(CameraId::CAMERA_1, ptz.clone())
                    .expect("target registration"),
            )
            .expect("limit set"),
            &ptz,
            B::PanTiltLimitSet,
            Set(S::PanTiltLimits),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::PanTiltLimits(
                crate::PanTiltLimitUpdate::set(
                    PanTiltLimitCorner::DownLeft,
                    crate::camera::PanTiltPosition::new(0, 0)
                )
            ))
        );
        row!(
            "pan/tilt limit clear",
            PanTiltLimitClear::new(PanTiltLimitCorner::UpRight),
            &ptz,
            B::PanTiltLimitClear,
            Clear(S::PanTiltLimits),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::PanTiltLimits(
                crate::PanTiltLimitUpdate::clear(PanTiltLimitCorner::UpRight)
            ))
        );

        row!(
            "preset recall speed",
            crate::command::PresetRecallSpeedCommand {
                speed: PresetRecallSpeed::new(12).expect("preset speed"),
            },
            &ptz,
            B::PresetRecallSpeed,
            Set(S::PresetRecallSpeed),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::PresetRecallSpeed(12)
            )
        );
        row!(
            "focus lock on",
            FocusLock::On,
            &ptz,
            B::FocusLock,
            Set(S::FocusLockMode),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::FocusLockMode(
                true
            ))
        );
        row!(
            "focus lock off",
            FocusLock::Off,
            &ptz,
            B::FocusLock,
            Set(S::FocusLockMode),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::FocusLockMode(
                false
            ))
        );

        row!(
            "spotlight on",
            SpotlightOn::new(),
            &fr7,
            B::SpotlightOn,
            Set(S::Spotlight),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::Spotlight(true))
        );
        row!(
            "spotlight off",
            SpotlightOff::new(),
            &fr7,
            B::SpotlightOff,
            Set(S::Spotlight),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::Spotlight(
                false
            ))
        );
        row!(
            "auto slow shutter on",
            AutoSlowShutterOn::new(),
            &evi,
            B::AutoSlowShutterOn,
            Set(S::AutoSlowShutter),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::AutoSlowShutter(true)
            )
        );
        row!(
            "auto slow shutter off",
            AutoSlowShutterOff::new(),
            &evi,
            B::AutoSlowShutterOff,
            Set(S::AutoSlowShutter),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::AutoSlowShutter(false)
            )
        );

        row!(
            "ND filter preset mode",
            NdFilterModeCommand::new(NdFilterMode::Preset),
            &fr7,
            B::NdFilterMode,
            Set(S::NdFilterMode),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::NdFilterMode(
                NdFilterMode::Preset
            ))
        );
        row!(
            "ND filter variable mode",
            NdFilterModeCommand::new(NdFilterMode::Variable),
            &fr7,
            B::NdFilterMode,
            Set(S::NdFilterMode),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::NdFilterMode(
                NdFilterMode::Variable
            ))
        );
        row!(
            "automatic ND on",
            AutoNdCommand::new(true),
            &fr7,
            B::NdFilterAutoOn,
            Set(S::AutoNdFilter),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::AutoNdFilter(
                true
            ))
        );
        row!(
            "automatic ND off",
            AutoNdCommand::new(false),
            &fr7,
            B::NdFilterAutoOff,
            Set(S::AutoNdFilter),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::AutoNdFilter(
                false
            ))
        );

        semantic_row!(
            "image freeze on",
            ImageFreeze::on(),
            B::ImageFreezeOn,
            Set(S::ImageFreeze),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::ImageFreeze(
                true
            ))
        );
        semantic_row!(
            "image freeze off",
            ImageFreeze::off(),
            B::ImageFreezeOff,
            Set(S::ImageFreeze),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::ImageFreeze(
                false
            ))
        );
        row!(
            "digital zoom on",
            DigitalZoom::new(true),
            &fr7,
            B::DigitalZoom,
            Set(S::DigitalZoomMode),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::DigitalZoomMode(true)
            )
        );
        row!(
            "digital zoom off",
            DigitalZoom::new(false),
            &fr7,
            B::DigitalZoom,
            Set(S::DigitalZoomMode),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::DigitalZoomMode(false)
            )
        );

        row!(
            "multicast on",
            MulticastStreaming::On,
            &ptz,
            B::MulticastStreamingOn,
            Set(S::MulticastStreaming),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::MulticastStreaming(true)
            )
        );
        row!(
            "multicast off",
            MulticastStreaming::Off,
            &ptz,
            B::MulticastStreamingOff,
            Set(S::MulticastStreaming),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::MulticastStreaming(false)
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
                Set(S::NdiQuality),
                crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::NdiQuality(
                    quality,
                )),
            );
        }

        semantic_row!(
            "tally brightness low",
            TallyBrightLo::new(),
            B::TallyBrightLow,
            Set(S::TallyBrightness),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::TallyBrightness(false)
            )
        );
        semantic_row!(
            "tally brightness high",
            TallyBrightHi::new(),
            B::TallyBrightHigh,
            Set(S::TallyBrightness),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::TallyBrightness(true)
            )
        );
        row!(
            "variable speed standard",
            crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Standard24),
            &fr7,
            B::VariableSpeedMode,
            Set(S::VariableSpeedMode),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::VariableSpeedMode(VariableSpeedMode::Standard24)
            )
        );
        row!(
            "variable speed fine",
            crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Fine50),
            &fr7,
            B::VariableSpeedMode,
            Set(S::VariableSpeedMode),
            crate::runtime::engine::AppliedStateProjection::set(
                crate::KnownState::VariableSpeedMode(VariableSpeedMode::Fine50)
            )
        );

        // PTZOptics tally-mode opcodes retain their closed state projections
        // even though no built-in profile currently opts into their dedicated
        // typed-support surface.
        semantic_row!(
            "tally on",
            TallyOn::new(),
            B::TallyOn,
            Set(S::TallyMode),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::TallyMode(true))
        );
        semantic_row!(
            "tally off",
            TallyOff::new(),
            B::TallyOff,
            Set(S::TallyMode),
            crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::TallyMode(
                false
            ))
        );
        semantic_row!(
            "tally flash",
            TallyFlash::new(),
            B::TallyFlash,
            Invalidate(S::TallyMode),
            crate::runtime::engine::AppliedStateProjection::invalidate(S::TallyMode)
        );

        // The combined-flip opcode carries both axes, so it records the pair
        // as `[horizontal, vertical]`. The single-axis opcodes move one axis
        // and leave the other unknown, so they invalidate the same key.
        for (name, mode, horizontal, vertical) in [
            (
                "combined flip off",
                crate::command::ImageFlipMode::Off,
                0,
                0,
            ),
            (
                "combined flip horizontal",
                crate::command::ImageFlipMode::Horizontal,
                1,
                0,
            ),
            (
                "combined flip vertical",
                crate::command::ImageFlipMode::Vertical,
                0,
                1,
            ),
            (
                "combined flip both",
                crate::command::ImageFlipMode::Both,
                1,
                1,
            ),
        ] {
            assert_state_row(
                name,
                &crate::command::ImageFlipCombinedCommand::new(mode),
                &ptz,
                B::ImageFlipCombined,
                Set(S::Flip),
                crate::runtime::engine::AppliedStateProjection::set(crate::KnownState::Flip(
                    crate::command::FlipState {
                        horizontal: horizontal != 0,
                        vertical: vertical != 0,
                    },
                )),
            );
        }
        row!(
            "separate vertical flip",
            ImageFlipCommand::new(Flip::On),
            &ptz,
            B::ImageFlipVertical,
            Invalidate(S::Flip),
            crate::runtime::engine::AppliedStateProjection::invalidate(S::Flip)
        );
        row!(
            "separate horizontal mirror",
            ImageMirrorCommand::new(true),
            &ptz,
            B::ImageFlipHorizontal,
            Invalidate(S::Flip),
            crate::runtime::engine::AppliedStateProjection::invalidate(S::Flip)
        );
        row!(
            "separate horizontal mirror off",
            ImageMirrorCommand::new(false),
            &ptz,
            B::ImageFlipHorizontalOff,
            Invalidate(S::Flip),
            crate::runtime::engine::AppliedStateProjection::invalidate(S::Flip)
        );
    }

    #[test]
    fn unsupported_write_only_state_commands_fail_before_encoding() {
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");

        reset_request_write_count();
        assert!(prepare_builtin_command(
            &crate::command::PresetRecallSpeedCommand {
                speed: PresetRecallSpeed::new(12).expect("preset speed"),
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
            &FocusLock::On,
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
            &MulticastStreaming::On,
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
            &crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Standard24),
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
        assert_policy::<FocusLock>(
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
        assert_policy::<MulticastStreaming>(
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
        R: Request<Class = request::Operation<completion::Targeted>>
            + OperationCommand<completion::Targeted>,
    {
        let _ = core::marker::PhantomData::<R>;
    }

    fn assert_applied_only<R>()
    where
        R: Request<Class = request::Operation<completion::AppliedOnly>>
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
                crate::types::ColorTemp::new(0x20).expect("valid color temperature"),
            ),
            7,
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
            crate::types::ShutterSpeed::new(1),
        ));
        assert_exact_request_size(&crate::command::Brightness::Reset);
        assert_exact_request_size(&crate::command::Brightness::SetLevel(
            crate::types::BrightnessLevel::new(1),
        ));
        assert_exact_request_size(&crate::command::Gain::Reset);
        assert_exact_request_size(&crate::command::Gain::SetValue(
            crate::types::GainLevel::new(1).expect("valid gain"),
        ));
        assert_exact_request_size(&crate::command::ColorTemperature::Reset);
        assert_exact_request_size(&crate::command::ColorTemperature::SetTemperature(
            crate::types::ColorTemp::new(0x20).expect("valid color temperature"),
        ));
        assert_exact_request_size(&crate::command::RedGain::Reset);
        assert_exact_request_size(&crate::command::RedGain::SetValue(
            crate::types::RedChannel::new(1).expect("valid red gain"),
        ));
        assert_exact_request_size(&crate::command::BlueGain::Reset);
        assert_exact_request_size(&crate::command::BlueGain::SetValue(
            crate::types::BlueChannel::new(1).expect("valid blue gain"),
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
        use crate::command::image::{Sharpness, SharpnessMode};
        let fr7 = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7"),
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
        // Set equality alone cannot see a row listed twice, so pin the entry
        // count against the ledger the set came from rather than against the
        // inventory's own length.
        assert_eq!(
            BUILTIN_TYPED_REQUEST_INVENTORY.len(),
            ledger_rows.len(),
            "the typed request inventory repeats a ledger row"
        );
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
            NdFilterDirect::new(NdFilterValue::new(4).unwrap()).affected_axes(),
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

        let nd = NdFilterValue::new(4).unwrap();
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
        let source = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "Documented Digital Range Without Permission".into();
        capabilities.typed_support =
            crate::capabilities::TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
        let profile = ProfileSpec::builder(capabilities)
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .position_inquiries(source.position_inquiries())
            .build()
            .expect("runtime partial profile");

        let midpoint = crate::units::UnitInterval::new(0.5).expect("unit interval midpoint");
        let normalized =
            ZoomTarget::from_normalized(midpoint, crate::ZoomDomain::OpticalPlusDigital, &profile)
                .expect("documented digital maximum maps the midpoint");
        // The FR7 combined midpoint is 0x3800, which is still in its optical
        // range. Use that exact raw value as the direct-position control.
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
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        let nd = NdFilterDirect::new(NdFilterValue::new(4).unwrap());

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
    fn noise_reduction_readback_and_control_grants_are_independent() {
        let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "NR inquiry without NR control".into();
        capabilities.typed_support = capabilities
            .typed_support
            .without(TypedSupportSurface::NoiseReduction2DControl)
            .without(TypedSupportSurface::NoiseReduction3DControl);
        let profile = ProfileSpec::builder(capabilities)
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .position_inquiries(source.position_inquiries())
            .build()
            .expect("readback may be documented independently from control");
        assert!(profile.capabilities().has_2d_nr());
        let registration =
            crate::TargetRegistration::new(CameraId::CAMERA_1, profile).expect("target");
        Request::validate_for_target(&crate::command::NoiseReduction2DInquiry, &registration)
            .expect("readback grant");
        reset_request_write_count();
        assert!(prepare_builtin_command(
            &crate::command::NoiseReduction2D::off(),
            &registration,
            OperationalTuning::new()
        )
        .is_err());
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn profile_validation_limits_typed_iris_to_supported_profiles() {
        let ptz = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZ profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBRCH900>().expect("H900 profile");
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
            &IrisDirect::new(IrisLevel::new(0x1E).unwrap()),
            &crate::TargetRegistration::new(
                (crate::target::TargetSlot::CAMERA_1).camera(),
                h900.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_ok());

        reset_request_write_count();
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
            &NdFilterDirect::new(NdFilterValue::new(4).unwrap()),
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
    fn high_gain_values_are_admitted_by_profiles_that_advertise_them() {
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBRCH900>().expect("H900 profile");
        let gain = crate::command::Gain::SetValue(
            crate::types::GainLevel::new(0x0C).expect("gain nibble must be representable"),
        );

        for profile in [&fr7, &h900] {
            assert!(
                prepare_builtin_command(
                    &gain,
                    &crate::TargetRegistration::new(
                        (crate::target::TargetSlot::CAMERA_1).camera(),
                        profile.clone()
                    )
                    .expect("valid target registration"),
                    OperationalTuning::new(),
                )
                .is_ok(),
                "{} advertises 0x0C gain",
                profile.capabilities().model_name,
            );
        }
    }

    #[test]
    fn fr7_rejects_every_shared_ae_mode_before_encoding() {
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");

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
        let generic = ProfileSpec::builder(capabilities)
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .position_inquiries(source.position_inquiries())
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
            .permits_typed(TypedSupportSurface::ExposureMode));

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
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let generic = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBRCH900>().expect("H900 profile");

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
        let facts = presets_facts(profile)?;
        require_in_range(
            "preset number",
            self.preset().value(),
            &crate::capabilities::CapabilityRange::<u8>::new(0, facts.highest),
        )
    }
}
impl BuiltinValidation for crate::command::Fr7SetSeparatePresetSpeed {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let facts = presets_facts(profile)?;
        require_in_range(
            "preset number",
            self.preset().value(),
            &crate::capabilities::CapabilityRange::<u8>::new(0, facts.highest),
        )
    }
}
impl BuiltinValidation for crate::command::Fr7SetSeparatePresetDuration {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let facts = presets_facts(profile)?;
        require_in_range(
            "preset number",
            self.preset().value(),
            &crate::capabilities::CapabilityRange::<u8>::new(0, facts.highest),
        )
    }
}
