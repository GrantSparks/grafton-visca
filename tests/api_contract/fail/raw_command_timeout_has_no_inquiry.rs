use grafton_visca::CommandCategory;

fn main() {
    let _ = CommandCategory::Inquiry;
}

//~ E0599
//~ "named `Inquiry` found for enum `CommandCategory`"
