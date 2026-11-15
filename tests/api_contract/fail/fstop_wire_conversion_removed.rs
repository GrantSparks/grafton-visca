fn main() {
    let _ = <grafton_visca::types::IrisLevel as From<grafton_visca::types::FStop>>::from(grafton_visca::types::FStop::F5_6);
}

//~ E0277
//~ "From<FStop>"
