use anchor_lang::prelude::*;

use crate::constant::MAX_AUTH_COUNT;

#[account]
#[derive(InitSpace)]
// Factory account
pub struct LendingAdmin {
    pub authority: Pubkey,         // Governance account
    pub liquidity_program: Pubkey, // Address of liquidity program
    pub rebalancer: Pubkey, // Address of rebalancer to call `rebalance()` and source for funding rewards (ReserveContract).

    pub next_lending_id: u16,

    #[max_len(MAX_AUTH_COUNT)]
    pub auths: Vec<Pubkey>, // Addresses that can call `rebalance()` and set source for funding rewards (ReserveContract).

    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Lending {
    pub mint: Pubkey, // address of the token
    pub f_token_mint: Pubkey,

    pub lending_id: u16,

    /// @dev number of decimals for the fToken, same as ASSET
    pub decimals: u8,

    /// @dev To read PDA of rewards rate model to get_rate instruction
    pub rewards_rate_model: Pubkey,

    /// @dev exchange price for the underlying asset in the liquidity protocol (without rewards)
    pub liquidity_exchange_price: u64,

    /// @dev exchange price between fToken and the underlying asset (with rewards)
    pub token_exchange_price: u64,

    /// @dev timestamp when exchange prices were updated the last time
    pub last_update_timestamp: u64,

    // Liquidity PDA accounts
    pub token_reserves_liquidity: Pubkey,

    pub supply_position_on_liquidity: Pubkey,

    pub bump: u8,
}
