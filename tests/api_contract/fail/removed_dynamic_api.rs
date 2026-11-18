//! Runtime profiles use concrete facade types, with no separate Dyn surface.
use grafton_visca::dynapi::{
//~^ E0432 "unresolved import `grafton_visca::dynapi`"
    BlockingDynSessionCamera, DynAppliedOperation, DynSessionCamera, DynTargetedOperation,
};
use grafton_visca::{
    BlockingDynSessionCamera as RootBlockingCamera, DynAppliedOperation as RootApplied,
    //~^ E0432 "unresolved imports `grafton_visca::BlockingDynSessionCamera`, `grafton_visca::DynAppliedOperation`, `grafton_visca::DynSessionCamera`, `grafton_visca::DynTargetedOperation`"
    DynSessionCamera as RootAsyncCamera, DynTargetedOperation as RootTargeted,
    //~^ E0432 "unresolved imports `grafton_visca::BlockingDynSessionCamera`, `grafton_visca::DynAppliedOperation`, `grafton_visca::DynSessionCamera`, `grafton_visca::DynTargetedOperation`"
};

fn main() {}
