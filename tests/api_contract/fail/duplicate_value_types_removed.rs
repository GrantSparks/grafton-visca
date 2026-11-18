// #806, #808, #819: superseded duplicates of canonical value types are gone,
// and only the wire value is named `ShutterSpeed` (the table entry is
// `capabilities::ShutterSpeedEntry`). One import per removed name, so each
// removal is pinned by its own diagnostic.
use grafton_visca::camera::profiles::G2Gain;
//~^ E0432 "unresolved import `grafton_visca::camera::profiles::G2Gain`"
//~^^ E0603 "module `camera` is private"
use grafton_visca::camera::profiles::G2PresetId;
//~^ E0432 "unresolved import `grafton_visca::camera::profiles::G2PresetId`"
//~^^ E0603 "module `camera` is private"
use grafton_visca::capabilities::ShutterSpeed;
//~^ E0432 "unresolved import `grafton_visca::capabilities::ShutterSpeed`"
use grafton_visca::command::FocusSpeed;
//~^ E0432 "unresolved import `grafton_visca::command::FocusSpeed`"
use grafton_visca::command::Iris;
//~^ E0603 "enum `Iris` is private"
use grafton_visca::command::NightDayMode;
//~^ E0432 "unresolved import `grafton_visca::command::NightDayMode`"
use grafton_visca::command::TallyStatus;
//~^ E0432 "unresolved import `grafton_visca::command::TallyStatus`"
use grafton_visca::command::Version;
//~^ E0432 "unresolved import `grafton_visca::command::Version`"
use grafton_visca::PanTiltPositionRaw;
//~^ E0432 "unresolved import `grafton_visca::PanTiltPositionRaw`"
use grafton_visca::ZoomPositionExt;
//~^ E0432 "unresolved import `grafton_visca::ZoomPositionExt`"

fn main() {}
