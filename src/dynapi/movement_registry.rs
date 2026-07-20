//! Declarative inventory for the dyn movement methods that need operation handles.
//!
//! The public dyn traits remain written out explicitly so their rustdoc and stable
//! 1.x signatures stay easy to review. Their repetitive adapters are generated
//! from this registry and all route through the command-derived operation core.

macro_rules! dyn_pan_tilt_operations {
    ($consumer:ident) => {
        $consumer! {
            pan_tilt_home() => pan_tilt_home_op: crate::camera::PanTiltOperation
                [DynOperationRequirement::None] =
                Ok::<_, crate::Error>(crate::command::pan_tilt::PanTilt::Home);
            pan_tilt_absolute(
                pan_deg: f64,
                tilt_deg: f64,
                speed: crate::types::SpeedLevel
            ) => pan_tilt_absolute_op: crate::camera::PanTiltOperation
                [DynOperationRequirement::None] =
                crate::camera::controls::pan_tilt::pan_tilt_absolute_command::<P>(
                    pan_deg, tilt_deg, speed,
                );
            pan_tilt_relative(
                pan_deg: f64,
                tilt_deg: f64,
                speed: crate::types::SpeedLevel
            ) => pan_tilt_relative_op: crate::camera::PanTiltOperation
                [DynOperationRequirement::None] =
                crate::camera::controls::pan_tilt::pan_tilt_relative_command::<P>(
                    pan_deg, tilt_deg, speed,
                );
            pan_tilt_reset() => pan_tilt_reset_op: crate::camera::PanTiltOperation
                [DynOperationRequirement::None] =
                Ok::<_, crate::Error>(crate::command::pan_tilt::PanTilt::Reset);
        }
    };
}

macro_rules! dyn_zoom_operations {
    ($consumer:ident) => {
        $consumer! {
            zoom_tele(speed: Option<crate::types::ZoomSpeed>) => none:
                crate::camera::ZoomOperation [DynOperationRequirement::None] =
                crate::camera::controls::zoom::zoom_tele_command::<P>(speed);
            zoom_wide(speed: Option<crate::types::ZoomSpeed>) => none:
                crate::camera::ZoomOperation [DynOperationRequirement::None] =
                crate::camera::controls::zoom::zoom_wide_command::<P>(speed);
            set_zoom(position: crate::types::ZoomPosition) => set_zoom_op:
                crate::camera::ZoomOperation [DynOperationRequirement::Typed(
                        crate::capabilities::TypedSupportSurface::DirectZoom,
                        "direct zoom positioning",
                    )] = crate::camera::controls::zoom::zoom_position_command::<P>(position);
            set_zoom_normalized(position: crate::UnitInterval) => none:
                crate::camera::ZoomOperation [DynOperationRequirement::Typed(
                        crate::capabilities::TypedSupportSurface::DirectZoom,
                        "direct zoom positioning",
                    )] = crate::camera::controls::zoom::zoom_normalized_command::<P>(position);
            set_zoom_normalized_in_domain(
                position: crate::UnitInterval,
                domain: crate::ZoomDomain
            ) => none: crate::camera::ZoomOperation
                [DynOperationRequirement::ZoomDomain(domain)] =
                crate::camera::controls::zoom::zoom_from_normalized_for_profile::<P>(
                    position, domain,
                )
                .and_then(crate::camera::controls::zoom::zoom_position_command::<P>);
        }
    };
}

macro_rules! dyn_focus_operations {
    ($consumer:ident) => {
        $consumer! {
            set_focus(position: crate::types::FocusPosition) => set_focus_op:
                crate::camera::FocusOperation [DynOperationRequirement::None] =
                crate::camera::controls::focus::focus_position_command::<P, _>(position);
        }
    };
}

macro_rules! dyn_preset_operations {
    ($consumer:ident) => {
        $consumer! {
            preset_recall(preset: crate::command::preset::PresetNumber) => preset_recall_op:
                crate::camera::PresetOperation [DynOperationRequirement::None] =
                crate::camera::controls::presets::preset_command::<P>(
                    crate::command::preset::PresetAction::Recall,
                    preset,
                );
        }
    };
}

pub(crate) use {
    dyn_focus_operations, dyn_pan_tilt_operations, dyn_preset_operations, dyn_zoom_operations,
};

#[cfg(test)]
macro_rules! define_inventory {
    ($($method:ident($($arg:ident: $arg_ty:ty),*) => $handle:ident: $category:ty [$requirement:expr] = $build:expr;)*) => {
        &[$(stringify!($method)),*]
    };
}

#[cfg(test)]
pub(crate) const PAN_TILT_INVENTORY: &[&str] = dyn_pan_tilt_operations!(define_inventory);
#[cfg(test)]
pub(crate) const ZOOM_INVENTORY: &[&str] = dyn_zoom_operations!(define_inventory);
#[cfg(test)]
pub(crate) const FOCUS_INVENTORY: &[&str] = dyn_focus_operations!(define_inventory);
#[cfg(test)]
pub(crate) const PRESET_INVENTORY: &[&str] = dyn_preset_operations!(define_inventory);
