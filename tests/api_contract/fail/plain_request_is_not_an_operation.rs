use grafton_visca::{
    command::PanTiltLimitCorner, completion::Targeted, request::builtin::PanTiltLimitClear,
    OperationCommand,
};

fn requires_targeted<O: OperationCommand<Targeted>>(_: O) {}

fn main() {
    requires_targeted(PanTiltLimitClear::new(PanTiltLimitCorner::UpRight));
}

//~ E0277
//~ "the trait bound `PanTiltLimitClear: OperationCommand<grafton_visca::completion::Targeted>` is not satisfied"
