use anchor_lang::prelude::*;

/// @dev min flashloan amount
pub const MIN_FLASHLOAN_AMOUNT: u64 = 1_000; // 1e3

// 0.5% max fee allowed
pub const FLASHLOAN_FEE_MAX: u16 = 50; // 1e4 = 100%, 1e2 = 1%

pub const FLASHLOAN_STACK_HEIGHT: usize = 1;

pub const FOUR_DECIMALS: u128 = 10000;

// &anchor_lang::solana_program::hash::hash(b"global:flashloan_payback").to_bytes()[..8];
pub const FLASHLOAN_PAYBACK_DISCRIMINATOR: &[u8] = &[213, 47, 153, 137, 84, 243, 94, 232];

pub const GOVERNANCE_MS: Pubkey = pubkey!("HqPrpa4ESBDnRHRWaiYtjv4xe93wvCS9NNZtDwR89cVa");
