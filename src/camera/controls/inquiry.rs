//! Unified inquiry control implementation using Mode trait.

use crate::{
    camera::CameraSend,
    command::{
        system::MotionSyncMode, ExposureMode, FocusMode, FocusZone, SharpnessMode, WhiteBalanceMode,
    },
    mode::Mode,
    Error,
};

/// Unified inquiry operations for cameras.
///
/// This trait provides inquiry methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::forward_control_to_session]
pub trait InquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    fn get_power_state(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the current zoom position.
    fn get_zoom_position(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<crate::types::ZoomPosition, Error>>;

    /// Get the current focus position.
    fn get_focus_position(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>>;

    /// Get the focus near limit position.
    fn get_focus_near_limit(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>>;

    /// Get the current focus zone.
    fn get_focus_zone(&self) -> <Self::Mode as Mode>::Ret<'_, Result<FocusZone, Error>>;

    /// Get the current exposure mode.
    fn get_exposure_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<ExposureMode, Error>>;

    /// Get the exposure compensation value.
    fn get_exposure_compensation(&self) -> <Self::Mode as Mode>::Ret<'_, Result<i8, Error>>;

    /// Check if exposure compensation is enabled.
    fn get_exposure_compensation_enabled(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the current iris value.
    fn get_iris(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the current shutter speed.
    fn get_shutter(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>>;

    /// Get the current gain value.
    fn get_gain(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the gain limit value.
    fn get_gain_limit(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the white balance mode.
    fn get_white_balance_mode(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<WhiteBalanceMode, Error>>;

    /// Get the current red gain.
    fn get_red_gain(&self) -> <Self::Mode as Mode>::Ret<'_, Result<i8, Error>>;

    /// Get the current blue gain.
    fn get_blue_gain(&self) -> <Self::Mode as Mode>::Ret<'_, Result<i8, Error>>;

    /// Get the red tuning value.
    fn get_red_tuning(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the blue tuning value.
    fn get_blue_tuning(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the current color temperature in Kelvin.
    fn get_color_temperature(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>>;

    /// Get the gamma level.
    fn get_gamma(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the brightness level.
    fn get_brightness(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>>;

    /// Get the sharpness mode.
    fn get_sharpness_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<SharpnessMode, Error>>;

    /// Get the saturation level.
    fn get_saturation(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the hue setting.
    fn get_hue(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Check if black and white mode is enabled.
    fn get_black_white(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the current video resolution mode.
    fn get_resolution(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the current picture effect mode.
    fn get_picture_effect(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the current ND filter position (Sony FR7 only).
    fn get_nd_filter_position(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the camera version information.
    fn get_version(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<crate::command::typed::VersionInfo, Error>>;

    /// Check if backlight compensation is enabled.
    fn get_backlight_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the image flip settings (mirror/reverse).
    fn get_image_flip(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<crate::command::typed::FlipState, Error>>;

    /// Get the current focus mode (Auto/Manual).
    fn get_focus_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<FocusMode, Error>>;

    /// Get the menu open/close status.
    fn get_menu_status(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the tally light status (red and green).
    fn get_tally_light_status(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<crate::command::typed::TallyStatusState, Error>>;

    /// Get the night/day mode status.
    fn get_night_day_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the current flip mode (combined horizontal/vertical).
    fn get_flip_mode(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<crate::command::typed::FlipState, Error>>;

    /// Get the standby mode status.
    fn get_standby_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the iris control mode.
    fn get_iris_control(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the defog level.
    fn get_defog_level(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the digital Ptz mode status.
    fn get_digital_ptz_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the exposure compensation position.
    fn get_exposure_compensation_position(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<u16, Error>>;

    /// Get the auto trace mode status.
    fn get_auto_trace_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the focus unlock state.
    fn get_focus_unlock(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the noise reduction level.
    fn get_noise_reduction_level(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the noise reduction 2D level.
    fn get_noise_reduction_2d(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the noise reduction 3D level.
    fn get_noise_reduction_3d(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the broadcast domain setting.
    fn get_broadcast_domain(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the noise reduction mode setting.
    fn get_noise_reduction_mode(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<crate::command::NrMode, Error>>;

    /// Get the black and white mode setting.
    fn get_black_white_mode(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<crate::command::BlackWhiteMode, Error>>;

    /// Get the USB audio state.
    fn get_usb_audio_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the two tone mode state.
    fn get_two_tone_mode_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the ND filter preset setting.
    fn get_nd_filter_preset(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;

    /// Get the digital mode state.
    fn get_digital_mode_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the tally auto adjust state.
    fn get_tally_auto_adjust_enabled(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Get the motion sync mode setting.
    fn get_motion_sync_mode(&self) -> <Self::Mode as Mode>::Ret<'_, Result<MotionSyncMode, Error>>;
}

/// Unified pan/tilt-specific inquiry operations for cameras.
///
/// This trait provides pan/tilt inquiry methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::forward_control_to_session]
pub trait PanTiltInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current pan and tilt position.
    fn get_pan_tilt_position(
        &self,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<crate::camera::PanTiltPosition, Error>>;
}

// Single unified implementation for InquiryControl
impl<M, P, Tr, Exec> InquiryControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CameraSend<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn get_power_state(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::PowerInquiry;
        self.send_and_parse(PowerInquiry)
    }

    fn get_zoom_position(&self) -> M::Ret<'_, Result<crate::types::ZoomPosition, Error>> {
        use crate::command::inquiry_structs::ZoomPositionInquiry;
        self.send_and_parse(ZoomPositionInquiry)
    }

    fn get_focus_position(&self) -> M::Ret<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::FocusPositionInquiry;
        self.send_and_parse(FocusPositionInquiry)
    }

    fn get_focus_near_limit(&self) -> M::Ret<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::FocusNearLimitInquiry;
        self.send_and_parse(FocusNearLimitInquiry)
    }

    fn get_focus_zone(&self) -> M::Ret<'_, Result<FocusZone, Error>> {
        use crate::command::inquiry_structs::FocusZoneInquiry;
        self.send_and_parse(FocusZoneInquiry)
    }

    fn get_exposure_mode(&self) -> M::Ret<'_, Result<ExposureMode, Error>> {
        use crate::command::inquiry_structs::ExposureModeInquiry;
        self.send_and_parse(ExposureModeInquiry)
    }

    fn get_exposure_compensation(&self) -> M::Ret<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationInquiry;
        self.send_and_parse(ExposureCompensationInquiry)
    }

    fn get_exposure_compensation_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationModeInquiry;
        self.send_and_parse(ExposureCompensationModeInquiry)
    }

    fn get_iris(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::IrisInquiry;
        self.send_and_parse(IrisInquiry)
    }

    fn get_shutter(&self) -> M::Ret<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ShutterInquiry;
        self.send_and_parse(ShutterInquiry)
    }

    fn get_gain(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GainInquiry;
        self.send_and_parse(GainInquiry)
    }

    fn get_gain_limit(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GainLimitInquiry;
        self.send_and_parse(GainLimitInquiry)
    }

    fn get_white_balance_mode(&self) -> M::Ret<'_, Result<WhiteBalanceMode, Error>> {
        use crate::command::inquiry_structs::WhiteBalanceModeInquiry;
        self.send_and_parse(WhiteBalanceModeInquiry)
    }

    fn get_red_gain(&self) -> M::Ret<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::RedGainInquiry;
        self.send_and_parse(RedGainInquiry)
    }

    fn get_blue_gain(&self) -> M::Ret<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::BlueGainInquiry;
        self.send_and_parse(BlueGainInquiry)
    }

    fn get_red_tuning(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::RedTuningInquiry;
        self.send_and_parse(RedTuningInquiry)
    }

    fn get_blue_tuning(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::BlueTuningInquiry;
        self.send_and_parse(BlueTuningInquiry)
    }

    fn get_color_temperature(&self) -> M::Ret<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ColorTemperatureInquiry;
        self.send_and_parse(ColorTemperatureInquiry)
    }

    fn get_gamma(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GammaInquiry;
        self.send_and_parse(GammaInquiry)
    }

    fn get_brightness(&self) -> M::Ret<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::BrightInquiry;
        self.send_and_parse(BrightInquiry)
    }

    fn get_sharpness_mode(&self) -> M::Ret<'_, Result<SharpnessMode, Error>> {
        use crate::command::inquiry_structs::SharpnessModeInquiry;
        self.send_and_parse(SharpnessModeInquiry)
    }

    fn get_saturation(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::SaturationInquiry;
        self.send_and_parse(SaturationInquiry)
    }

    fn get_hue(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::HueInquiry;
        self.send_and_parse(HueInquiry)
    }

    fn get_black_white(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::BlackWhiteInquiry;
        self.send_and_parse(BlackWhiteInquiry)
    }

    fn get_resolution(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::ResolutionInquiry;
        self.send_and_parse(ResolutionInquiry)
    }

    fn get_picture_effect(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::PictureEffectInquiry;
        self.send_and_parse(PictureEffectInquiry)
    }

    fn get_nd_filter_position(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NdFilterInquiry;
        self.send_and_parse(NdFilterInquiry)
    }

    fn get_version(&self) -> M::Ret<'_, Result<crate::command::typed::VersionInfo, Error>> {
        use crate::command::inquiry_structs::VersionInquiry;
        self.send_and_parse(VersionInquiry)
    }

    fn get_backlight_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::BacklightInquiry;
        self.send_and_parse(BacklightInquiry)
    }

    fn get_image_flip(&self) -> M::Ret<'_, Result<crate::command::typed::FlipState, Error>> {
        use crate::command::inquiry_structs::ImageFlipInquiry;
        self.send_and_parse(ImageFlipInquiry)
    }

    fn get_focus_mode(&self) -> M::Ret<'_, Result<FocusMode, Error>> {
        use crate::command::inquiry_structs::FocusModeInquiry;
        self.send_and_parse(FocusModeInquiry)
    }

    fn get_menu_status(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::MenuOpenCloseInquiry;
        self.send_and_parse(MenuOpenCloseInquiry)
    }

    fn get_tally_light_status(
        &self,
    ) -> M::Ret<'_, Result<crate::command::typed::TallyStatusState, Error>> {
        use crate::command::inquiry_structs::TallyStatusInquiry;
        self.send_and_parse(TallyStatusInquiry)
    }

    fn get_night_day_mode(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::NightDayModeInquiry;
        self.send_and_parse(NightDayModeInquiry)
    }

    fn get_flip_mode(&self) -> M::Ret<'_, Result<crate::command::typed::FlipState, Error>> {
        use crate::command::inquiry_structs::FlipModeInquiry;
        self.send_and_parse(FlipModeInquiry)
    }

    fn get_standby_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::StandbyInquiry;
        self.send_and_parse(StandbyInquiry)
    }

    fn get_iris_control(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::IrisControlInquiry;
        self.send_and_parse(IrisControlInquiry)
    }

    fn get_defog_level(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::DefogLevelInquiry;
        self.send_and_parse(DefogLevelInquiry)
    }

    fn get_digital_ptz_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::DigitalPtzInquiry;
        self.send_and_parse(DigitalPtzInquiry)
    }

    fn get_exposure_compensation_position(&self) -> M::Ret<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationPositionInquiry;
        self.send_and_parse(ExposureCompensationPositionInquiry)
    }

    fn get_auto_trace_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::AutoTraceInquiry;
        self.send_and_parse(AutoTraceInquiry)
    }

    fn get_focus_unlock(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::FocusUnlockInquiry;
        self.send_and_parse(FocusUnlockInquiry)
    }

    fn get_noise_reduction_level(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NrLevelInquiry;
        self.send_and_parse(NrLevelInquiry)
    }

    fn get_noise_reduction_2d(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NoiseReduction2DInquiry;
        self.send_and_parse(NoiseReduction2DInquiry)
    }

    fn get_noise_reduction_3d(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NoiseReduction3DInquiry;
        self.send_and_parse(NoiseReduction3DInquiry)
    }

    fn get_broadcast_domain(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::BroadcastDomainInquiry;
        self.send_and_parse(BroadcastDomainInquiry)
    }

    fn get_noise_reduction_mode(&self) -> M::Ret<'_, Result<crate::command::NrMode, Error>> {
        use crate::command::inquiry_structs::NrModeInquiry;
        self.send_and_parse(NrModeInquiry)
    }

    fn get_black_white_mode(&self) -> M::Ret<'_, Result<crate::command::BlackWhiteMode, Error>> {
        use crate::command::inquiry_structs::BlackWhiteModeInquiry;
        self.send_and_parse(BlackWhiteModeInquiry)
    }

    fn get_usb_audio_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::UsbAudioInquiry;
        self.send_and_parse(UsbAudioInquiry)
    }

    fn get_two_tone_mode_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::TwoToneModeInquiry;
        self.send_and_parse(TwoToneModeInquiry)
    }

    fn get_nd_filter_preset(&self) -> M::Ret<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NdFilterPresetInquiry;
        self.send_and_parse(NdFilterPresetInquiry)
    }

    fn get_digital_mode_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::DigitalInquiry;
        self.send_and_parse(DigitalInquiry)
    }

    fn get_tally_auto_adjust_enabled(&self) -> M::Ret<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::TallyAutoAdjustInquiry;
        self.send_and_parse(TallyAutoAdjustInquiry)
    }

    fn get_motion_sync_mode(&self) -> M::Ret<'_, Result<MotionSyncMode, Error>> {
        use crate::command::inquiry_structs::MotionSyncModeInquiry;
        self.send_and_parse(MotionSyncModeInquiry)
    }
}

// Single unified implementation for PanTiltInquiryControl
impl<M, P, Tr, Exec> PanTiltInquiryControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CameraSend<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn get_pan_tilt_position(&self) -> M::Ret<'_, Result<crate::camera::PanTiltPosition, Error>> {
        use crate::command::inquiry_structs::PanTiltPositionInquiry;
        self.send_and_parse(PanTiltPositionInquiry)
    }
}
