use grafton_visca::{command::PowerInquiry, PlainCommand};

fn requires_plain<C: PlainCommand>(_: C) {}

fn main() {
    requires_plain(PowerInquiry);
}
