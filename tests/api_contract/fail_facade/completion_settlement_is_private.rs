use grafton_visca::{operation::SettlementPlan, operation::Targeted};

fn main() {
    // The lifecycle's public outcome remains independently available.
    let _: Option<grafton_visca::operation::Settlement> = None;
    let _: Option<SettlementPlan<Targeted>> = None;
}

//~ E0603
//~ "struct `SettlementPlan` is private"
