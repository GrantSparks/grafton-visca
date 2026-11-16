use grafton_visca::{capabilities::*, profiles::*};

fn needs_focus_infinity<P: HasFocusInfinity>() {}
fn needs_preset_memory<P: HasPresetMemory>() {}
fn needs_shutter_control<P: HasShutter>() {}
fn needs_gain_control<P: HasGain>() {}
fn needs_gain_limit<P: HasGainLimit>() {}
fn needs_menu_display<P: HasMenuDisplay>() {}
fn needs_menu_enter_return<P: HasMenuEnterReturn>() {}
fn needs_menu_status_inquiry<P: HasMenuDisplayInquiry>() {}

fn needs_shutter_direct<P: HasDirectShutter>() {}

fn needs_power_standby<P: HasPowerStandby>() {}

fn main() {
    needs_power_standby::<SonyBrc300>();
    needs_power_standby::<NearusBrc300>();
    needs_power_standby::<GenericVisca>();
    needs_shutter_direct::<SonyBrcH900>();
    needs_focus_infinity::<PtzOpticsG2>();
    needs_preset_memory::<SonyBrcH900>();
    needs_shutter_control::<SonyFr7>();
    needs_gain_control::<SonyFr7>();
    needs_gain_limit::<SonyEviH100>();
    needs_menu_display::<PtzOpticsG2>();
    needs_menu_enter_return::<SonyFr7>();
    needs_menu_status_inquiry::<SonyEviH100>();
}

//~ E0277
//~ "does not declare focus-to-infinity control support"
//~ "does not declare preset store and reset support"
//~ "does not declare shared shutter control and inquiry support"
//~ "does not declare shared gain control and inquiry support"
//~ "does not declare gain-limit control and inquiry support"
//~ "does not declare menu display on/off control support"
//~ "does not declare menu enter and return control support"
//~ "does not declare menu-status inquiry support"
//~ "does not declare direct shutter positioning support"
//~ "does not declare power standby support"
