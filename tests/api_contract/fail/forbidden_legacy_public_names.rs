// These names belonged to the removed 1.x facade and must not be re-exported
// by the owner-backed 2.0 API.
use grafton_visca::{
    AsyncCamera, BlockingCamera, BlockingClient, InFlight, OpKind, OperationMetadata, PowerControl,
    //~^ E0432 "unresolved imports `grafton_visca::AsyncCamera`"
    ResponseFuture, RuntimeHandle,
    //~^ E0432 "unresolved imports `grafton_visca::AsyncCamera`"
};

use grafton_visca::mode::{Async, Blocking, BlockingFutureExt, Mode};
//~^ E0432 "unresolved import `grafton_visca::mode`"

fn main() {}
