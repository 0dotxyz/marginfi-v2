#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;

#[cfg_attr(feature = "anchor", account)]
#[cfg_attr(feature = "anchor", derive(InitSpace))]
#[cfg_attr(not(feature = "anchor"), derive(Debug, PartialEq, Eq, Copy, Clone))]
pub struct UnwindConfig {
    pub fee_multiplier_bps: u32,
    pub min_unwind_lamports: u64,
    pub bump: u8,
}

#[cfg_attr(feature = "anchor", account)]
#[cfg_attr(feature = "anchor", derive(InitSpace))]
#[cfg_attr(not(feature = "anchor"), derive(Debug, PartialEq, Eq, Copy, Clone))]
pub struct UnwindTicket {
    pub sol_bank: Pubkey,
    pub stake_account: Pubkey,
    pub rent_recipient: Pubkey,
    pub principal_lamports: u64,
    pub stake_rent_prefund_lamports: u64,
    pub created_epoch: u64,
    pub bump: u8,
}
