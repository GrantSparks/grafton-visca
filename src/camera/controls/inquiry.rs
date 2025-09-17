//! inquiry control implementation using Mode trait.

use crate::{
    camera::ViscaClient,
    command::{
        system::MotionSyncMode, ExposureMode, FocusMode, FocusZone, SharpnessMode, WhiteBalanceMode,
    },
    mode::Mode,
    Error,
};

/// inquiry operations for cameras.
///
/// This trait provides inquiry methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait InquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    fn power_state(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current zoom position.
    fn zoom_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ZoomPosition, Error>>;

    /// Get the current focus position.
    fn focus_position(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the focus near limit position.
    fn focus_near_limit(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the current focus zone.
    fn focus_zone(&self) -> <Self::Mode as Mode>::Fut<'_, Result<FocusZone, Error>>;

    /// Get the current exposure mode.
    fn exposure_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<ExposureMode, Error>>;

    /// Get the exposure compensation value.
    fn exposure_compensation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<i8, Error>>;

    /// Check if exposure compensation is enabled.
    fn exposure_compensation_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current iris value.
    fn iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the current shutter speed.
    fn shutter(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the current gain value.
    fn gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the gain limit value.
    fn gain_limit(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the white balance mode.
    fn white_balance_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<WhiteBalanceMode, Error>>;

    /// Get the current red gain.
    fn red_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<i8, Error>>;

    /// Get the current blue gain.
    fn blue_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<i8, Error>>;

    /// Get the red tuning value.
    fn red_tuning(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the blue tuning value.
    fn blue_tuning(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the current color temperature in Kelvin.
    fn color_temperature(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the gamma level.
    fn gamma(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the brightness level.
    fn brightness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the sharpness mode.
    fn sharpness_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<SharpnessMode, Error>>;

    /// Get the saturation level.
    fn saturation(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the hue setting.
    fn hue(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Check if black and white mode is enabled.
    fn black_white(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current video resolution mode.
    fn resolution(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the current picture effect mode.
    fn picture_effect(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the current ND filter position (Sony FR7 only).
    fn nd_filter_position(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the camera version information.
    fn version(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::typed::VersionInfo, Error>>;

    /// Check if backlight compensation is enabled.
    fn backlight_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the image flip settings (mirror/reverse).
    fn image_flip(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::FlipState, Error>>;

    /// Get the current focus mode (Auto/Manual).
    fn focus_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<FocusMode, Error>>;

    /// Get the menu open/close status.
    fn menu_status(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the tally light status (red and green).
    fn tally_light_status(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::typed::TallyStatusState, Error>>;

    /// Get the night/day mode status.
    fn night_day_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current flip mode (combined horizontal/vertical).
    fn flip_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::FlipState, Error>>;

    /// Get the standby mode status.
    fn standby_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the iris control mode.
    fn iris_control(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the defog level.
    fn defog_level(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the digital Ptz mode status.
    fn digital_ptz_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the exposure compensation position.
    fn exposure_compensation_position(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the auto trace mode status.
    fn auto_trace_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the focus unlock state.
    fn focus_unlock(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the noise reduction level.
    fn noise_reduction_level(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the noise reduction 2D level.
    fn noise_reduction_2d(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the noise reduction 3D level.
    fn noise_reduction_3d(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the broadcast domain setting.
    fn broadcast_domain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the noise reduction mode setting.
    fn noise_reduction_mode(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::NoiseReductionMode, Error>>;

    /// Get the black and white mode setting.
    fn black_white_mode(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::BlackWhiteMode, Error>>;

    /// Get the USB audio state.
    fn usb_audio_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the two tone mode state.
    fn two_tone_mode_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the ND filter preset setting.
    fn nd_filter_preset(&self) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the digital mode state.
    fn digital_mode_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the tally auto adjust state.
    fn tally_auto_adjust_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the motion sync mode setting.
    fn motion_sync_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<MotionSyncMode, Error>>;
}

/// pan/tilt-specific inquiry operations for cameras.
///
/// This trait provides pan/tilt inquiry methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait PanTiltInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current pan and tilt position.
    fn pan_tilt_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::camera::PanTiltPosition, Error>>;
}

// Single unified implementation for InquiryControl
impl<M, P, Tr, Exec> InquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn power_state(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::PowerInquiry;
        self.query(PowerInquiry)
    }

    fn zoom_position(&self) -> M::Fut<'_, Result<crate::types::ZoomPosition, Error>> {
        use crate::command::inquiry_structs::ZoomPositionInquiry;
        self.query(ZoomPositionInquiry)
    }

    fn focus_position(&self) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::FocusPositionInquiry;
        self.query(FocusPositionInquiry)
    }

    fn focus_near_limit(&self) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::FocusNearLimitInquiry;
        self.query(FocusNearLimitInquiry)
    }

    fn focus_zone(&self) -> M::Fut<'_, Result<FocusZone, Error>> {
        use crate::command::inquiry_structs::FocusZoneInquiry;
        self.query(FocusZoneInquiry)
    }

    fn exposure_mode(&self) -> M::Fut<'_, Result<ExposureMode, Error>> {
        use crate::command::inquiry_structs::ExposureModeInquiry;
        self.query(ExposureModeInquiry)
    }

    fn exposure_compensation(&self) -> M::Fut<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationInquiry;
        self.query(ExposureCompensationInquiry)
    }

    fn exposure_compensation_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationModeInquiry;
        self.query(ExposureCompensationModeInquiry)
    }

    fn iris(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::IrisInquiry;
        self.query(IrisInquiry)
    }

    fn shutter(&self) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ShutterInquiry;
        self.query(ShutterInquiry)
    }

    fn gain(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GainInquiry;
        self.query(GainInquiry)
    }

    fn gain_limit(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GainLimitInquiry;
        self.query(GainLimitInquiry)
    }

    fn white_balance_mode(&self) -> M::Fut<'_, Result<WhiteBalanceMode, Error>> {
        use crate::command::inquiry_structs::WhiteBalanceModeInquiry;
        self.query(WhiteBalanceModeInquiry)
    }

    fn red_gain(&self) -> M::Fut<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::RedGainInquiry;
        self.query(RedGainInquiry)
    }

    fn blue_gain(&self) -> M::Fut<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::BlueGainInquiry;
        self.query(BlueGainInquiry)
    }

    fn red_tuning(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::RedTuningInquiry;
        self.query(RedTuningInquiry)
    }

    fn blue_tuning(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::BlueTuningInquiry;
        self.query(BlueTuningInquiry)
    }

    fn color_temperature(&self) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ColorTemperatureInquiry;
        self.query(ColorTemperatureInquiry)
    }

    fn gamma(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GammaInquiry;
        self.query(GammaInquiry)
    }

    fn brightness(&self) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::BrightnessInquiry;
        self.query(BrightnessInquiry)
    }

    fn sharpness_mode(&self) -> M::Fut<'_, Result<SharpnessMode, Error>> {
        use crate::command::inquiry_structs::SharpnessModeInquiry;
        self.query(SharpnessModeInquiry)
    }

    fn saturation(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::SaturationInquiry;
        self.query(SaturationInquiry)
    }

    fn hue(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::HueInquiry;
        self.query(HueInquiry)
    }

    fn black_white(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::BlackWhiteInquiry;
        self.query(BlackWhiteInquiry)
    }

    fn resolution(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::ResolutionInquiry;
        self.query(ResolutionInquiry)
    }

    fn picture_effect(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::PictureEffectInquiry;
        self.query(PictureEffectInquiry)
    }

    fn nd_filter_position(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NdFilterInquiry;
        self.query(NdFilterInquiry)
    }

    fn version(&self) -> M::Fut<'_, Result<crate::command::typed::VersionInfo, Error>> {
        use crate::command::inquiry_structs::VersionInquiry;
        self.query(VersionInquiry)
    }

    fn backlight_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::BacklightInquiry;
        self.query(BacklightInquiry)
    }

    fn image_flip(&self) -> M::Fut<'_, Result<crate::command::FlipState, Error>> {
        use crate::command::inquiry_structs::ImageFlipInquiry;
        self.query(ImageFlipInquiry)
    }

    fn focus_mode(&self) -> M::Fut<'_, Result<FocusMode, Error>> {
        use crate::command::inquiry_structs::FocusModeInquiry;
        self.query(FocusModeInquiry)
    }

    fn menu_status(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::MenuOpenCloseInquiry;
        self.query(MenuOpenCloseInquiry)
    }

    fn tally_light_status(
        &self,
    ) -> M::Fut<'_, Result<crate::command::typed::TallyStatusState, Error>> {
        use crate::command::inquiry_structs::TallyStatusInquiry;
        self.query(TallyStatusInquiry)
    }

    fn night_day_mode(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::NightDayModeInquiry;
        self.query(NightDayModeInquiry)
    }

    fn flip_mode(&self) -> M::Fut<'_, Result<crate::command::FlipState, Error>> {
        use crate::command::inquiry_structs::FlipStateInquiry;
        self.query(FlipStateInquiry)
    }

    fn standby_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::StandbyInquiry;
        self.query(StandbyInquiry)
    }

    fn iris_control(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::IrisControlInquiry;
        self.query(IrisControlInquiry)
    }

    fn defog_level(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::DefogLevelInquiry;
        self.query(DefogLevelInquiry)
    }

    fn digital_ptz_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::DigitalPtzInquiry;
        self.query(DigitalPtzInquiry)
    }

    fn exposure_compensation_position(&self) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationPositionInquiry;
        self.query(ExposureCompensationPositionInquiry)
    }

    fn auto_trace_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::AutoTraceInquiry;
        self.query(AutoTraceInquiry)
    }

    fn focus_unlock(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::FocusUnlockInquiry;
        self.query(FocusUnlockInquiry)
    }

    fn noise_reduction_level(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NrLevelInquiry;
        self.query(NrLevelInquiry)
    }

    fn noise_reduction_2d(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NoiseReduction2DInquiry;
        self.query(NoiseReduction2DInquiry)
    }

    fn noise_reduction_3d(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NoiseReduction3DInquiry;
        self.query(NoiseReduction3DInquiry)
    }

    fn broadcast_domain(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::BroadcastDomainInquiry;
        self.query(BroadcastDomainInquiry)
    }

    fn noise_reduction_mode(
        &self,
    ) -> M::Fut<'_, Result<crate::command::NoiseReductionMode, Error>> {
        use crate::command::inquiry_structs::NrModeInquiry;
        self.query(NrModeInquiry)
    }

    fn black_white_mode(&self) -> M::Fut<'_, Result<crate::command::BlackWhiteMode, Error>> {
        use crate::command::inquiry_structs::BlackWhiteModeInquiry;
        self.query(BlackWhiteModeInquiry)
    }

    fn usb_audio_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::UsbAudioInquiry;
        self.query(UsbAudioInquiry)
    }

    fn two_tone_mode_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::TwoToneModeInquiry;
        self.query(TwoToneModeInquiry)
    }

    fn nd_filter_preset(&self) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NdFilterPresetInquiry;
        self.query(NdFilterPresetInquiry)
    }

    fn digital_mode_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::DigitalInquiry;
        self.query(DigitalInquiry)
    }

    fn tally_auto_adjust_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::TallyAutoAdjustInquiry;
        self.query(TallyAutoAdjustInquiry)
    }

    fn motion_sync_mode(&self) -> M::Fut<'_, Result<MotionSyncMode, Error>> {
        use crate::command::inquiry_structs::MotionSyncModeInquiry;
        self.query(MotionSyncModeInquiry)
    }
}

// Single unified implementation for PanTiltInquiryControl
impl<M, P, Tr, Exec> PanTiltInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn pan_tilt_position(&self) -> M::Fut<'_, Result<crate::camera::PanTiltPosition, Error>> {
        use crate::command::inquiry_structs::PanTiltPositionInquiry;
        self.query(PanTiltPositionInquiry)
    }
}
