use anchor_lang::prelude::*;

use crate::constants::{MAX_DIVISOR, MAX_MULTIPLIER};
use crate::errors::ErrorCodes;
use crate::helper::get_multiplier_and_divisor;

#[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, InitSpace, Copy, PartialEq)]
pub enum SourceType {
    #[default]
    Pyth,

    StakePool,

    MsolPool,

    Redstone,

    Chainlink,

    SinglePool,

    JupLend,
}

#[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, InitSpace, Copy)]
pub struct Sources {
    pub source: Pubkey,
    pub invert: bool,
    pub multiplier: u128, // unused in current implementation
    pub divisor: u128,    // unused in current implementation
    pub source_type: SourceType,
}

impl Sources {
    pub fn is_valid(&self) -> bool {
        self.source != Pubkey::default()
            && self.divisor != 0
            && self.multiplier != 0
            && self.divisor <= MAX_DIVISOR
            && self.multiplier <= MAX_MULTIPLIER
    }

    pub fn is_single_pool_source(&self) -> bool {
        self.source_type == SourceType::SinglePool
    }

    pub fn is_jup_lend_source(&self) -> bool {
        self.source_type == SourceType::JupLend
    }

    pub fn verify_source(&self, account: &AccountInfo) -> Result<()> {
        if account.key() != self.source {
            return err!(ErrorCodes::InvalidSource);
        }

        Ok(())
    }
}

pub struct Price {
    pub price: u128,
    pub exponent: Option<u8>,
}

impl Price {
    pub fn get(&self) -> Result<(u128, u128, u128)> {
        if self.exponent.is_none() {
            Ok((self.price, 1, 1))
        } else {
            let (multiplier, divisor) = get_multiplier_and_divisor(self.exponent.unwrap() as u32);
            Ok((self.price, multiplier, divisor))
        }
    }
}
