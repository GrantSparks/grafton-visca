use grafton_visca::{operation, raw, request::OperationCommand};

fn requires_targeted<T: OperationCommand<operation::Targeted>>() {}

fn main() {
    requires_targeted::<raw::AppliedOnlyRequest>();
}

//~ E0277
//~ "AppliedOnlyRequest: OperationCommand<"
