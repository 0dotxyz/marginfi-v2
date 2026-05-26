use trident_fuzz::fuzzing::*;

use crate::types::marginfi::OracleSetup;

#[derive(Clone, Copy)]
pub struct Currency {
    pub mint: Pubkey,
    pub mint_authority: Pubkey,
}

impl Currency {
    pub fn new(mint: Pubkey, mint_authority: Pubkey) -> Self {
        Self {
            mint,
            mint_authority,
        }
    }
}

#[derive(Clone, Copy)]
pub struct FuzzTestBank {
    pub currency: Currency,
    pub address: Pubkey,
    pub oracle_setup: (OracleSetup, Pubkey),
    /// True if the underlying mint is Token-2022 with `TransferFeeConfig`.
    /// On a deposit, the user is debited the full `amount` but the bank
    /// vault only receives `amount − fee` (the fee goes to the mint's
    /// withheld balance). Conservation and exact-amount invariants must
    /// account for that — helpers skip the strict checks when this is true.
    pub has_transfer_fee: bool,
}
