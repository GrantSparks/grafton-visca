use grafton_visca::StateKnowledge;

fn main() {
    let _ = core::mem::size_of::<StateKnowledge>();
}

//~ E0432
//~ "unresolved import `grafton_visca::StateKnowledge`"
