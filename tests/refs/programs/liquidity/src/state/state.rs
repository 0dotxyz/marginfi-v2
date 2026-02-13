use anchor_lang::prelude::*;

use crate::constants::{MAX_AUTH_COUNT, MAX_USER_CLASSES};

use library::math::safe_math::*;

#[account]
#[derive(InitSpace)]
pub struct Liquidity {
    pub authority: Pubkey, // Main liquidity authority, can be Governance account or multisig
    pub revenue_collector: Pubkey, // Address that collects fees
    pub status: bool,      // true = locked, false = unlocked
    pub bump: u8,
}

impl Liquidity {
    pub fn is_locked(&self) -> bool {
        self.status
    }

    pub fn init(&mut self, authority: Pubkey, revenue_collector: Pubkey, bump: u8) -> Result<()> {
        self.authority = authority;
        self.revenue_collector = revenue_collector;
        self.status = false;
        self.bump = bump;

        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct UserClass {
    pub addr: Pubkey,
    pub class: u8,
}

#[account]
#[derive(InitSpace)]
pub struct AuthorizationList {
    #[max_len(MAX_AUTH_COUNT)]
    pub auth_users: Vec<Pubkey>, // Authorization list
    #[max_len(MAX_AUTH_COUNT)]
    pub guardians: Vec<Pubkey>, // Guardian list
    #[max_len(MAX_USER_CLASSES)]
    pub user_classes: Vec<UserClass>, // User class list
}

impl AuthorizationList {
    pub fn init(&mut self, authority: Pubkey) -> Result<()> {
        self.auth_users.push(authority);
        self.guardians.push(authority);

        Ok(())
    }
}

#[account(zero_copy)]
#[derive(InitSpace)]
#[repr(C, packed)]
pub struct UserClaim {
    pub user: Pubkey,
    pub amount: u64,
    pub mint: Pubkey,
}

impl UserClaim {
    pub fn init(&mut self, user: Pubkey, mint: Pubkey) -> Result<()> {
        self.user = user;
        self.mint = mint;
        Ok(())
    }

    pub fn balance(&self) -> u64 {
        self.amount
    }

    pub fn reset_balance(&mut self) -> Result<()> {
        self.amount = 0;
        Ok(())
    }

    pub fn approve(&mut self, amount: u64) -> Result<()> {
        self.amount = self.amount.safe_add(amount)?;
        Ok(())
    }
}
