use grafton_visca::{
    command::pan_tilt::ClearLimit, operation::Targeted, request::OperationCommand,
};

fn requires_targeted<O: OperationCommand<Targeted>>() {}

fn main() {
    requires_targeted::<ClearLimit>();
    //~^ E0277 "ClearLimit: OperationCommand<"
}
