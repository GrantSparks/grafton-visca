use grafton_visca::{command::power::StatusInquiry, request::PlainCommand};

fn requires_plain<C: PlainCommand>(_: C) {}

fn main() {
    requires_plain(StatusInquiry);
}

//~ E0271
//~ "type mismatch resolving `<StatusInquiry as Request>::Class == PlainClass`"
