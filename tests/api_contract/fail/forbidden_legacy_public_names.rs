// These names belonged to the removed 1.x facade and must not be re-exported
// by the owner-backed 2.0 API.
use grafton_visca::{
    AsyncCamera, BlockingCamera, BlockingClient, InFlight, OpKind, OperationMetadata, PowerControl,
    ResponseFuture, RuntimeHandle,
};

use grafton_visca::mode::{Async, Blocking, BlockingFutureExt, Mode};

fn main() {}

//~ E0432
//~ "unresolved imports `grafton_visca::AsyncCamera`"
//~ "unresolved import `grafton_visca::mode`"
