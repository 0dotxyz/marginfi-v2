use bytemuck::Zeroable;
use fixed::types::I80F48;
use types::FeeState;

pub mod constants;
pub mod macros;
pub mod types;

#[cfg(feature = "ix_builders")]
pub mod ix_builders;
#[cfg(feature = "pdas")]
pub mod pdas;

/// Builders address every instruction to [`ID`], which the SDK resolves from the network
/// feature. Picking one is mandatory here so a client cannot ship instructions aimed at the
/// wrong cluster's program.
#[cfg(all(
    feature = "ix_builders",
    not(any(
        feature = "mainnet-beta",
        feature = "devnet",
        feature = "staging",
        feature = "stagingalt",
        feature = "localnet",
    ))
))]
compile_error!(
    "marginfi-type-crate: `ix_builders` requires a network feature, one of `mainnet-beta`, \
     `devnet`, `staging`, `stagingalt`, or `localnet`."
);

/// Number of network features enabled; the SDK resolves exactly one program ID.
const NETWORK_FEATURES: usize = cfg!(feature = "mainnet-beta") as usize
    + cfg!(feature = "devnet") as usize
    + cfg!(feature = "staging") as usize
    + cfg!(feature = "stagingalt") as usize
    + cfg!(feature = "localnet") as usize;
// Folds to a literal in each configuration; only the multi-feature case can fail.
#[allow(clippy::absurd_extreme_comparisons)]
const _: () = assert!(
    NETWORK_FEATURES <= 1,
    "marginfi-type-crate: enable exactly one network feature (`mainnet-beta`, `devnet`, \
     `staging`, `stagingalt`, or `localnet`)."
);

// Keep these IDs synchronized with id-crate; the SDK has no dependency on the program crate.
#[cfg(any(feature = "anchor", feature = "ix_builders", feature = "pdas"))]
#[cfg(feature = "mainnet-beta")]
solana_pubkey::declare_id!("MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA");
#[cfg(any(feature = "anchor", feature = "ix_builders", feature = "pdas"))]
#[cfg(feature = "devnet")]
solana_pubkey::declare_id!("neetcne3Ctrrud7vLdt2ypMm21gZHGN2mCmqWaMVcBQ");
#[cfg(any(feature = "anchor", feature = "ix_builders", feature = "pdas"))]
#[cfg(feature = "staging")]
solana_pubkey::declare_id!("stag8sTKds2h4KzjUw3zKTsxbqvT4XKHdaR9X9E6Rct");
#[cfg(any(feature = "anchor", feature = "ix_builders", feature = "pdas"))]
#[cfg(feature = "stagingalt")]
solana_pubkey::declare_id!("5UDghkpgW1HfYSrmEj2iAApHShqU44H6PKTAar9LL9bY");
#[cfg(any(feature = "anchor", feature = "ix_builders", feature = "pdas"))]
#[cfg(not(any(
    feature = "mainnet-beta",
    feature = "devnet",
    feature = "staging",
    feature = "stagingalt"
)))]
solana_pubkey::declare_id!("2jGhuVUuy3umdzByFx8sNWUAaf5vaeuDm78RDPEnhrMr");

/// Just a sample function demonstrating usage.
pub fn generic_fee_state() -> FeeState {
    let mut fee_state = FeeState::zeroed();
    fee_state.program_fee_fixed = I80F48::from_num(0.01).into();
    fee_state.program_fee_rate = I80F48::from_num(0.05).into();
    fee_state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_fee_state_sample() {
        let state = generic_fee_state();
        let fee: I80F48 = state.program_fee_fixed.into();
        assert_eq!(fee, I80F48::from_num(0.01));
    }

    #[cfg(not(any(feature = "anchor", feature = "ix_builders", feature = "pdas")))]
    #[test]
    fn types_only_uses_stub_pubkeys() {
        use crate::types::{pubkey, AccountMeta, Bank, Pubkey};

        let key: Pubkey = pubkey::Pubkey::new([7; 32]);
        let mut bank = Bank::zeroed();
        bank.mint = key;
        let meta = AccountMeta {
            pubkey: bank.mint,
            is_signer: false,
            is_writable: true,
        };
        assert_eq!(meta.pubkey.to_bytes(), [7; 32]);
    }
}
