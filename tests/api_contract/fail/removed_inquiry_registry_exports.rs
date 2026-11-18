//! Decoder registry internals have no supported command/wire export aliases.
use grafton_visca::command::{InquiryData as LegacyData, InquiryKind as LegacyKind};
//~^ E0603 "enum `InquiryData` is private"
//~^^ E0603 "enum `InquiryKind` is private"
use grafton_visca::wire::{InquiryData, InquiryKind};
//~^ E0432 "unresolved imports `grafton_visca::wire::InquiryData`, `grafton_visca::wire::InquiryKind`"

fn main() {}
