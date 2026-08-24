//! The anchor half of the encoding check. This target builds the type crate with `anchor`, so
//! `AnchorSerialize` is the derive under test; `type-crate/tests/borsh_arg_encoding.rs` asserts
//! the same shapes with the `borsh::BorshSerialize` derive.

include!("../../../../type-crate/tests/fixtures/arg_encodings.rs");

#[test]
fn anchor_derive_matches_the_borsh_spec() {
    for (label, derived, expected) in encodings() {
        assert_eq!(derived, expected, "anchor derive disagrees for {label}");
    }
}
