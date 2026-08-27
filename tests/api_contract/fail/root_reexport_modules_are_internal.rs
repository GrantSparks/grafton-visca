// Several root types are re-exported out of private modules. The type is
// public; the module it is written in is not, so applications cannot pin the
// crate's internal file layout by naming it.

fn main() {
    let _ = core::any::TypeId::of::<grafton_visca::error::Error>();
    let _ = core::any::TypeId::of::<grafton_visca::session_config::SessionConfig>();
    let _ = core::any::TypeId::of::<grafton_visca::outcome::CancellationOutcome>();
    let _ = core::any::TypeId::of::<grafton_visca::operation_id::OperationId>();
    let _ = core::any::TypeId::of::<grafton_visca::requests::ControlClass>();
}

//~ E0603
//~ "module `error` is private"
//~ "module `session_config` is private"
//~ "module `outcome` is private"
//~ "module `operation_id` is private"
//~ "module `requests` is private"
