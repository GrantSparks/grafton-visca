// #806, #808, #819: superseded duplicates of canonical value types are gone,
// and only the wire value is named `ShutterSpeed` (the table entry is
// `capabilities::ShutterSpeedEntry`). One import per removed name, so each
// removal is pinned by its own diagnostic.
use grafton_visca::camera::profiles::G2Gain;
use grafton_visca::camera::profiles::G2PresetId;
use grafton_visca::capabilities::ShutterSpeed;
use grafton_visca::command::FocusSpeed;
use grafton_visca::command::IrisControl;
use grafton_visca::command::NightDayMode;
use grafton_visca::command::TallyStatus;
use grafton_visca::command::Version;
use grafton_visca::PanTiltPositionRaw;
use grafton_visca::ZoomPositionExt;

fn main() {}

//~ E0432
//~ "no `G2Gain` in `camera::profiles`"
//~ "no `G2PresetId` in `camera::profiles`"
//~ "no `ShutterSpeed` in `capabilities`"
//~ "no `FocusSpeed` in `command`"
//~ "no `IrisControl` in `command`"
//~ "no `NightDayMode` in `command`"
//~ "no `TallyStatus` in `command`"
//~ "no `Version` in `command`"
//~ "no `PanTiltPositionRaw` in the root"
//~ "no `ZoomPositionExt` in the root"
