use grafton_visca::{completion, raw, OperationCommand};

fn requires_targeted<T: OperationCommand<completion::Targeted>>() {}

fn main() {
    requires_targeted::<raw::AppliedOnly>();
}
