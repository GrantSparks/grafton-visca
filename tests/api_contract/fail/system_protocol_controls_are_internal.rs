// Broadcast setup and socket cancellation need owner-held transport and
// correlation authority. They must not be constructible as generic commands.
use grafton_visca::request::builtin::{AddressSet, CommandCancel, InterfaceClear};

fn main() {}

//~ E0432
//~ "unresolved imports `grafton_visca::request::builtin::AddressSet`, `grafton_visca::request::builtin::CommandCancel`, `grafton_visca::request::builtin::InterfaceClear`"
