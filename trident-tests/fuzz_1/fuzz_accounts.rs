use trident_fuzz::fuzzing::*;

/// Storage for all account addresses used in fuzz testing.
///
/// This struct serves as a centralized repository for account addresses,
/// enabling their reuse across different instruction flows and test scenarios.
///
/// Docs: https://ackee.xyz/trident/docs/latest/trident-api-macro/trident-types/fuzz-accounts/
#[derive(Default)]
pub struct AccountAddresses {
    pub marginfi_group: AddressStorage,

    pub marginfi_account: AddressStorage,

    pub authority: AddressStorage,

    pub fee_payer: AddressStorage,

    pub instructions_sysvar: AddressStorage,

    pub system_program: AddressStorage,

    pub marginfi_program: AddressStorage,

    pub call_log: AddressStorage,

    pub payer: AddressStorage,

    pub group: AddressStorage,

    pub signer: AddressStorage,

    pub bank: AddressStorage,

    pub liquidity_vault: AddressStorage,

    pub insurance_vault: AddressStorage,

    pub insurance_vault_authority: AddressStorage,

    pub token_program: AddressStorage,

    pub pool_auth: AddressStorage,

    pub mint_a: AddressStorage,

    pub mint_b: AddressStorage,

    pub pool_a: AddressStorage,

    pub pool_b: AddressStorage,

    pub liquidation_record: AddressStorage,

    pub liquidation_receiver: AddressStorage,

    pub user_authority: AddressStorage,

    pub source_a: AddressStorage,

    pub destination_b: AddressStorage,

    pub target: AddressStorage,

    pub global_fee_admin: AddressStorage,

    pub fee_state: AddressStorage,

    pub admin: AddressStorage,

    pub drift_oracle: AddressStorage,

    pub liquidity_vault_authority: AddressStorage,

    pub signer_token_account: AddressStorage,

    pub drift_state: AddressStorage,

    pub integration_acc_2: AddressStorage,

    pub integration_acc_3: AddressStorage,

    pub integration_acc_1: AddressStorage,

    pub drift_spot_market_vault: AddressStorage,

    pub mint: AddressStorage,

    pub drift_program: AddressStorage,

    pub intermediary_token_account: AddressStorage,

    pub destination_token_account: AddressStorage,

    pub harvest_drift_spot_market: AddressStorage,

    pub harvest_drift_spot_market_vault: AddressStorage,

    pub drift_signer: AddressStorage,

    pub reward_mint: AddressStorage,

    pub rent: AddressStorage,

    pub drift_reward_oracle: AddressStorage,

    pub drift_reward_spot_market: AddressStorage,

    pub drift_reward_mint: AddressStorage,

    pub drift_reward_oracle_2: AddressStorage,

    pub drift_reward_spot_market_2: AddressStorage,

    pub drift_reward_mint_2: AddressStorage,

    pub staked_settings: AddressStorage,

    pub risk_admin: AddressStorage,

    pub global_fee_wallet: AddressStorage,

    pub metadata: AddressStorage,

    pub f_token_mint: AddressStorage,

    pub lending_admin: AddressStorage,

    pub supply_token_reserves_liquidity: AddressStorage,

    pub lending_supply_position_on_liquidity: AddressStorage,

    pub rate_model: AddressStorage,

    pub vault: AddressStorage,

    pub liquidity: AddressStorage,

    pub liquidity_program: AddressStorage,

    pub rewards_rate_model: AddressStorage,

    pub juplend_program: AddressStorage,

    pub associated_token_program: AddressStorage,

    pub claim_account: AddressStorage,

    pub lending_market: AddressStorage,

    pub lending_market_authority: AddressStorage,

    pub reserve_liquidity_supply: AddressStorage,

    pub reserve_collateral_mint: AddressStorage,

    pub reserve_destination_deposit_collateral: AddressStorage,

    pub obligation_farm_user_state: AddressStorage,

    pub reserve_farm_state: AddressStorage,

    pub kamino_program: AddressStorage,

    pub farms_program: AddressStorage,

    pub collateral_token_program: AddressStorage,

    pub liquidity_token_program: AddressStorage,

    pub instruction_sysvar_account: AddressStorage,

    pub user_state: AddressStorage,

    pub farm_state: AddressStorage,

    pub global_config: AddressStorage,

    pub user_reward_ata: AddressStorage,

    pub rewards_vault: AddressStorage,

    pub rewards_treasury_vault: AddressStorage,

    pub farm_vaults_authority: AddressStorage,

    pub scope_prices: AddressStorage,

    pub user_metadata: AddressStorage,

    pub pyth_oracle: AddressStorage,

    pub switchboard_price_oracle: AddressStorage,

    pub switchboard_twap_oracle: AddressStorage,

    pub reserve_liquidity_mint: AddressStorage,

    pub reserve_source_collateral: AddressStorage,

    pub bank_liquidity_vault_authority: AddressStorage,

    pub asset_bank: AddressStorage,

    pub liab_bank: AddressStorage,

    pub liquidator_marginfi_account: AddressStorage,

    pub liquidatee_marginfi_account: AddressStorage,

    pub bank_liquidity_vault: AddressStorage,

    pub bank_insurance_vault: AddressStorage,

    pub ixs_sysvar: AddressStorage,

    pub emissions_mint: AddressStorage,

    pub emissions_auth: AddressStorage,

    pub emissions_vault: AddressStorage,

    pub destination_account: AddressStorage,

    pub bank_mint: AddressStorage,

    pub fee_vault_authority: AddressStorage,

    pub fee_vault: AddressStorage,

    pub sol_pool: AddressStorage,

    pub stake_pool: AddressStorage,

    pub source_bank: AddressStorage,

    pub copy_from_bank: AddressStorage,

    pub copy_to_bank: AddressStorage,

    pub fee_ata: AddressStorage,

    pub emode_admin: AddressStorage,

    pub delegate_curve_admin: AddressStorage,

    pub delegate_limit_admin: AddressStorage,

    pub delegate_emissions_admin: AddressStorage,

    pub emissions_token_account: AddressStorage,

    pub emissions_funding_account: AddressStorage,

    pub dst_token_account: AddressStorage,

    pub fees_destination_account: AddressStorage,

    pub order: AddressStorage,

    pub fee_recipient: AddressStorage,

    pub executor: AddressStorage,

    pub execute_record: AddressStorage,

    pub instruction_sysvar: AddressStorage,

    pub reserve_collateral_supply: AddressStorage,

    pub user_collateral: AddressStorage,

    pub pyth_price: AddressStorage,

    pub switchboard_feed: AddressStorage,

    pub solend_program: AddressStorage,

    pub old_marginfi_account: AddressStorage,

    pub new_marginfi_account: AddressStorage,

    pub new_authority: AddressStorage,

    pub metadata_admin: AddressStorage,
}
