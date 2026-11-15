use grafton_visca::{capabilities::*, profiles::*};

fn needs_focus_infinity<P: HasFocusInfinity>() {}
fn needs_preset_memory<P: HasPresetMemory>() {}
fn needs_shutter_control<P: HasShutterControl>() {}
fn needs_gain_control<P: HasGainControl>() {}
fn needs_gain_limit<P: HasGainLimit>() {}
fn needs_standard_white_balance_modes<P: HasStandardWhiteBalanceModes>() {}
fn needs_menu_display<P: HasMenuDisplay>() {}
fn needs_menu_enter_return<P: HasMenuEnterReturn>() {}
fn needs_menu_status_inquiry<P: HasMenuStatusInquiry>() {}

fn needs_shutter_direct<P: HasShutterDirect>() {}

fn needs_power_standby<P: HasPowerStandby>() {}

fn main() {
    needs_power_standby::<SonyBRC300>();
    needs_power_standby::<NearusBRC300>();
    needs_power_standby::<GenericVisca>();
    needs_shutter_direct::<SonyBRCH900>();
    needs_focus_infinity::<PtzOpticsG2>();
    needs_preset_memory::<SonyBRCH900>();
    needs_shutter_control::<SonyFR7>();
    needs_gain_control::<SonyFR7>();
    needs_gain_limit::<SonyEVIH100>();
    needs_standard_white_balance_modes::<SonyFR7>();
    needs_menu_display::<PtzOpticsG2>();
    needs_menu_enter_return::<SonyFR7>();
    needs_menu_status_inquiry::<SonyEVIH100>();
}

//~ E0277
//~ "does not declare focus-to-infinity control support"
//~ "does not declare preset store and reset support"
//~ "does not declare shared shutter control and inquiry support"
//~ "does not declare shared gain control and inquiry support"
//~ "does not declare gain-limit control and inquiry support"
//~ "does not declare standard automatic, indoor and outdoor white-balance modes support"
//~ "does not declare menu display on/off control support"
//~ "does not declare menu enter and return control support"
//~ "does not declare menu-status inquiry support"
//~ "does not declare direct shutter positioning support"
//~ "does not declare power standby support"
