fn main() {
    let data = [0x02];
    let _ = grafton_visca::command::response::payload::Payload::new(&data);
}

//~ E0603
//~ "module `response` is private"
