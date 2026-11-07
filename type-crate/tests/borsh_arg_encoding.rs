//! Runs with `ix_builders` and without `anchor`, so `borsh::BorshSerialize` is the derive under
//! test. `programs/marginfi/tests/misc/anchor_arg_encoding.rs` asserts the same shapes with the
//! `AnchorSerialize` derive, which is a different proc macro.

include!("fixtures/arg_encodings.rs");

#[test]
fn borsh_derive_matches_the_borsh_spec() {
    for (label, derived, expected) in encodings() {
        assert_eq!(derived, expected, "borsh derive disagrees for {label}");
    }
}
