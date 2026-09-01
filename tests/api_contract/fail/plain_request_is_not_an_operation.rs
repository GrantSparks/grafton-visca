use grafton_visca::{
    command::PanTiltLimitCorner, completion::Targeted, request::builtin::PanTiltLimitClear,
    OperationCommand,
};

fn requires_targeted<O: OperationCommand<Targeted>>(_: O) {}

fn main() {
    requires_targeted(PanTiltLimitClear::new(PanTiltLimitCorner::UpRight));
}

//~ E0277
//~ "PanTiltLimitClear: OperationCommand<"
