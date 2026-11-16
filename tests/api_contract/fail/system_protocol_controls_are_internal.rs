// Broadcast setup and socket cancellation need owner-held transport and
// correlation authority. They must not be constructible as generic commands.
use grafton_visca::command::system::{AddressSet, CommandCancel, InterfaceClear};

fn main() {}

//~ E0432
//~ "unresolved imports `grafton_visca::command::system::AddressSet`, `grafton_visca::command::system::CommandCancel`, `grafton_visca::command::system::InterfaceClear`"
