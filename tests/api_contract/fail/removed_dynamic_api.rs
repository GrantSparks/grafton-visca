//! Runtime profiles use concrete facade types, with no separate Dyn surface.
use grafton_visca::dynapi::{
    BlockingDynSessionCamera, DynAppliedOperation, DynSessionCamera, DynTargetedOperation,
};
use grafton_visca::{
    BlockingDynSessionCamera as RootBlockingCamera, DynAppliedOperation as RootApplied,
    DynSessionCamera as RootAsyncCamera, DynTargetedOperation as RootTargeted,
};

fn main() {}

//~ E0432
//~ "unresolved import `grafton_visca::dynapi`"
//~ "no `BlockingDynSessionCamera` in the root"
//~ "no `DynAppliedOperation` in the root"
//~ "no `DynSessionCamera` in the root"
//~ "no `DynTargetedOperation` in the root"
