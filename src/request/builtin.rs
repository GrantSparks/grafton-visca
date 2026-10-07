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
        surface::typed_surface_for_command,
        Flip, Focus, Iris, NdFilterMode, NdFilterStep, NdFilterStepCommand, NdFilterValue, PanTilt,
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

#[cfg(test)]
pub(crate) fn reset_request_write_count() {
    REQUEST_WRITE_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn request_write_count() -> usize {
    REQUEST_WRITE_COUNT.with(std::cell::Cell::get)
}
use crate::types::{
    FocusPosition, IrisLevel, PanSpeed, SpeedLevel, TiltSpeed, ZoomPosition, ZoomSpeed,
};
use crate::{
    capabilities::PanTiltWireCodec,
    command::pan_tilt::{PanTiltFraming, PanTiltProfiled},
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
/// with no repeated pattern, and sets `SELECTS_ROW_BY_VALUE`. Each pattern is
/// a path naming one variant, so a wildcard or binding cannot absorb a
/// variant added later. The validator
/// gates each value on the row it selects. Every row then classifies like
/// the entry's row, and every noun row sending the type must be a `by_value`
/// row whose value selects that noun row's command: the typed-request
/// inventory checks both at compile time.
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

    (@wire $this:tt) => { *$this };
    (@wire $this:tt, $wire:expr) => { ($wire)($this) };

    (@contract $type:ty, $row:ident [$(profile_axes)?] [$($by_value:ident)?]) => {
        impl BuiltinRequestContract for $type {
            const LEDGER_ROW: BuiltinCommand = BuiltinCommand::$row;
            $( builtin_request!(@selects_row_by_value $by_value); )?
        }
    };
    (@contract $type:ty, $row:ident [$flag:ident] $by_value:tt) => {
        compile_error!(concat!(
            "unknown builtin_request! flag `",
            stringify!($flag),
            "`; expected profile_axes"
        ));
    };
    (@selects_row_by_value $value:ident) => {
        const SELECTS_ROW_BY_VALUE: bool = true;
    };

    (@rows $type:ty, |$value:ident| $($scrutinee:ident).+ {
        $($pattern:path => $row:ident),+
    }) => {
        impl $type {
            /// The ledger row this value serves, whose typed capability gate
            /// admits it.
            #[deny(unreachable_patterns)]
            pub(crate) const fn ledger_row(&self) -> BuiltinCommand {
                let $value = self;
                match $($scrutinee).+ {
                    $( $pattern => BuiltinCommand::$row, )+
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
            $(, rows: |$value:ident| match $($scrutinee:ident).+ {
                $($pattern:path => $value_row:ident),+ $(,)?
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

            fn encoded_size(&self) -> usize {
                encoded_len(&builtin_request!(@wire self $(, $wire)?), Self::MAX_SIZE)
            }

            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                #[cfg(test)]
                REQUEST_WRITE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
                WireEncode::write_into(
                    &builtin_request!(@wire self $(, $wire)?),
                    camera_id,
                    buffer,
                )
            }

            fn validate_for_profile(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
                <Self as BuiltinValidation>::validate(self, profile)
            }

            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn admission_control_class(
                &self,
                _authority: crate::requests::RequestContractAuthority,
            ) -> Result<ControlClass, Error> {
                Ok(Self::CONTROL_CLASS)
            }

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
        $( builtin_request!(@rows $type, |$value| $($scrutinee).+ {
            $($pattern => $value_row),+
        }); )?
    )+};
}

/// Exact frame length of a built-in wire value, measured by a dry run into an
/// empty buffer.
///
/// Every built-in encoder writes through the crate's frame writer, which
/// counts a frame that does not fit and reports its exact length. A value the
/// encoder rejects reports `fallback`; writing it then fails with the same
/// error.
fn encoded_len(wire: &impl WireEncode, fallback: usize) -> usize {
    match WireEncode::write_into(wire, CameraId::CAMERA_1, &mut []) {
        Ok(length) => length,
        Err(Error::BufferTooSmall { required, .. }) => required,
        Err(_) => fallback,
    }
}

pub(crate) trait BuiltinValidation {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error>;

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
    range: &std::ops::RangeInclusive<T>,
) -> Result<(), Error>
where
    T: Copy + PartialOrd + Into<i32>,
{
    if range.contains(&value) {
        Ok(())
    } else {
        Err(Error::parameter_out_of_range(
            parameter,
            value.into(),
            (*range.start()).into(),
            (*range.end()).into(),
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

fn validate_pan_tilt(profile: &crate::ProfileSpec) -> Result<(), Error> {
    require(profile.capabilities().has_pan_tilt, "pan/tilt control")
}

fn validate_pan_tilt_speed(
    profile: &crate::ProfileSpec,
    pan: PanSpeed,
    tilt: TiltSpeed,
) -> Result<(), Error> {
    validate_pan_tilt(profile)?;
    let capabilities = profile.capabilities();
    require_in_range("pan speed", pan.value(), &capabilities.pan_speed)?;
    require_in_range("tilt speed", tilt.value(), &capabilities.tilt_speed)?;
    Ok(())
}

/// Validates paired speeds for a position command whose profile may carry a
/// one-speed wire form.
fn validate_pan_tilt_position_speed(
    profile: &crate::ProfileSpec,
    pan: PanSpeed,
    tilt: TiltSpeed,
) -> Result<(), Error> {
    validate_pan_tilt_speed(profile, pan, tilt)?;
    if profile
        .pan_tilt_coordinates()
        .is_some_and(|conversion| conversion.wire_codec() == PanTiltWireCodec::SonyBrc300)
        && pan.value() != tilt.value()
    {
        return Err(Error::InvalidRequest(
            "Sony BRC-300 absolute and relative position commands have one speed byte; pan and tilt speeds must match".into(),
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
fn pan_tilt_position_speeds_from_level(
    speed: SpeedLevel,
    profile: &crate::ProfileSpec,
) -> Result<(PanSpeed, TiltSpeed), Error> {
    let capabilities = profile.capabilities();
    let brc300_framing = profile
        .pan_tilt_coordinates()
        .is_some_and(|conversion| conversion.wire_codec() == PanTiltWireCodec::SonyBrc300);

    if brc300_framing {
        let minimum = (*capabilities.pan_speed.start()).max(*capabilities.tilt_speed.start());
        let maximum = (*capabilities.pan_speed.end()).min(*capabilities.tilt_speed.end());
        let speed = speed.to_pan_speed().clamp(minimum, maximum);
        let pan_speed = PanSpeed::new(speed)?;
        let tilt_speed = TiltSpeed::new(speed)?;
        Ok((pan_speed, tilt_speed))
    } else {
        let pan_speed = PanSpeed::new(speed.to_pan_speed().clamp(
            *capabilities.pan_speed.start(),
            *capabilities.pan_speed.end(),
        ))?;
        let tilt_speed = TiltSpeed::new(speed.to_tilt_speed().clamp(
            *capabilities.tilt_speed.start(),
            *capabilities.tilt_speed.end(),
        ))?;
        Ok((pan_speed, tilt_speed))
    }
}

fn validate_pan_tilt_position(
    profile: &crate::ProfileSpec,
    pan: i32,
    tilt: i32,
    conversion: PanTiltCoordinateConversion,
) -> Result<(), Error> {
    validate_pan_tilt(profile)?;
    let capabilities = profile.capabilities();
    if profile.pan_tilt_coordinates() != Some(conversion) {
        return Err(Error::InvalidRequest(
            "pan/tilt request was converted for a different profile".into(),
        ));
    }
    require_in_range("pan position", pan, &capabilities.pan_range)?;
    require_in_range("tilt position", tilt, &capabilities.tilt_range)?;
    Ok(())
}

fn validate_iris_control(profile: &crate::ProfileSpec, row: BuiltinCommand) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(
        capabilities.has_exposure && capabilities.iris_range.is_some(),
        "iris control",
    )?;
    validate_static_typed_command(profile, row, "typed iris control")
}

fn validate_nd_filter_control(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_nd_filter, "ND filter control")?;
    validate_static_typed_command(profile, row, "typed ND filter control")
}

fn validate_nd_filter_step(profile: &crate::ProfileSpec, row: BuiltinCommand) -> Result<(), Error> {
    validate_nd_filter_control(profile, row)?;
    if matches!(
        profile.capabilities().nd_filter_mode,
        crate::capabilities::NdFilterMode::Variable
    ) {
        Ok(())
    } else {
        // The step encoder is documented for the variable-ND VISCA
        // extension.  A generic stepped metadata fact is not evidence that
        // this vendor-specific wire command is valid for that profile.
        Err(Error::FeatureNotSupported {
            feature: "variable ND filter step movement",
        })
    }
}

fn validate_variable_nd_filter_control(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
) -> Result<(), Error> {
    validate_nd_filter_control(profile, row)?;
    if matches!(
        profile.capabilities().nd_filter_mode,
        crate::capabilities::NdFilterMode::Variable
    ) {
        Ok(())
    } else {
        Err(Error::FeatureNotSupported {
            feature: "variable ND filter control",
        })
    }
}

/// Gates the source-backed Sony FR7 red/green tally family. PTZOptics packed
/// status, brightness, mode, and auto-adjust candidates deliberately remain
/// low-level wire types and are refused by every built-in profile.
fn validate_tally_state(profile: &crate::ProfileSpec, row: BuiltinCommand) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_tally, "tally control")?;
    validate_static_typed_command(profile, row, "typed tally control")
}

/// Validates a command gate derived from the static noun-table row.
///
/// The dynamic facade has no marker bounds, so it must consult the same typed
/// support surface that its static method's `where Has*` clause projects to.
/// Only rows with a typed surface may call this helper.
fn validate_static_typed_command(
    profile: &crate::ProfileSpec,
    command: BuiltinCommand,
    feature: &'static str,
) -> Result<(), Error> {
    let Some(surface) = typed_surface_for_command(command) else {
        return Err(Error::InvalidRequest(
            "static typed-command gate is missing from the noun surface".into(),
        ));
    };
    require(profile.capabilities().permits_typed(surface), feature)
}

/// USB audio is a model capability, not a family-wide PTZOptics assumption.
///
/// Keep runtime validation on the same registry facts that emit the static
/// `HasUsbAudio` marker. In particular, a G3 profile remains denied until its
/// UAC command and response support are independently evidenced.
fn validate_usb_audio_state(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_usb_audio, "USB audio control")?;
    validate_static_typed_command(profile, row, "typed USB audio control")
}

fn validate_power_state(profile: &crate::ProfileSpec, standby: bool) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_power, "power control")?;
    if standby {
        require(capabilities.supports_standby, "standby power control")?;
    }
    Ok(())
}

fn validate_exposure_mode(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    mode: crate::command::ExposureMode,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_exposure, "exposure control")?;
    require(
        capabilities.supports_exposure_mode(mode),
        crate::command::exposure::SHARED_EXPOSURE_MODE_FEATURE,
    )?;
    validate_static_typed_command(
        profile,
        row,
        crate::command::exposure::SHARED_EXPOSURE_MODE_FEATURE,
    )
}

fn validate_exposure_compensation(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    value: Option<i8>,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_exposure, "exposure control")?;
    validate_static_typed_command(profile, row, "exposure compensation")?;
    if let Some(value) = value {
        let range =
            capabilities
                .exposure_comp_range
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "exposure compensation range",
                })?;
        require_in_range("exposure compensation", value, range)?;
    }
    Ok(())
}

fn validate_numeric_exposure(profile: &crate::ProfileSpec) -> Result<(), Error> {
    require(profile.capabilities().has_exposure, "exposure control")
}

fn validate_shutter(profile: &crate::ProfileSpec, value: Option<u8>) -> Result<(), Error> {
    validate_numeric_exposure(profile)?;
    if let Some(value) = value {
        if !profile
            .capabilities()
            .shutter_speeds
            .iter()
            .any(|speed| speed.value == value)
        {
            // The shutter table is a set of codes, not a range.
            return Err(Error::invalid_parameter(
                "shutter speed",
                Cow::Owned(value.to_string()),
                Cow::Borrowed("code is not in the profile's shutter table"),
            ));
        }
    }
    Ok(())
}

fn validate_brightness(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    value: Option<u8>,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_exposure, "exposure control")?;
    validate_static_typed_command(profile, row, "brightness control")?;
    if let Some(value) = value {
        let range =
            capabilities
                .exposure_brightness_range
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "brightness range",
                })?;
        require_in_domain("brightness level", value, range)?;
    }
    Ok(())
}

fn validate_gain(profile: &crate::ProfileSpec, value: Option<u8>) -> Result<(), Error> {
    validate_numeric_exposure(profile)?;
    if let Some(value) = value {
        require_in_range("gain level", value, &profile.capabilities().gain_range)?;
    }
    Ok(())
}

/// Checks the white-balance domain and the profile's mode list, then, for a
/// mode whose ledger row has one, that row's typed capability gate.
fn validate_white_balance_mode(
    profile: &crate::ProfileSpec,
    command: &crate::command::WhiteBalanceCommand,
) -> Result<(), Error> {
    use crate::command::WhiteBalanceMode;

    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    require(
        capabilities.white_balance_modes.contains(&command.mode),
        "selected white-balance mode",
    )?;
    let feature = match command.mode {
        WhiteBalanceMode::OnePush => "one-push white balance",
        WhiteBalanceMode::ATW => "auto-tracking white balance",
        WhiteBalanceMode::ColorTemperature => "color-temperature white balance",
        // These modes select ledger rows whose noun rows carry no `Has*`
        // marker, so they have no typed gate: the profile's mode list above is
        // their whole check. The build fails if one of those rows gains a
        // typed surface.
        WhiteBalanceMode::Auto
        | WhiteBalanceMode::Indoor
        | WhiteBalanceMode::Outdoor
        | WhiteBalanceMode::Manual => {
            const {
                assert!(
                    !white_balance_mode_is_typed(WhiteBalanceMode::Auto)
                        && !white_balance_mode_is_typed(WhiteBalanceMode::Indoor)
                        && !white_balance_mode_is_typed(WhiteBalanceMode::Outdoor)
                        && !white_balance_mode_is_typed(WhiteBalanceMode::Manual)
                );
            }
            return Ok(());
        }
    };
    validate_static_typed_command(profile, command.ledger_row(), feature)
}

/// Whether the ledger row a white-balance mode selects has a typed gate.
const fn white_balance_mode_is_typed(mode: crate::command::WhiteBalanceMode) -> bool {
    typed_surface_for_command(crate::command::WhiteBalanceCommand { mode }.ledger_row()).is_some()
}

fn validate_awb_sensitivity(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    validate_static_typed_command(profile, row, "auto white-balance sensitivity")
}

fn validate_tuning(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    value: i8,
    red: bool,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    validate_static_typed_command(profile, row, "RGB tuning")?;
    let range = if red {
        capabilities.rg_tuning_range.as_ref()
    } else {
        capabilities.bg_tuning_range.as_ref()
    }
    .ok_or(Error::FeatureNotSupported {
        feature: "RGB tuning range",
    })?;
    require_in_range("RGB tuning", value, range)?;
    Ok(())
}

fn validate_color_temperature(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    value: Option<crate::types::ColorTemp>,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    validate_static_typed_command(profile, row, "color temperature control")?;
    if let Some(value) = value {
        let range = capabilities
            .color_temp_range
            .as_ref()
            .ok_or(Error::FeatureNotSupported {
                feature: "color-temperature range",
            })?;
        // Profile metadata is expressed in Kelvin.
        let kelvin = value.to_kelvin();
        require_in_range("color temperature", kelvin, range)?;
    }
    Ok(())
}

fn validate_rgb_gain(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    value: Option<u8>,
    red: bool,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_white_balance, "white balance control")?;
    validate_static_typed_command(profile, row, "RGB gain control")?;
    if let Some(value) = value {
        let range = if red {
            capabilities.red_gain_range.as_ref()
        } else {
            capabilities.blue_gain_range.as_ref()
        }
        .ok_or(Error::FeatureNotSupported {
            feature: "RGB gain range",
        })?;
        require_in_range("RGB gain", value, range)?;
    }
    Ok(())
}

fn validate_image_control(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    feature: &'static str,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_image_processing, "image processing")?;
    validate_static_typed_command(profile, row, feature)
}

fn validate_image_range(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    feature: &'static str,
    range: Option<&std::ops::RangeInclusive<u8>>,
    value: u8,
) -> Result<(), Error> {
    validate_image_control(profile, row, feature)?;
    let range = range.ok_or(Error::FeatureNotSupported { feature })?;
    require_in_range(feature, value, range)?;
    Ok(())
}

fn validate_flip_mode(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    mode: crate::command::ImageFlipMode,
) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_image_processing, "image processing")?;
    validate_static_typed_command(profile, row, "combined image flip")?;
    match mode {
        crate::command::ImageFlipMode::Off | crate::command::ImageFlipMode::Vertical => {
            require(capabilities.supports_flip, "vertical image flip")
        }
        crate::command::ImageFlipMode::Horizontal => {
            require(capabilities.supports_mirror, "horizontal image mirror")
        }
        crate::command::ImageFlipMode::Both => {
            require(capabilities.supports_flip, "vertical image flip")?;
            require(capabilities.supports_mirror, "horizontal image mirror")
        }
    }
}

fn validate_separate_flip(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    feature: &'static str,
) -> Result<(), Error> {
    require(
        profile.capabilities().has_image_processing,
        "image processing",
    )?;
    validate_static_typed_command(profile, row, feature)
}

/// The profile's motion-sync speed range, which is also its only motion-sync
/// fact: `None` means the camera has no motion sync.
fn motion_sync_speed_range(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
) -> Result<&std::ops::RangeInclusive<u8>, Error> {
    let capabilities = profile.capabilities();
    let range =
        capabilities
            .motion_sync_speed_range
            .as_ref()
            .ok_or(Error::FeatureNotSupported {
                feature: "motion sync",
            })?;
    validate_static_typed_command(profile, row, "typed motion sync")?;
    Ok(range)
}

fn validate_motion_sync_speed(
    profile: &crate::ProfileSpec,
    row: BuiltinCommand,
    speed: crate::types::MotionSyncSpeed,
) -> Result<(), Error> {
    require_in_range(
        "motion sync speed",
        speed.value(),
        motion_sync_speed_range(profile, row)?,
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
        validate_exposure_mode(profile, Self::LEDGER_ROW, self.mode)
    }
}

impl BuiltinValidation for crate::command::exposure::ExposureCompensation {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let value = match self {
            Self::SetLevel(level) => Some(level.value()),
            _ => None,
        };
        validate_exposure_compensation(profile, Self::LEDGER_ROW, value)
    }
}

impl BuiltinValidation for crate::command::exposure::DynamicRange {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(profile.capabilities().has_exposure, "exposure control")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "wide dynamic range")
    }
}

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
            Self::LEDGER_ROW,
            match self {
                Self::SetLevel(level) => Some(level.value()),
                _ => None,
            },
        )
    }
}

impl BuiltinValidation for crate::command::exposure::AntiFlickerCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(profile, Self::LEDGER_ROW, "anti-flicker control")
    }
}

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

impl BuiltinValidation for crate::command::color::OnePushTriggerCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_white_balance, "white balance control")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "one-push white balance")
    }
}

impl BuiltinValidation for crate::command::color::RedTuningCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tuning(profile, Self::LEDGER_ROW, self.level.value(), true)
    }
}

impl BuiltinValidation for crate::command::color::BlueTuningCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tuning(profile, Self::LEDGER_ROW, self.level.value(), false)
    }
}

impl BuiltinValidation for crate::command::color::SaturationCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            Self::LEDGER_ROW,
            "saturation control",
            profile.capabilities().saturation_range.as_ref(),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::color::HueCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            Self::LEDGER_ROW,
            "hue control",
            profile.capabilities().hue_range.as_ref(),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::color::ColorTemperature {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_color_temperature(
            profile,
            Self::LEDGER_ROW,
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
            Self::LEDGER_ROW,
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
            Self::LEDGER_ROW,
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
                profile,
                Self::LEDGER_ROW,
                "sharpness control",
                profile.capabilities().sharpness_range.as_ref(),
                value.value(),
            ),
            _ => validate_image_control(profile, Self::LEDGER_ROW, "sharpness control"),
        }
    }
}

impl BuiltinValidation for crate::command::image::Luminance {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            Self::LEDGER_ROW,
            "luminance control",
            profile.capabilities().luminance_range.as_ref(),
            self.value.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image::Contrast {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            Self::LEDGER_ROW,
            "contrast control",
            profile.capabilities().contrast_range.as_ref(),
            self.value.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image::GammaCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_range(
            profile,
            Self::LEDGER_ROW,
            "gamma control",
            profile.capabilities().gamma_range.as_ref(),
            self.level.value(),
        )
    }
}

impl BuiltinValidation for crate::command::image::BacklightCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        validate_image_control(profile, Self::LEDGER_ROW, "backlight compensation")?;
        require(capabilities.has_exposure, "exposure control")?;
        require(capabilities.has_backlight_comp, "backlight compensation")
    }
}

impl BuiltinValidation for crate::command::image::NoiseReduction2DModeCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_2d_nr, "2D noise reduction")?;
        validate_image_control(profile, Self::LEDGER_ROW, "2D noise reduction mode")
    }
}

impl BuiltinValidation for crate::command::image::NoiseReduction2D {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_2d_nr, "2D noise reduction")?;
        validate_image_control(profile, Self::LEDGER_ROW, "2D noise reduction control")
    }
}

impl BuiltinValidation for crate::command::image::NoiseReduction3D {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_3d_nr, "3D noise reduction")?;
        validate_image_control(profile, Self::LEDGER_ROW, "3D noise reduction control")
    }
}

impl BuiltinValidation for crate::command::image::ImageFlipCombinedCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_flip_mode(profile, Self::LEDGER_ROW, self.mode)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // The combined opcode carries both axes in one parameter byte, so a
        // successful application establishes the complete pair.
        let crate::command::FlipState {
            horizontal,
            vertical,
        } = self.mode.into();
        state_projection::<Self>(&[i64::from(horizontal), i64::from(vertical)])
    }
}

impl BuiltinValidation for crate::command::image::PictureEffectCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_image_control(profile, Self::LEDGER_ROW, "picture effect")
    }
}

impl BuiltinValidation for crate::command::focus::FocusZoneCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "focus zone")?;
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

impl BuiltinValidation for crate::command::focus::AutoFocusSensitivityCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "auto-focus sensitivity")
    }
}

impl BuiltinValidation for crate::command::focus::FocusNearLimitCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "focus near limit")?;
        let value = self.position.value();
        require_in_range("focus near limit", value, &capabilities.focus_range)
    }
}

impl BuiltinValidation for crate::command::white_balance::WhiteBalanceCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_white_balance_mode(profile, self)
    }
}

impl BuiltinValidation for crate::command::white_balance::AWBSensitivityCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_awb_sensitivity(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for crate::command::menu::SetMenuDisplay {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for crate::command::menu::MenuNavigate {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for crate::command::menu::PerformMenuAction {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for crate::command::menu::DirectMenuControl {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(profile, Self::LEDGER_ROW, "direct menu control")
    }
}

impl BuiltinValidation for crate::command::streaming::UsbAudio {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_usb_audio_state(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for crate::command::system::SettingsSaveCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(profile, Self::LEDGER_ROW, "settings save")
    }
}

impl BuiltinValidation for crate::command::tally::TallyRedOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for crate::command::tally::TallyRedOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for crate::command::tally::TallyGreenOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for crate::command::tally::TallyGreenOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_tally_state(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for crate::command::motion_sync::SetMotionSyncMode {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        motion_sync_speed_range(profile, Self::LEDGER_ROW).map(|_| ())
    }
}

impl BuiltinValidation for crate::command::motion_sync::SetMotionSyncPreset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_motion_sync_speed(profile, Self::LEDGER_ROW, self.speed())
    }
}

impl BuiltinValidation for crate::command::system::AddressSetCommand {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for crate::command::system::InterfaceClearCommand {
    fn validate(&self, _profile: &crate::ProfileSpec) -> Result<(), Error> {
        Ok(())
    }
}

impl BuiltinValidation for crate::command::system::CommandCancelCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(
            profile.supports_command_cancel(),
            "VISCA socket cancellation",
        )
    }
}

impl BuiltinValidation for ImageFlipCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_separate_flip(profile, Self::LEDGER_ROW, "vertical image flip")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // This opcode moves only the vertical axis. The horizontal axis keeps
        // whatever value it had, which this request does not know, so the
        // complete pair stops being known.
        state_projection::<Self>(&[])
    }
}

impl BuiltinValidation for ImageMirrorCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_separate_flip(profile, Self::LEDGER_ROW, "horizontal image mirror")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // The mirror opcode is the horizontal twin of `ImageFlipCommand` and
        // leaves the vertical axis unknown for the same reason.
        state_projection::<Self>(&[])
    }
}

fn state_projection<T: BuiltinRequestContract>(
    values: &[i64],
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    use crate::command::semantics::AppliedStateEffectRequirement;

    match const { crate::command::semantics::state_effect::<T>() } {
        AppliedStateEffectRequirement::Set(state) => {
            crate::runtime::engine::AppliedStateProjection::set(state, values).ok()
        }
        AppliedStateEffectRequirement::Clear(state) => {
            crate::runtime::engine::AppliedStateProjection::clear_with_values(state, values).ok()
        }
        AppliedStateEffectRequirement::Invalidate(state) => Some(
            crate::runtime::engine::AppliedStateProjection::invalidate(state),
        ),
    }
}

fn bool_projection<T: BuiltinRequestContract>(
    enabled: bool,
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    state_projection::<T>(&[i64::from(enabled)])
}

fn scalar_projection<T: BuiltinRequestContract>(
    value: i64,
) -> Option<crate::runtime::engine::AppliedStateProjection> {
    state_projection::<T>(&[value])
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreparedPanTiltPosition {
    pan: i32,
    tilt: i32,
    conversion: PanTiltCoordinateConversion,
}

impl PreparedPanTiltPosition {
    fn for_profile(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        let conversion = profile
            .pan_tilt_coordinates()
            .ok_or(Error::FeatureNotSupported {
                feature: "pan/tilt coordinate conversion",
            })?;
        let (pan, tilt) = profile.convert_pan_tilt_degrees(pan.0, tilt.0)?;
        Ok(Self {
            pan,
            tilt,
            conversion,
        })
    }

    fn validate(self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_position(profile, self.pan, self.tilt, self.conversion)
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
    crate::command::nd_filter::AutoNdCommand => NdFilterAutoOn: Plain { size: 7 };
    crate::command::flip::ImageFreeze => ImageFreezeOn: Plain { size: 6 };
    crate::command::zoom::DigitalZoom => DigitalZoom: Plain { size: 6 };
    crate::command::streaming::MulticastStreaming => MulticastStreamingOn: Plain {
        size: 6,
        policy: (Network, Standard, Normal),
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
    crate::command::exposure::ExposureCompensation => ExposureCompensationOn: Plain { size: 9 };
    crate::command::exposure::DynamicRange => DynamicRange: Plain { size: 9 };
    crate::command::exposure::Shutter => ShutterReset: Plain { size: 9 };
    crate::command::exposure::Brightness => BrightnessReset: Plain { size: 9 };
    crate::command::exposure::AntiFlickerCommand => AntiFlicker: Plain { size: 6 };
    crate::command::gain::Gain => GainReset: Plain { size: 9 };
    crate::command::gain::GainLimitCommand => GainLimit: Plain { size: 6 };
    crate::command::color::OnePushTriggerCommand => OnePushWhiteBalanceTrigger: Plain { size: 6 };
    crate::command::color::RedTuningCommand => RedTuning: Plain { size: 9 };
    crate::command::color::BlueTuningCommand => BlueTuning: Plain { size: 9 };
    crate::command::color::SaturationCommand => Saturation: Plain { size: 9 };
    crate::command::color::HueCommand => Hue: Plain { size: 9 };
    crate::command::color::ColorTemperature => ColorTemperatureReset: Plain { size: 7 };
    crate::command::color::RedGain => RedGainReset: Plain { size: 9 };
    crate::command::color::BlueGain => BlueGainReset: Plain { size: 9 };
    crate::command::image::Sharpness => SharpnessMode: Plain { size: 9 };
    crate::command::image::Luminance => Luminance: Plain { size: 9 };
    crate::command::image::Contrast => Contrast: Plain { size: 9 };
    crate::command::image::GammaCommand => Gamma: Plain { size: 6 };
    crate::command::image::BacklightCommand => Backlight: Plain { size: 6 };
    crate::command::image::NoiseReduction2DModeCommand => NoiseReduction2dMode: Plain { size: 6 };
    crate::command::image::NoiseReduction2D => NoiseReduction2d: Plain { size: 6 };
    crate::command::image::NoiseReduction3D => NoiseReduction3d: Plain { size: 6 };
    crate::command::image::ImageFlipCombinedCommand => ImageFlipBoth: Plain { size: 6 };
    crate::command::image::PictureEffectCommand => PictureEffect: Plain { size: 6 };
    crate::command::focus::FocusZoneCommand => FocusZone: Plain { size: 6 };
    crate::command::focus::AutoFocusSensitivityCommand => FocusAutoSensitivity: Plain { size: 6 };
    crate::command::focus::FocusNearLimitCommand => FocusNearLimit: Plain { size: 9 };
    crate::command::white_balance::WhiteBalanceCommand => WhiteBalanceAuto: Plain {
        size: 6,
        rows: |command| match command.mode {
            crate::command::WhiteBalanceMode::Auto => WhiteBalanceAuto,
            crate::command::WhiteBalanceMode::Indoor => WhiteBalanceIndoor,
            crate::command::WhiteBalanceMode::Outdoor => WhiteBalanceOutdoor,
            crate::command::WhiteBalanceMode::OnePush => WhiteBalanceOnePush,
            crate::command::WhiteBalanceMode::ATW => WhiteBalanceAutoTracking,
            crate::command::WhiteBalanceMode::Manual => WhiteBalanceManual,
            crate::command::WhiteBalanceMode::ColorTemperature => WhiteBalanceColorTemperature,
        },
    };
    crate::command::white_balance::AWBSensitivityCommand => AutoWhiteBalanceSensitivity: Plain {
        size: 6,
    };
    crate::command::menu::SetMenuDisplay => MenuDisplay: Plain { size: 6 };
    crate::command::menu::MenuNavigate => MenuNavigate: Plain { size: 9 };
    crate::command::menu::PerformMenuAction => MenuSelect: Plain { size: 6 };
    crate::command::menu::DirectMenuControl => DirectMenu: Plain { size: 8 };
    crate::command::streaming::UsbAudio => UsbAudioOn: Plain { size: 7 };
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
    };
}

/// One semantic row tied to a concrete typed request implementation.
///
/// Each inventory entry is an inline const that asserts, while the `#[used]`
/// inventory is initialized, that its request type may serve its row. The
/// test-only metadata then gives the runtime audit a readable row.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BuiltinTypedRequestCoverage {
    /// Authoritative semantic ledger row.
    #[cfg(test)]
    pub row: BuiltinCommand,
    /// Stable source type name used by diagnostics and audits.
    #[cfg(test)]
    pub type_name: &'static str,
    /// Command branch/value family represented by the typed request.
    #[cfg(test)]
    pub branch: &'static str,
}

/// The one construction site of [`BuiltinTypedRequestCoverage`].
///
/// The inline const asserts at compile time that `$ty` serves `$row`. A
/// `by_value` request is a constant, so its own `ledger_row` must select
/// `$row`; any other request form is checked on its type alone.
macro_rules! typed_request_coverage {
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
                #[cfg(test)]
                row: BuiltinCommand::$row,
                #[cfg(test)]
                type_name: stringify!($ty),
                #[cfg(test)]
                branch: $branch,
            }
        }
    };
}

/// Fails const evaluation unless `T` may serve `row` through a request form
/// that is not a constant: the row must classify exactly like `T`'s own
/// ledger row (class, axes and state effect) and carry the same typed
/// capability gate, and `T` must not select its row by value.
const fn assert_row_served_by<T: BuiltinRequestContract>(row: BuiltinCommand) {
    assert!(
        row.classification()
            .same_contract(T::LEDGER_ROW.classification()),
        "a typed request serves a ledger row whose classification differs from its own row",
    );
    assert!(
        !T::SELECTS_ROW_BY_VALUE,
        "a typed request that selects its ledger row by value is sent by a noun row \
         that is not a `by_value` row",
    );
    assert!(
        same_typed_gate(
            typed_surface_for_command(row),
            typed_surface_for_command(T::LEDGER_ROW),
        ),
        "a typed request serves ledger rows with different typed capability gates",
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

const fn same_typed_gate(
    left: Option<TypedSupportSurface>,
    right: Option<TypedSupportSurface>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => left as u8 == right as u8,
        (None, Some(_)) | (Some(_), None) => false,
    }
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
            use crate::{command, request::builtin};
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
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

impl PanTiltAbsolute {
    /// Creates an absolute target request.
    pub fn for_profile(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        validate_pan_tilt_position_speed(profile, pan_speed, tilt_speed)?;
        Ok(Self {
            position: PreparedPanTiltPosition::for_profile(pan, tilt, profile)?,
            pan_speed,
            tilt_speed,
        })
    }

    /// Creates an absolute target from one coarse speed through the profile's
    /// position-command grammar.
    pub(crate) fn for_profile_speed_level(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        let (pan_speed, tilt_speed) = pan_tilt_position_speeds_from_level(speed, profile)?;
        Self::for_profile(pan, tilt, pan_speed, tilt_speed, profile)
    }
}

builtin_request! {
    PanTiltAbsolute => PanTiltAbsolute: Targeted {
        size: 16,
        wire: |value: &PanTiltAbsolute| PanTiltProfiled::AbsolutePosition {
            framing: PanTiltFraming::for_conversion(value.position.conversion),
            pan: value.position.pan,
            tilt: value.position.tilt,
            pan_speed: value.pan_speed,
            tilt_speed: value.tilt_speed,
        },
    };
}

/// Relative pan/tilt offset converted from degrees through one validated profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanTiltRelative {
    position: PreparedPanTiltPosition,
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

impl PanTiltRelative {
    /// Creates a relative target request.
    pub fn for_profile(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        validate_pan_tilt_position_speed(profile, pan_speed, tilt_speed)?;
        Ok(Self {
            position: PreparedPanTiltPosition::for_profile(pan, tilt, profile)?,
            pan_speed,
            tilt_speed,
        })
    }

    /// Creates a relative target from one coarse speed through the profile's
    /// position-command grammar.
    pub(crate) fn for_profile_speed_level(
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        let (pan_speed, tilt_speed) = pan_tilt_position_speeds_from_level(speed, profile)?;
        Self::for_profile(pan, tilt, pan_speed, tilt_speed, profile)
    }
}

builtin_request! {
    PanTiltRelative => PanTiltRelative: Targeted {
        size: 16,
        wire: |value: &PanTiltRelative| PanTiltProfiled::RelativePosition {
            framing: PanTiltFraming::for_conversion(value.position.conversion),
            pan: value.position.pan,
            tilt: value.position.tilt,
            pan_speed: value.pan_speed,
            tilt_speed: value.tilt_speed,
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
    pub fn for_profile(
        corner: PanTiltLimitCorner,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        profile: &crate::ProfileSpec,
    ) -> Result<Self, Error> {
        Ok(Self {
            corner,
            position: PreparedPanTiltPosition::for_profile(pan, tilt, profile)?,
        })
    }
}

builtin_request! {
    PanTiltLimitSet => PanTiltLimitSet: Plain {
        size: 16,
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
        validate_pan_tilt(profile)?;
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
        size: 16,
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
        let capabilities = profile.capabilities();
        let optical_max = *capabilities.zoom_range_optical.end();
        let digital_max = capabilities
            .zoom_range_digital
            .as_ref()
            .map(|range| *range.end());
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
        rows: |trigger| match trigger {
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
        size: 9,
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

impl BuiltinValidation for PanTiltHome {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt(profile)
    }
}

impl BuiltinValidation for PanTiltReset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt(profile)
    }
}

impl BuiltinValidation for PanTiltDrive {
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
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_position_speed(profile, self.pan_speed, self.tilt_speed)?;
        self.position.validate(profile)
    }
}

impl BuiltinValidation for PanTiltRelative {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt_position_speed(profile, self.pan_speed, self.tilt_speed)?;
        self.position.validate(profile)
    }
}

impl BuiltinValidation for PanTiltLimitSet {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        self.position.validate(profile)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        // Keep the corner and converted position in the bounded value so a
        // later owner/cache layer can merge corner-local limit updates without
        // consulting the request or the camera facade.
        state_projection::<Self>(&[
            i64::from(self.corner.to_byte()),
            i64::from(self.position.pan),
            i64::from(self.position.tilt),
        ])
    }
}

impl BuiltinValidation for PanTiltLimitClear {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_pan_tilt(profile)?;
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

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        state_projection::<Self>(&[i64::from(self.corner.to_byte())])
    }
}

impl BuiltinValidation for ZoomTarget {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_zoom, "zoom control")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "direct zoom positioning")?;
        if matches!(self.1, Some(crate::ZoomDomain::OpticalPlusDigital)) {
            require(
                capabilities.permits_typed(TypedSupportSurface::DigitalZoomRange),
                "optical-plus-digital zoom positioning",
            )?;
        }
        // A validated profile's digital range starts where the optical range
        // ends, so the accepted positions are one contiguous range.
        let optical = &capabilities.zoom_range_optical;
        let accepted = match capabilities.zoom_range_digital.as_ref() {
            Some(digital) if capabilities.permits_typed(TypedSupportSurface::DigitalZoomRange) => {
                *optical.start()..=*digital.end()
            }
            _ => optical.clone(),
        };
        require_in_range("zoom position", self.0.value(), &accepted)
    }
}

impl BuiltinValidation for ZoomDrive {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_zoom, "zoom control")?;
        let speed = match self {
            Self::Tele | Self::Wide => return Ok(()),
            Self::TeleVariable(speed) | Self::WideVariable(speed) => speed.value(),
        };
        require(capabilities.supports_variable_zoom, "variable zoom drive")?;
        require_in_range("zoom speed", speed, &capabilities.zoom_speed)
    }
}

impl BuiltinValidation for ZoomStop {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(profile.capabilities().has_zoom, "zoom control")
    }
}

impl BuiltinValidation for FocusTarget {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        require_in_range("focus position", self.0.value(), &capabilities.focus_range)
    }
}

impl BuiltinValidation for FocusInfinity {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(profile.capabilities().has_focus, "focus control")
    }
}

impl BuiltinValidation for FocusDrive {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        let speed = match self {
            Self::Far | Self::Near => return Ok(()),
            Self::FarVariable(speed) | Self::NearVariable(speed) => speed.value(),
        };
        require_in_range("focus speed", speed, &capabilities.focus_speed)
    }
}

impl BuiltinValidation for FocusStop {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        require(profile.capabilities().has_focus, "focus control")
    }
}

impl BuiltinValidation for FocusTrigger {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        let feature = match self {
            Self::OnePush => "one-push focus",
            Self::Snap => "snap focus",
        };
        validate_static_typed_command(profile, self.ledger_row(), feature)
    }
}

impl BuiltinValidation for FocusModeCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        if matches!(self, Self::Auto | Self::Toggle) {
            require(capabilities.has_auto_focus, "automatic focus")?;
        }
        Ok(())
    }
}

impl BuiltinValidation for IrisReset {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_iris_control(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for IrisUp {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_iris_control(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for IrisDown {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_iris_control(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for IrisDirect {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_iris_control(profile, Self::LEDGER_ROW)?;
        let range =
            profile
                .capabilities()
                .iris_range
                .as_ref()
                .ok_or(Error::FeatureNotSupported {
                    feature: "iris range",
                })?;
        require_in_domain("iris level", u16::from(self.0.value()), range)
    }
}

impl BuiltinValidation for NdFilterDirect {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_nd_filter_control(profile, Self::LEDGER_ROW)?;
        if !matches!(
            profile.capabilities().nd_filter_mode,
            crate::capabilities::NdFilterMode::Variable
        ) {
            return Err(Error::FeatureNotSupported {
                feature: "direct ND filter positioning",
            });
        }
        // NdFilterValue validates the protocol's exact variable-ND range at
        // construction.  Keep the value extraction here explicit so profile
        // admission cannot silently turn a future wider value into a
        // valid typed request without a corresponding profile fact.
        require_in_range(
            "ND filter value",
            self.0.value(),
            &(0..=NdFilterValue::MAX_VALUE),
        )
    }
}

impl BuiltinValidation for NdFilterStepUp {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_nd_filter_step(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for NdFilterStepDown {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_nd_filter_step(profile, Self::LEDGER_ROW)
    }
}

impl BuiltinValidation for PushAfPress {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "push autofocus")
    }
}

impl BuiltinValidation for PushAfRelease {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus control")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "push autofocus")
    }
}

fn validate_preset(profile: &crate::ProfileSpec, preset: PresetNumber) -> Result<(), Error> {
    let capabilities = profile.capabilities();
    require(capabilities.has_presets, "preset control")?;
    require_in_range(
        "preset number",
        preset.value(),
        &(0..=capabilities.highest_preset),
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
        validate_static_typed_command(profile, Self::LEDGER_ROW, "preset recall speed")?;
        let capabilities = profile.capabilities();
        require(capabilities.has_presets, "preset control")?;
        let speed = self.speed.value();
        let range = capabilities
            .preset_speed_range
            .as_ref()
            .ok_or(Error::FeatureNotSupported {
                feature: "preset recall speed range",
            })?;
        require_in_range("preset recall speed", speed, range)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        scalar_projection::<Self>(i64::from(self.speed.value()))
    }
}

impl BuiltinValidation for crate::command::focus::FocusLock {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_focus, "focus lock")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "typed focus lock")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(matches!(self, Self::On))
    }
}

impl BuiltinValidation for crate::command::exposure::SpotlightOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(profile, Self::LEDGER_ROW, "spotlight control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(true)
    }
}

impl BuiltinValidation for crate::command::exposure::SpotlightOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(profile, Self::LEDGER_ROW, "spotlight control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(false)
    }
}

impl BuiltinValidation for crate::command::exposure::AutoSlowShutterOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(profile, Self::LEDGER_ROW, "auto slow shutter control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(true)
    }
}

impl BuiltinValidation for crate::command::exposure::AutoSlowShutterOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(profile, Self::LEDGER_ROW, "auto slow shutter control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(false)
    }
}

impl BuiltinValidation for crate::command::nd_filter::NdFilterModeCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_variable_nd_filter_control(profile, Self::LEDGER_ROW)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        scalar_projection::<Self>(match self.mode() {
            NdFilterMode::Preset => 0,
            NdFilterMode::Variable => 1,
        })
    }
}

impl BuiltinValidation for crate::command::nd_filter::AutoNdCommand {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_variable_nd_filter_control(profile, Self::LEDGER_ROW)
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(self.enabled())
    }
}

impl BuiltinValidation for crate::command::flip::ImageFreeze {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let command = if self.enabled() {
            BuiltinCommand::ImageFreezeOn
        } else {
            BuiltinCommand::ImageFreezeOff
        };
        validate_static_typed_command(profile, command, "validated image-freeze command")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(self.enabled())
    }
}

impl BuiltinValidation for crate::command::zoom::DigitalZoom {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_zoom, "zoom control")?;
        require(capabilities.zoom_range_digital.is_some(), "digital zoom")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "typed digital zoom toggle")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(self.enabled())
    }
}

impl BuiltinValidation for crate::command::streaming::MulticastStreaming {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let command = match self {
            Self::On => BuiltinCommand::MulticastStreamingOn,
            Self::Off => BuiltinCommand::MulticastStreamingOff,
        };
        validate_static_typed_command(profile, command, "multicast streaming")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(matches!(self, Self::On))
    }
}

impl BuiltinValidation for crate::command::streaming::SetNdiQuality {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(profile, Self::LEDGER_ROW, "NDI quality control")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        let value = match self.quality {
            crate::types::NdiQuality::High => 1,
            crate::types::NdiQuality::Medium => 2,
            crate::types::NdiQuality::Low => 3,
            crate::types::NdiQuality::Off => 4,
        };
        scalar_projection::<Self>(value)
    }
}

impl BuiltinValidation for crate::command::tally::TallyBrightLo {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(
            profile,
            Self::LEDGER_ROW,
            "validated tally-brightness command",
        )
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        scalar_projection::<Self>(0)
    }
}

impl BuiltinValidation for crate::command::tally::TallyBrightHi {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(
            profile,
            Self::LEDGER_ROW,
            "validated tally-brightness command",
        )
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        scalar_projection::<Self>(1)
    }
}

impl BuiltinValidation for crate::command::variable_speed::SetVariableSpeedMode {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        let capabilities = profile.capabilities();
        require(capabilities.has_pan_tilt, "pan/tilt control")?;
        require(capabilities.has_variable_speed, "variable speed mode")?;
        validate_static_typed_command(profile, Self::LEDGER_ROW, "typed variable speed mode")
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        let value = match self.mode {
            crate::command::VariableSpeedMode::Standard24 => 1,
            crate::command::VariableSpeedMode::Fine50 => 2,
        };
        scalar_projection::<Self>(value)
    }
}

impl BuiltinValidation for crate::command::tally::TallyOn {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(
            profile,
            Self::LEDGER_ROW,
            "validated PTZOptics tally-mode command",
        )
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(true)
    }
}

impl BuiltinValidation for crate::command::tally::TallyOff {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(
            profile,
            Self::LEDGER_ROW,
            "validated PTZOptics tally-mode command",
        )
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        bool_projection::<Self>(false)
    }
}

impl BuiltinValidation for crate::command::tally::TallyFlash {
    fn validate(&self, profile: &crate::ProfileSpec) -> Result<(), Error> {
        validate_static_typed_command(
            profile,
            Self::LEDGER_ROW,
            "validated PTZOptics tally-mode command",
        )
    }

    fn applied_state(&self) -> Option<crate::runtime::engine::AppliedStateProjection> {
        state_projection::<Self>(&[])
    }
}

#[cfg(test)]
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
            prepare_builtin_command, prepare_builtin_operation, prepare_command, ClassSelection,
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

        let gain = capabilities.gain_range.clone();
        let above_gain = *gain.end() + 1;
        assert_profile_range_error(
            validate_gain(&g2, Some(above_gain)),
            "gain level",
            i32::from(above_gain),
            (i32::from(*gain.start()), i32::from(*gain.end())),
        );

        let above_presets = capabilities.highest_preset + 1;
        assert_profile_range_error(
            validate_preset(&g2, PresetNumber::new(above_presets).expect("preset")),
            "preset number",
            i32::from(above_presets),
            (0, i32::from(capabilities.highest_preset)),
        );

        // The G2 tilt speed range is narrower than the syntactic `TiltSpeed`
        // domain, so the profile check is the one that rejects.
        let tilt = capabilities.tilt_speed.clone();
        let above_tilt = *tilt.end() + 1;
        assert_profile_range_error(
            validate_pan_tilt_speed(
                &g2,
                PanSpeed::new(*capabilities.pan_speed.start()).expect("pan speed"),
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
                CameraId::CAMERA_1,
                profile,
                OperationalTuning::new(),
            )
            .is_ok());
        }

        for profile in [&g3, &fr7, &generic] {
            assert!(prepare_builtin_command(
                &crate::command::UsbAudio::On,
                CameraId::CAMERA_1,
                profile,
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
                            CameraId::CAMERA_1,
                            profile,
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
        let coordinates = source
            .pan_tilt_coordinates()
            .expect("G2 pan/tilt conversion");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "Every white-balance mode".into();
        capabilities.white_balance_modes = MODES.to_vec();
        capabilities.has_one_push_wb = true;
        capabilities.color_temp_range = Some(2500..=8000);
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
                .pan_tilt_coordinates(
                    coordinates.coordinate_system(),
                    coordinates.pan_degrees_to_units(),
                    coordinates.tilt_degrees_to_units(),
                )
                .pan_tilt_wire_codec(coordinates.wire_codec())
                .transports(source.transports())
                .envelope(source.envelope())
                .timing(source.timing())
                .maximum_command_sockets(source.maximum_command_sockets())
                .supports_operation_complete(source.supports_operation_complete())
                .supports_command_cancel(source.supports_command_cancel())
                .preset_recall_axes(source.preset_recall_axes())
                .position_inquiries(source.position_inquiries())
                .build()
                .expect("runtime profile admitting every white-balance mode")
        };

        let full = profile(capabilities.typed_support);
        for mode in MODES {
            assert!(
                WhiteBalanceCommand::new(mode)
                    .validate_for_profile(&full)
                    .is_ok(),
                "{mode:?} must be admitted when every row gate is permitted"
            );
        }
        for (denied, surface, feature) in gated {
            let profile = profile(capabilities.typed_support.without(surface));
            for mode in MODES {
                let result = WhiteBalanceCommand::new(mode).validate_for_profile(&profile);
                if mode == denied {
                    assert!(
                        matches!(
                            result,
                            Err(Error::FeatureNotSupported { feature: reported })
                                if reported == feature
                        ),
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

        let standard_absolute = PanTiltAbsolute::for_profile_speed_level(
            Degrees(10.0),
            Degrees(-5.0),
            SpeedLevel::Medium,
            &standard,
        )
        .expect("standard coarse absolute position");
        assert_eq!(
            wire(&standard_absolute),
            vec![
                0x81, 0x01, 0x06, 0x02, 0x0C, 0x0A, 0x00, 0x00, 0x09, 0x00, 0x0F, 0x0F, 0x0B, 0x08,
                0xFF,
            ]
        );

        let brc300_absolute = PanTiltAbsolute::for_profile_speed_level(
            Degrees(45.0),
            Degrees(15.0),
            SpeedLevel::Fastest,
            &brc300,
        )
        .expect("BRC-300 coarse absolute position");
        assert_eq!(
            wire(&brc300_absolute),
            vec![
                0x81, 0x01, 0x06, 0x02, 0x18, 0x00, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
                0x00, 0xFF,
            ]
        );

        let brc300_relative = PanTiltRelative::for_profile_speed_level(
            Degrees(45.0),
            Degrees(15.0),
            SpeedLevel::Fastest,
            &brc300,
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
            PanTiltAbsolute::for_profile_speed_level(
                Degrees(0.0),
                Degrees(0.0),
                SpeedLevel::Fastest,
                profile,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{} must lower SpeedLevel::Fastest to a valid absolute request: {error}",
                    profile.capabilities().model_name
                )
            });
            PanTiltRelative::for_profile_speed_level(
                Degrees(0.0),
                Degrees(0.0),
                SpeedLevel::Fastest,
                profile,
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

    /// Exhaustive value-domain sweep for #683.
    ///
    /// Every valid preset number (0..=255) and every direct-menu control
    /// parameter (0..=255) must encode to a frame whose length equals the
    /// request's own `encoded_size()` and that ends in the terminator. The old
    /// const builder swallowed a trailing `0xFF` *data* byte as the terminator,
    /// so `write_into` reported one byte short of `encoded_size()` and
    /// `prepared::encode` rejected preset 255 and `direct(_, 0xFF)` outright.
    /// This test would catch any reintroduction of that last-byte inference.
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
            assert_eq!(PresetSet::new(preset).encoded_size(), 7);
            assert_eq!(PresetReset::new(preset).encoded_size(), 7);

            // Targeted operation: PresetRecall (needs the profile for its axes).
            let recall = PresetRecall::for_profile(preset, &fr7).expect("FR7 preset recall");
            let recall_bytes = wire(&recall);
            assert_eq!(
                recall_bytes.len(),
                recall.encoded_size(),
                "PresetRecall {number} length must equal encoded_size"
            );
            assert_eq!(
                recall_bytes,
                vec![0x81, 0x01, 0x04, 0x3F, 0x02, number, 0xFF],
                "PresetRecall {number} wire bytes"
            );
        }

        // Direct menu control: sweep control2 over its whole domain, including
        // 0xFF, across a spread of control1 values.
        for control1 in [0x00u8, 0x7F, 0x80, 0xFF] {
            for control2 in 0..=u8::MAX {
                let command = DirectMenuControl::new(control1, control2);
                if control1 == 0xFF && (0x81..=0x88).contains(&control2) {
                    assert!(matches!(command, Err(Error::InvalidRequest(_))));
                    continue;
                }
                let command = command.expect("all remaining direct-menu controls are valid");
                let bytes = wire(&command);
                assert_eq!(
                    bytes.len(),
                    command.encoded_size(),
                    "direct menu {control1:#x},{control2:#x} length must equal encoded_size"
                );
                assert_eq!(
                    bytes,
                    vec![0x81, 0x01, 0x7E, 0x04, 0x72, control1, control2, 0xFF],
                    "direct menu {control1:#x},{control2:#x} wire bytes"
                );
            }
        }
    }

    /// End-to-end proof for #683 through the prepared (validate + encode) path
    /// on Sony FR7, the profile that both allows preset 255 and supports direct
    /// menu control. Before the fix these returned an `InvalidRequest`
    /// contract-string error from `encode` because `write_into` fell one byte
    /// short of `encoded_size()`.
    #[test]
    fn preset_255_and_direct_menu_ff_prepare_on_fr7() {
        use crate::command::menu::DirectMenuControl;

        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let preset = PresetNumber::new(255).expect("255 is a valid preset number");

        prepare_builtin_command(
            &PresetSet::new(preset),
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .expect("preset 255 set must prepare end-to-end on FR7");

        prepare_builtin_command(
            &PresetReset::new(preset),
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .expect("preset 255 reset must prepare end-to-end on FR7");

        prepare_builtin_operation::<completion::Targeted, _>(
            &PresetRecall::for_profile(preset, &fr7).expect("FR7 preset recall"),
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .expect("preset 255 recall must prepare end-to-end on FR7");

        prepare_builtin_command(
            &DirectMenuControl::new(0x00, 0xFF).expect("0xFF data byte is valid"),
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .expect("direct menu (_, 0xFF) must prepare end-to-end on FR7");
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
            CameraId::CAMERA_1,
            profile,
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

        // Pan/tilt limits carry the corner discriminator in both actions. A
        // set additionally carries the two converted coordinates, staying
        // within the four-scalar projection bound.
        row!(
            "pan/tilt limit set",
            PanTiltLimitSet::for_profile(
                PanTiltLimitCorner::DownLeft,
                Degrees(0.0),
                Degrees(0.0),
                &ptz,
            )
            .expect("limit set"),
            &ptz,
            B::PanTiltLimitSet,
            Set(S::PanTiltLimits),
            crate::runtime::engine::AppliedStateProjection::set(
                S::PanTiltLimits,
                &[i64::from(PanTiltLimitCorner::DownLeft.to_byte()), 0, 0],
            )
            .expect("limit set projection")
        );
        row!(
            "pan/tilt limit clear",
            PanTiltLimitClear::new(PanTiltLimitCorner::UpRight),
            &ptz,
            B::PanTiltLimitClear,
            Clear(S::PanTiltLimits),
            crate::runtime::engine::AppliedStateProjection::clear_with_values(
                S::PanTiltLimits,
                &[i64::from(PanTiltLimitCorner::UpRight.to_byte())],
            )
            .expect("limit clear projection")
        );

        row!(
            "preset recall speed",
            crate::command::PresetRecallSpeedCommand {
                speed: PresetRecallSpeed::new(12).expect("preset speed"),
            },
            &ptz,
            B::PresetRecallSpeed,
            Set(S::PresetRecallSpeed),
            crate::runtime::engine::AppliedStateProjection::set(S::PresetRecallSpeed, &[12])
                .expect("preset speed projection")
        );
        row!(
            "focus lock on",
            FocusLock::On,
            &ptz,
            B::FocusLock,
            Set(S::FocusLockMode),
            crate::runtime::engine::AppliedStateProjection::set(S::FocusLockMode, &[1])
                .expect("focus lock projection")
        );
        row!(
            "focus lock off",
            FocusLock::Off,
            &ptz,
            B::FocusLock,
            Set(S::FocusLockMode),
            crate::runtime::engine::AppliedStateProjection::set(S::FocusLockMode, &[0])
                .expect("focus lock projection")
        );

        row!(
            "spotlight on",
            SpotlightOn::new(),
            &fr7,
            B::SpotlightOn,
            Set(S::Spotlight),
            crate::runtime::engine::AppliedStateProjection::set(S::Spotlight, &[1])
                .expect("spotlight projection")
        );
        row!(
            "spotlight off",
            SpotlightOff::new(),
            &fr7,
            B::SpotlightOff,
            Set(S::Spotlight),
            crate::runtime::engine::AppliedStateProjection::set(S::Spotlight, &[0])
                .expect("spotlight projection")
        );
        row!(
            "auto slow shutter on",
            AutoSlowShutterOn::new(),
            &evi,
            B::AutoSlowShutterOn,
            Set(S::AutoSlowShutter),
            crate::runtime::engine::AppliedStateProjection::set(S::AutoSlowShutter, &[1])
                .expect("slow shutter projection")
        );
        row!(
            "auto slow shutter off",
            AutoSlowShutterOff::new(),
            &evi,
            B::AutoSlowShutterOff,
            Set(S::AutoSlowShutter),
            crate::runtime::engine::AppliedStateProjection::set(S::AutoSlowShutter, &[0])
                .expect("slow shutter projection")
        );

        row!(
            "ND filter preset mode",
            NdFilterModeCommand::new(NdFilterMode::Preset),
            &fr7,
            B::NdFilterMode,
            Set(S::NdFilterMode),
            crate::runtime::engine::AppliedStateProjection::set(S::NdFilterMode, &[0])
                .expect("ND mode projection")
        );
        row!(
            "ND filter variable mode",
            NdFilterModeCommand::new(NdFilterMode::Variable),
            &fr7,
            B::NdFilterMode,
            Set(S::NdFilterMode),
            crate::runtime::engine::AppliedStateProjection::set(S::NdFilterMode, &[1])
                .expect("ND mode projection")
        );
        row!(
            "automatic ND on",
            AutoNdCommand::new(true),
            &fr7,
            B::NdFilterAutoOn,
            Set(S::AutoNdFilter),
            crate::runtime::engine::AppliedStateProjection::set(S::AutoNdFilter, &[1])
                .expect("automatic ND projection")
        );
        row!(
            "automatic ND off",
            AutoNdCommand::new(false),
            &fr7,
            B::NdFilterAutoOff,
            Set(S::AutoNdFilter),
            crate::runtime::engine::AppliedStateProjection::set(S::AutoNdFilter, &[0])
                .expect("automatic ND projection")
        );

        semantic_row!(
            "image freeze on",
            ImageFreeze::on(),
            B::ImageFreezeOn,
            Set(S::ImageFreeze),
            crate::runtime::engine::AppliedStateProjection::set(S::ImageFreeze, &[1])
                .expect("image freeze projection")
        );
        semantic_row!(
            "image freeze off",
            ImageFreeze::off(),
            B::ImageFreezeOff,
            Set(S::ImageFreeze),
            crate::runtime::engine::AppliedStateProjection::set(S::ImageFreeze, &[0])
                .expect("image freeze projection")
        );
        row!(
            "digital zoom on",
            DigitalZoom::new(true),
            &fr7,
            B::DigitalZoom,
            Set(S::DigitalZoomMode),
            crate::runtime::engine::AppliedStateProjection::set(S::DigitalZoomMode, &[1])
                .expect("digital zoom projection")
        );
        row!(
            "digital zoom off",
            DigitalZoom::new(false),
            &fr7,
            B::DigitalZoom,
            Set(S::DigitalZoomMode),
            crate::runtime::engine::AppliedStateProjection::set(S::DigitalZoomMode, &[0])
                .expect("digital zoom projection")
        );

        row!(
            "multicast on",
            MulticastStreaming::On,
            &ptz,
            B::MulticastStreamingOn,
            Set(S::MulticastStreaming),
            crate::runtime::engine::AppliedStateProjection::set(S::MulticastStreaming, &[1])
                .expect("multicast projection")
        );
        row!(
            "multicast off",
            MulticastStreaming::Off,
            &ptz,
            B::MulticastStreamingOff,
            Set(S::MulticastStreaming),
            crate::runtime::engine::AppliedStateProjection::set(S::MulticastStreaming, &[0])
                .expect("multicast projection")
        );
        for (name, quality, value) in [
            ("high", NdiQuality::High, 1),
            ("medium", NdiQuality::Medium, 2),
            ("low", NdiQuality::Low, 3),
            ("off", NdiQuality::Off, 4),
        ] {
            let command = SetNdiQuality::new(quality);
            assert_state_row(
                &format!("NDI quality {name}"),
                &command,
                &ptz,
                B::NdiQuality,
                Set(S::NdiQuality),
                crate::runtime::engine::AppliedStateProjection::set(S::NdiQuality, &[value])
                    .expect("NDI quality projection"),
            );
        }

        semantic_row!(
            "tally brightness low",
            TallyBrightLo::new(),
            B::TallyBrightLow,
            Set(S::TallyBrightness),
            crate::runtime::engine::AppliedStateProjection::set(S::TallyBrightness, &[0])
                .expect("tally brightness projection")
        );
        semantic_row!(
            "tally brightness high",
            TallyBrightHi::new(),
            B::TallyBrightHigh,
            Set(S::TallyBrightness),
            crate::runtime::engine::AppliedStateProjection::set(S::TallyBrightness, &[1])
                .expect("tally brightness projection")
        );
        row!(
            "variable speed standard",
            crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Standard24),
            &fr7,
            B::VariableSpeedMode,
            Set(S::VariableSpeedMode),
            crate::runtime::engine::AppliedStateProjection::set(S::VariableSpeedMode, &[1])
                .expect("variable speed projection")
        );
        row!(
            "variable speed fine",
            crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Fine50),
            &fr7,
            B::VariableSpeedMode,
            Set(S::VariableSpeedMode),
            crate::runtime::engine::AppliedStateProjection::set(S::VariableSpeedMode, &[2])
                .expect("variable speed projection")
        );

        // PTZOptics tally-mode opcodes retain their closed state projections
        // even though no built-in profile currently opts into their dedicated
        // typed-support surface.
        semantic_row!(
            "tally on",
            TallyOn::new(),
            B::TallyOn,
            Set(S::TallyMode),
            crate::runtime::engine::AppliedStateProjection::set(S::TallyMode, &[1])
                .expect("tally mode projection")
        );
        semantic_row!(
            "tally off",
            TallyOff::new(),
            B::TallyOff,
            Set(S::TallyMode),
            crate::runtime::engine::AppliedStateProjection::set(S::TallyMode, &[0])
                .expect("tally mode projection")
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
                crate::runtime::engine::AppliedStateProjection::set(
                    S::Flip,
                    &[horizontal, vertical],
                )
                .expect("combined flip projection"),
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
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &FocusLock::On,
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &SpotlightOn::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &AutoSlowShutterOn::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &NdFilterModeCommand::new(NdFilterMode::Variable),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &DigitalZoom::new(true),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &MulticastStreaming::On,
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &SetNdiQuality::new(NdiQuality::High),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyBrightLo::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &crate::command::SetVariableSpeedMode::new(VariableSpeedMode::Standard24),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyOn::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_command(
            &TallyFlash::new(),
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
        )
        .is_err());
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn write_only_state_requests_use_their_exact_plain_policies() {
        assert_policy::<PanTiltLimitSet>(
            16,
            TimeoutClass::Quick,
            RetryClass::Standard,
            ControlClass::Normal,
        );
        assert_policy::<PanTiltLimitClear>(
            16,
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
        assert_eq!(written, request.encoded_size());
        assert!(written <= R::MAX_SIZE);
    }

    fn assert_exact_declared_wire_size<R: Request>(request: &R, expected: usize) {
        assert_eq!(R::MAX_SIZE, expected);
        assert_eq!(request.encoded_size(), expected);
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
                CameraId::CAMERA_1,
                &profile,
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
            9,
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
        let conversion = source
            .pan_tilt_coordinates()
            .expect("FR7 pan/tilt conversion");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "Documented Digital Range Without Permission".into();
        capabilities.typed_support =
            crate::capabilities::TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
        let profile = ProfileSpec::builder(capabilities)
            .pan_tilt_coordinates(
                conversion.coordinate_system(),
                conversion.pan_degrees_to_units(),
                conversion.tilt_degrees_to_units(),
            )
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .preset_recall_axes(source.preset_recall_axes())
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
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("explicit optical position remains governed by its numeric range");

        let error = prepare_builtin_operation::<completion::Targeted, _>(
            &normalized,
            CameraId::CAMERA_1,
            &profile,
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
                CameraId::CAMERA_1,
                &fr7,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &IrisUp,
                CameraId::CAMERA_1,
                &fr7,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &IrisDown,
                CameraId::CAMERA_1,
                &fr7,
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        let iris_direct = IrisDirect::new(IrisLevel::new(4).unwrap());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &iris_direct,
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_err());
        for result in [
            prepare_builtin_operation::<completion::Targeted, _>(
                &nd,
                CameraId::CAMERA_1,
                &generic,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &NdFilterStepUp,
                CameraId::CAMERA_1,
                &generic,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::Targeted, _>(
                &NdFilterStepDown,
                CameraId::CAMERA_1,
                &generic,
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        for result in [
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                &PushAfPress,
                CameraId::CAMERA_1,
                &generic,
                OperationalTuning::new(),
            ),
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                &PushAfRelease,
                CameraId::CAMERA_1,
                &generic,
                OperationalTuning::new(),
            ),
        ] {
            assert!(result.is_err());
        }
        assert_eq!(request_write_count(), 0);
    }

    #[test]
    fn noise_reduction_metadata_requires_matching_inquiry_and_control_support() {
        let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let coordinates = source
            .pan_tilt_coordinates()
            .expect("G2 pan/tilt conversion");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "NR inquiry without NR control".into();
        capabilities.typed_support = capabilities
            .typed_support
            .without(TypedSupportSurface::NoiseReduction2DControl)
            .without(TypedSupportSurface::NoiseReduction3DControl);
        let error = ProfileSpec::builder(capabilities)
            .pan_tilt_coordinates(
                coordinates.coordinate_system(),
                coordinates.pan_degrees_to_units(),
                coordinates.tilt_degrees_to_units(),
            )
            .pan_tilt_wire_codec(coordinates.wire_codec())
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .preset_recall_axes(source.preset_recall_axes())
            .position_inquiries(source.position_inquiries())
            .build()
            .expect_err("inquiry-only NR runtime profile must be rejected");
        assert!(matches!(error, Error::InvalidRequest(message)
            if message.contains("noise-reduction metadata and paired inquiry/control typed support must agree")));
    }

    #[test]
    fn profile_validation_limits_typed_iris_to_supported_profiles() {
        let ptz = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZ profile");
        let fr7 = ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile");
        let h900 = ProfileSpec::from_compile_time::<SonyBRCH900>().expect("H900 profile");
        let iris = IrisDirect::new(IrisLevel::new(4).unwrap());

        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            CameraId::CAMERA_1,
            &ptz,
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &iris,
            CameraId::CAMERA_1,
            &ptz,
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &IrisDirect::new(IrisLevel::new(0x1E).unwrap()),
            CameraId::CAMERA_1,
            &h900,
            OperationalTuning::new(),
        )
        .is_ok());

        reset_request_write_count();
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_err());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &iris,
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &NdFilterDirect::new(NdFilterValue::new(4).unwrap()),
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &NdFilterStepUp,
            CameraId::CAMERA_1,
            &fr7,
            OperationalTuning::new(),
        )
        .is_ok());
        assert!(prepare_builtin_operation::<completion::AppliedOnly, _>(
            &PushAfPress,
            CameraId::CAMERA_1,
            &fr7,
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
                    CameraId::CAMERA_1,
                    profile,
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
                CameraId::CAMERA_1,
                &fr7,
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
        let coordinates = source
            .pan_tilt_coordinates()
            .expect("generic pan/tilt conversion");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "Exposure inventory without typed permission".into();
        capabilities.typed_support = capabilities
            .typed_support
            .without(TypedSupportSurface::ExposureMode);
        let generic = ProfileSpec::builder(capabilities)
            .pan_tilt_coordinates(
                coordinates.coordinate_system(),
                coordinates.pan_degrees_to_units(),
                coordinates.tilt_degrees_to_units(),
            )
            .pan_tilt_wire_codec(coordinates.wire_codec())
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .preset_recall_axes(source.preset_recall_axes())
            .position_inquiries(source.position_inquiries())
            .build()
            .expect("runtime profile without exposure-mode permission");
        assert!(!generic.capabilities().exposure_modes.is_empty());
        assert!(!generic
            .capabilities()
            .permits_typed(TypedSupportSurface::ExposureMode));

        reset_request_write_count();
        let error = prepare_builtin_command(
            &ExposureCommand::new(ExposureMode::Auto),
            CameraId::CAMERA_1,
            &generic,
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
                    CameraId::CAMERA_1,
                    $profile,
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
