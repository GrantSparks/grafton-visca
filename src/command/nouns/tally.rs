//! Tally indication, brightness and readback requests.

use crate::command::{
    nouns::forward_inquiry,
    semantics::{BuiltinCommand, BuiltinRequestContract},
};
#[cfg(any(feature = "blocking", feature = "async"))]
use crate::request::builtin_contract::BuiltinStateProjection;
use crate::request::builtin_contract::{BuiltinStateValue, BuiltinValidation};
use crate::types::{TallyBrightness, TallyMode};
use crate::{CameraId, ControlClass, Request, RetryClass, TimeoutClass};

/// Reads the red tally indication.
#[derive(Debug, Clone, Copy)]
pub struct RedInquiry;
forward_inquiry!(RedInquiry, crate::command::TallyRedInquiry; crate::command::TallyRedInquiry);

/// Reads the green tally indication.
#[derive(Debug, Clone, Copy)]
pub struct GreenInquiry;
forward_inquiry!(GreenInquiry, crate::command::TallyGreenInquiry; crate::command::TallyGreenInquiry);

// Selection retains the original protocol-granular request on every path.
// In particular, target validation continues to use that request's exact row,
// source evidence and capability gate rather than a family-wide permission.
macro_rules! selected_tally_request {
    (
        $(#[$doc:meta])* $name:ident {
            $field:ident: $value:ty,
            primary: $primary:ty,
            rows: { $($pattern:pat => ($inner:expr, $row:ident)),+ $(,)? }
        }
    ) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            $field: $value,
        }

        impl $name {
            /// Creates the checked tally setting.
            #[must_use]
            pub const fn new($field: $value) -> Self { Self { $field } }

            /// Returns the requested setting.
            #[must_use]
            pub const fn $field(self) -> $value { self.$field }

            /// The exact protocol row selected by this setting.
            #[cfg(test)]
            pub(crate) const fn ledger_row(&self) -> BuiltinCommand {
                match self.$field { $($pattern => BuiltinCommand::$row),+ }
            }

            pub(crate) const VALUE_SELECTED_LEDGER_ROWS: &'static [BuiltinCommand] = &[$(BuiltinCommand::$row),+];
        }

        impl Request for $name {
            type Class = <$primary as Request>::Class;
            const MAX_SIZE: usize = <$primary as Request>::MAX_SIZE;
            const TIMEOUT_CLASS: TimeoutClass = <$primary as Request>::TIMEOUT_CLASS;
            const RETRY_CLASS: RetryClass = <$primary as Request>::RETRY_CLASS;
            const CONTROL_CLASS: ControlClass = <$primary as Request>::CONTROL_CLASS;

            fn timeout_class(&self) -> TimeoutClass {
                match self.$field { $($pattern => Request::timeout_class(&$inner)),+ }
            }
            fn retry_class(&self) -> RetryClass {
                match self.$field { $($pattern => Request::retry_class(&$inner)),+ }
            }
            fn control_class(&self) -> ControlClass {
                match self.$field { $($pattern => Request::control_class(&$inner)),+ }
            }
            fn motion_axes(&self) -> Option<crate::AffectedAxes> {
                match self.$field { $($pattern => Request::motion_axes(&$inner)),+ }
            }
            fn reply_shape(&self) -> crate::raw::RawReplyShape {
                match self.$field { $($pattern => Request::reply_shape(&$inner)),+ }
            }
            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> crate::Result<usize> {
                match self.$field { $($pattern => Request::write_into(&$inner, camera_id, buffer)),+ }
            }
            fn validate_for_target(&self, target: &crate::TargetRegistration) -> crate::Result<()> {
                match self.$field { $($pattern => Request::validate_for_target(&$inner, target)),+ }
            }
            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn write_for_target(&self, target: &crate::TargetRegistration, buffer: &mut [u8], authority: crate::requests::RequestContractAuthority) -> crate::Result<usize> {
                match self.$field { $($pattern => Request::write_for_target(&$inner, target, buffer, authority)),+ }
            }
            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn admission_control_class(&self, authority: crate::requests::RequestContractAuthority) -> crate::Result<ControlClass> {
                match self.$field { $($pattern => Request::admission_control_class(&$inner, authority)),+ }
            }
            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn declared_max_size(&self, authority: crate::requests::RequestContractAuthority) -> usize {
                match self.$field { $($pattern => Request::declared_max_size(&$inner, authority)),+ }
            }
            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn pan_tilt_max_speed(&self, authority: crate::requests::RequestContractAuthority) -> Option<u8> {
                match self.$field { $($pattern => Request::pan_tilt_max_speed(&$inner, authority)),+ }
            }
            #[cfg(any(feature = "blocking", feature = "async"))]
            #[doc(hidden)]
            #[allow(private_interfaces)]
            fn applied_state_projection(&self, _authority: crate::requests::AppliedStateAuthority) -> Option<crate::runtime::engine::AppliedStateProjection> {
                <<Self as BuiltinRequestContract>::State as BuiltinStateProjection<Self>>::project(self)
            }
        }

        impl BuiltinValidation for $name {}

        impl BuiltinRequestContract for $name {
            const LEDGER_ROW: BuiltinCommand = <$primary as BuiltinRequestContract>::LEDGER_ROW;
            type State = <$primary as BuiltinRequestContract>::State;
            #[cfg(test)]
            fn selected_ledger_row(&self) -> BuiltinCommand { self.ledger_row() }
            const LEDGER_ROWS: &'static [BuiltinCommand] = Self::VALUE_SELECTED_LEDGER_ROWS;
        }

        const _: () = <$name as BuiltinRequestContract>::CLASS_MATCHES_ROW;
        $(const _: () = assert!(
            BuiltinCommand::$row.classification().same_contract(<$primary as BuiltinRequestContract>::LEDGER_ROW.classification()),
            "a tally selector changes its semantic state/class contract",
        );)+
        $(const _: () = {
            const fn assert_policy<R: Request>(_: &R) {
                assert!(R::MAX_SIZE <= <$name as Request>::MAX_SIZE, "tally selector exceeds its declared bound");
                assert!(R::TIMEOUT_CLASS as u8 == <$name as Request>::TIMEOUT_CLASS as u8, "tally selector changes timeout class");
                assert!(R::RETRY_CLASS as u8 == <$name as Request>::RETRY_CLASS as u8, "tally selector changes retry class");
                assert!(R::CONTROL_CLASS as u8 == <$name as Request>::CONTROL_CLASS as u8, "tally selector changes control class");
            }
            assert_policy(&$inner);
        };)+
    };
}

selected_tally_request! {
    /// Enables or disables the red tally indication.
    SetRed {
        enabled: bool,
        primary: crate::command::TallyRedOn,
        rows: {
            true => (crate::command::TallyRedOn::new(), TallyRedOn),
            false => (crate::command::TallyRedOff::new(), TallyRedOff),
        }
    }
}

selected_tally_request! {
    /// Enables or disables the green tally indication.
    SetGreen {
        enabled: bool,
        primary: crate::command::TallyGreenOn,
        rows: {
            true => (crate::command::TallyGreenOn::new(), TallyGreenOn),
            false => (crate::command::TallyGreenOff::new(), TallyGreenOff),
        }
    }
}

selected_tally_request! {
    /// Selects low or high tally brightness without changing its documented bytes.
    SetBrightness {
        brightness: TallyBrightness,
        primary: crate::command::TallyBrightLo,
        rows: {
            TallyBrightness::Low => (crate::command::TallyBrightLo::new(), TallyBrightLow),
            TallyBrightness::High => (crate::command::TallyBrightHi::new(), TallyBrightHigh),
        }
    }
}

impl BuiltinStateValue<TallyBrightness> for SetBrightness {
    fn state_value(&self) -> TallyBrightness {
        self.brightness
    }
}

selected_tally_request! {
    /// Selects a steady, flashing or disabled tally indication.
    SetMode {
        mode: TallyMode,
        primary: crate::command::TallyOn,
        rows: {
            TallyMode::Off => (crate::command::TallyOff::new(), TallyOff),
            TallyMode::On => (crate::command::TallyOn::new(), TallyOn),
            TallyMode::Flash => (crate::command::TallyFlash::new(), TallyFlash),
        }
    }
}

impl BuiltinStateValue<TallyMode> for SetMode {
    fn state_value(&self) -> TallyMode {
        self.mode
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn wire(request: &impl Request) -> Vec<u8> {
        let mut bytes = [0; 16];
        let written = request
            .write_into(CameraId::CAMERA_1, &mut bytes)
            .expect("tally frame");
        bytes[..written].to_vec()
    }

    #[test]
    fn merged_tally_setters_preserve_every_literal_packet_and_selected_row() {
        for (enabled, code, red, green) in [
            (
                true,
                0x02,
                BuiltinCommand::TallyRedOn,
                BuiltinCommand::TallyGreenOn,
            ),
            (
                false,
                0x03,
                BuiltinCommand::TallyRedOff,
                BuiltinCommand::TallyGreenOff,
            ),
        ] {
            let red_request = SetRed::new(enabled);
            let green_request = SetGreen::new(enabled);
            assert_eq!(red_request.ledger_row(), red);
            assert_eq!(green_request.ledger_row(), green);
            assert_eq!(
                wire(&red_request),
                vec![0x81, 1, 0x7E, 1, 0x0A, 0, code, 0xFF]
            );
            assert_eq!(
                wire(&green_request),
                vec![0x81, 1, 0x7E, 4, 0x1A, 0, code, 0xFF]
            );
        }
        for (brightness, code) in [(TallyBrightness::Low, 4), (TallyBrightness::High, 5)] {
            assert_eq!(
                wire(&SetBrightness::new(brightness)),
                vec![0x81, 1, 0x7E, 1, 0x0A, 1, code, 0xFF]
            );
        }
        for (mode, code, row) in [
            (TallyMode::Off, 3, BuiltinCommand::TallyOff),
            (TallyMode::On, 2, BuiltinCommand::TallyOn),
            (TallyMode::Flash, 1, BuiltinCommand::TallyFlash),
        ] {
            let request = SetMode::new(mode);
            assert_eq!(request.ledger_row(), row);
            assert_eq!(wire(&request), vec![0x81, 0x0A, 2, 2, code, 0xFF]);
        }
    }
    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn merged_tally_setters_project_the_same_typed_setting_including_flash() {
        use crate::runtime::engine::AppliedStateProjection;
        use crate::{requests::AppliedStateAuthority, KnownState};
        for mode in [TallyMode::Off, TallyMode::On, TallyMode::Flash] {
            assert_eq!(
                SetMode::new(mode).applied_state_projection(AppliedStateAuthority::new()),
                Some(AppliedStateProjection::set(KnownState::TallyMode(mode)))
            );
        }
        for brightness in [TallyBrightness::Low, TallyBrightness::High] {
            assert_eq!(
                SetBrightness::new(brightness)
                    .applied_state_projection(AppliedStateAuthority::new()),
                Some(AppliedStateProjection::set(KnownState::TallyBrightness(
                    brightness
                )))
            );
        }
    }
}
