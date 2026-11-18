// Several root types are re-exported out of private modules. The type is
// public; the module it is written in is not, so applications cannot pin the
// crate's internal file layout by naming it.

fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::operational_tuning::OperationalTuning>();
    //~^ E0603 "module `operational_tuning` is private"
    let _ = core::any::TypeId::of::<grafton_visca::session_config::SessionConfig>();
    //~^ E0603 "module `session_config` is private"
    let _ = core::any::TypeId::of::<grafton_visca::outcome::CancellationOutcome>();
    //~^ E0603 "module `outcome` is private"
    let _ = core::any::TypeId::of::<grafton_visca::operation_id::OperationId>();
    //~^ E0603 "module `operation_id` is private"
    let _ = core::any::TypeId::of::<grafton_visca::requests::SubmissionClass>();
    //~^ E0603 "module `requests` is private"
}
