//! Decoder registry internals have no supported command/wire export aliases.
use grafton_visca::command::{InquiryData as LegacyData, InquiryKind as LegacyKind};
use grafton_visca::wire::{InquiryData, InquiryKind};

fn main() {}

//~ E0432
//~ E0603
//~ "no `InquiryData` in `wire`"
//~ "no `InquiryKind` in `wire`"
//~ "enum `InquiryData` is private"
//~ "enum `InquiryKind` is private"
