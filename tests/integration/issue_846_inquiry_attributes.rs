//! Typed selector decoding must agree with the variant accepted by the derive.

include!("../api_contract/pass/visca_inquiry_parser_attributes.rs");

#[test]
fn typed_mode_nibble_and_explicit_variant_decoders_share_their_payload() {
    main();
    assert!(ModeInquiry.decoder().decode(&[0x00, 0x00]).is_err());
    assert!(NibbleInquiry.decoder().decode(&[0x10, 0x05]).is_err());
}
