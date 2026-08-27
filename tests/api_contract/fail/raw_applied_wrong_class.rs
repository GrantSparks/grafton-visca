use grafton_visca::{completion, raw, OperationCommand};

fn requires_targeted<T: OperationCommand<completion::Targeted>>() {}

fn main() {
    requires_targeted::<raw::AppliedOnly>();
}

//~ E0277
//~ "the trait bound `grafton_visca::raw::AppliedOnly: OperationCommand<grafton_visca::completion::Targeted>` is not satisfied"
