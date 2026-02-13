use anchor_lang::prelude::*;

#[account(zero_copy)]
#[derive(InitSpace)]
#[repr(C, packed)]
pub struct VaultConfig {
    pub vault_id: u16, // Vault ID

    pub supply_rate_magnifier: i16, // Supply rate magnifier; 10000 = 100%, -10000 = -100%
    pub borrow_rate_magnifier: i16, // Borrow rate magnifier; 10000 = 100%, -10000 = -100%
    pub collateral_factor: u16,     // Collateral factor. 800 = 0.8 = 80%
    pub liquidation_threshold: u16, // Liquidation Threshold. 900 = 0.9 = 90%
    pub liquidation_max_limit: u16, // Liquidation Max Limit. 950 = 0.95 = 95%
    pub withdraw_gap: u16,          // Withdraw gap. 100 = 0.1 = 10%
    pub liquidation_penalty: u16,   // Liquidation penalty. 100 = 0.01 = 1%
    pub borrow_fee: u16,            // Borrow fee. 100 = 0.01 = 1%
    pub oracle: Pubkey,             // Oracle PDA address from oracle program
    pub rebalancer: Pubkey,         // Address of rebalancer
    pub liquidity_program: Pubkey,  // Address of liquidity
    pub oracle_program: Pubkey,     // Address of oracle

    pub supply_token: Pubkey, // Address of supply token mint
    pub borrow_token: Pubkey, // Address of borrow token mint

    pub bump: u8, // Account bump
}
