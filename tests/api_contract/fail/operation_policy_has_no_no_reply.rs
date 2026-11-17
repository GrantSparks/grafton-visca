use grafton_visca::raw::OperationReplyShape;

fn main() {
    let _ = OperationReplyShape::NoReply;
}

//~ E0599
//~ "named `NoReply` found for enum `OperationReplyShape`"
