use crate::StakedSettingsEditConfig;
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{BankConfigOpt, HealthCache, WrappedI80F48};

// Event headers

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GroupEventHeader {
    pub signer: Option<Pubkey>,
    pub marginfi_group: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AccountEventHeader {
    pub signer: Option<Pubkey>,
    pub marginfi_account: Pubkey,
    pub marginfi_account_authority: Pubkey,
    pub marginfi_group: Pubkey,
}

// marginfi group events

#[event]
pub struct MarginfiGroupCreateEvent {
    pub header: GroupEventHeader,
}

#[event]
pub struct MarginfiGroupConfigureEvent {
    pub header: GroupEventHeader,
    pub admin: Pubkey,
    pub flags: u64,
}

#[event]
pub struct LendingPoolBankCreateEvent {
    pub header: GroupEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
}

#[event]
pub struct LendingPoolBankConfigureEvent {
    pub header: GroupEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub config: BankConfigOpt,
}

#[event]
pub struct LendingPoolBankConfigureOracleEvent {
    pub header: GroupEventHeader,
    pub bank: Pubkey,
    pub oracle_setup: u8,
    pub oracle: Pubkey,
}

#[event]
pub struct LendingPoolBankSetFixedOraclePriceEvent {
    pub header: GroupEventHeader,
    pub bank: Pubkey,
    pub price: WrappedI80F48,
}

#[event]
pub struct LendingPoolBankConfigureFrozenEvent {
    pub header: GroupEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub deposit_limit: u64,
    pub borrow_limit: u64,
}

#[event]
pub struct EditStakedSettingsEvent {
    pub group: Pubkey,
    pub settings: StakedSettingsEditConfig,
}

#[event]
pub struct LendingPoolBankAccrueInterestEvent {
    pub header: GroupEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub delta: u64,
    pub fees_collected: f64,
    pub insurance_collected: f64,
}

#[event]
pub struct LendingPoolBankCollectFeesEvent {
    pub header: GroupEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub group_fees_collected: f64,
    pub group_fees_outstanding: f64,
    pub insurance_fees_collected: f64,
    pub insurance_fees_outstanding: f64,
}

#[event]
pub struct LendingPoolBankHandleBankruptcyEvent {
    pub header: AccountEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub bad_debt: f64,
    pub covered_amount: f64,
    pub socialized_amount: f64,
}

// marginfi account events

#[event]
pub struct MarginfiAccountCreateEvent {
    pub header: AccountEventHeader,
}

#[event]
pub struct LendingAccountDepositEvent {
    pub header: AccountEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

#[event]
pub struct LendingAccountRepayEvent {
    pub header: AccountEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub close_balance: bool,
}

#[event]
pub struct LendingAccountBorrowEvent {
    pub header: AccountEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

#[event]
pub struct LendingAccountWithdrawEvent {
    pub header: AccountEventHeader,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub close_balance: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct LiquidationBalances {
    pub liquidatee_asset_balance: f64,
    pub liquidatee_liability_balance: f64,
    pub liquidator_asset_balance: f64,
    pub liquidator_liability_balance: f64,
}

#[event]
pub struct LendingAccountLiquidateEvent {
    pub header: AccountEventHeader,
    pub liquidatee_marginfi_account: Pubkey,
    pub liquidatee_marginfi_account_authority: Pubkey,
    pub asset_bank: Pubkey,
    pub asset_mint: Pubkey,
    pub liability_bank: Pubkey,
    pub liability_mint: Pubkey,
    pub liquidatee_pre_health: f64,
    pub liquidatee_post_health: f64,
    pub pre_balances: LiquidationBalances,
    pub post_balances: LiquidationBalances,
}

#[event]
pub struct MarginfiAccountTransferToNewAccount {
    pub header: AccountEventHeader,
    pub old_account: Pubkey,
    pub old_account_authority: Pubkey,
    pub new_account_authority: Pubkey,
}

#[event]
pub struct MarginfiAccountFreezeEvent {
    pub header: AccountEventHeader,
    pub frozen: bool,
}

#[event]
pub struct HealthPulseEvent {
    pub account: Pubkey,
    pub health_cache: HealthCache,
}

#[event]
pub struct LiquidationReceiverEvent {
    pub marginfi_account: Pubkey,
    pub liquidation_receiver: Pubkey,
    pub liquidatee_assets_seized: f64,
    pub liquidatee_liability_repaid: f64,
    pub lamps_fee_paid: u32,
}

#[event]
pub struct DeleverageEvent {
    pub marginfi_account: Pubkey,
    pub risk_admin: Pubkey,
    pub deleveragee_assets_seized: f64,
    pub deleveragee_liability_repaid: f64,
}
