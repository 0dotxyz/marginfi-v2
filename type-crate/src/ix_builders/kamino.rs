use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::KaminoConfigCompact;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`kamino_init_obligation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KaminoInitObligation {
    pub fee_payer: Pubkey,
    pub bank: Pubkey,
    pub signer_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    pub user_metadata: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub integration_acc_1: Pubkey,
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_destination_deposit_collateral: Pubkey,
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
    pub kamino_program: Pubkey,
    pub farms_program: Pubkey,
    pub collateral_token_program: Pubkey,
    pub liquidity_token_program: Pubkey,
    pub instruction_sysvar_account: Pubkey,
    pub rent: Pubkey,
    pub system_program: Pubkey,
}

impl KaminoInitObligation {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::KAMINO_INIT_OBLIGATION;
}

impl ToAccountMetas for KaminoInitObligation {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = Vec::with_capacity(23);
        metas.push(AccountMeta::new(self.fee_payer, true));
        metas.push(AccountMeta::new_readonly(self.bank, false));
        metas.push(AccountMeta::new(self.signer_token_account, false));
        metas.push(AccountMeta::new(self.liquidity_vault_authority, false));
        metas.push(AccountMeta::new(self.liquidity_vault, false));
        metas.push(AccountMeta::new(self.integration_acc_2, false));
        metas.push(AccountMeta::new(self.user_metadata, false));
        metas.push(AccountMeta::new_readonly(self.lending_market, false));
        metas.push(AccountMeta::new_readonly(
            self.lending_market_authority,
            false,
        ));
        metas.push(AccountMeta::new(self.integration_acc_1, false));
        metas.push(AccountMeta::new(self.mint, false));
        metas.push(AccountMeta::new(self.reserve_liquidity_supply, false));
        metas.push(AccountMeta::new(self.reserve_collateral_mint, false));
        metas.push(AccountMeta::new(
            self.reserve_destination_deposit_collateral,
            false,
        ));
        match self.obligation_farm_user_state {
            Some(key) => metas.push(AccountMeta::new(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        match self.reserve_farm_state {
            Some(key) => metas.push(AccountMeta::new(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas.push(AccountMeta::new_readonly(self.kamino_program, false));
        metas.push(AccountMeta::new_readonly(self.farms_program, false));
        metas.push(AccountMeta::new_readonly(
            self.collateral_token_program,
            false,
        ));
        metas.push(AccountMeta::new_readonly(
            self.liquidity_token_program,
            false,
        ));
        metas.push(AccountMeta::new_readonly(
            self.instruction_sysvar_account,
            false,
        ));
        metas.push(AccountMeta::new_readonly(self.rent, false));
        metas.push(AccountMeta::new_readonly(self.system_program, false));
        metas
    }
}

/// (permissionless) Initialize a Kamino obligation for a marginfi bank
/// * amount - In token, in native decimals. Must be >10 (i.e. 10 lamports, not 10 tokens). Lost
///   forever. Generally, try to make this the equivalent of around $1, in case Kamino ever
///   rounds small balances down to zero.
pub fn kamino_init_obligation(accounts: &KaminoInitObligation, amount: u64) -> Instruction {
    let mut data = KaminoInitObligation::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`kamino_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KaminoDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub signer_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub integration_acc_1: Pubkey,
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_destination_deposit_collateral: Pubkey,
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
    pub kamino_program: Pubkey,
    pub farms_program: Pubkey,
    pub collateral_token_program: Pubkey,
    pub liquidity_token_program: Pubkey,
    pub instruction_sysvar_account: Pubkey,
}

impl KaminoDeposit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::KAMINO_DEPOSIT;
}

impl ToAccountMetas for KaminoDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = Vec::with_capacity(22);
        metas.push(AccountMeta::new_readonly(self.group, false));
        metas.push(AccountMeta::new(self.marginfi_account, false));
        metas.push(AccountMeta::new_readonly(self.authority, true));
        metas.push(AccountMeta::new(self.bank, false));
        metas.push(AccountMeta::new(self.signer_token_account, false));
        metas.push(AccountMeta::new(self.liquidity_vault_authority, false));
        metas.push(AccountMeta::new(self.liquidity_vault, false));
        metas.push(AccountMeta::new(self.integration_acc_2, false));
        metas.push(AccountMeta::new_readonly(self.lending_market, false));
        metas.push(AccountMeta::new_readonly(
            self.lending_market_authority,
            false,
        ));
        metas.push(AccountMeta::new(self.integration_acc_1, false));
        metas.push(AccountMeta::new_readonly(self.mint, false));
        metas.push(AccountMeta::new(self.reserve_liquidity_supply, false));
        metas.push(AccountMeta::new(self.reserve_collateral_mint, false));
        metas.push(AccountMeta::new(
            self.reserve_destination_deposit_collateral,
            false,
        ));
        match self.obligation_farm_user_state {
            Some(key) => metas.push(AccountMeta::new(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        match self.reserve_farm_state {
            Some(key) => metas.push(AccountMeta::new(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas.push(AccountMeta::new_readonly(self.kamino_program, false));
        metas.push(AccountMeta::new_readonly(self.farms_program, false));
        metas.push(AccountMeta::new_readonly(
            self.collateral_token_program,
            false,
        ));
        metas.push(AccountMeta::new_readonly(
            self.liquidity_token_program,
            false,
        ));
        metas.push(AccountMeta::new_readonly(
            self.instruction_sysvar_account,
            false,
        ));
        metas
    }
}

/// (user) Deposit into a Kamino pool through a marginfi account
/// * amount - in the liquidity token (e.g. if there is a Kamino USDC bank, pass the amount of
///   USDC desired), in native decimals.
pub fn kamino_deposit(
    accounts: &KaminoDeposit,
    amount: u64,
    refresh_reserve: Option<bool>,
) -> Instruction {
    let mut data = KaminoDeposit::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    refresh_reserve.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`kamino_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KaminoWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub destination_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub integration_acc_1: Pubkey,
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_source_collateral: Pubkey,
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
    pub kamino_program: Pubkey,
    pub farms_program: Pubkey,
    pub collateral_token_program: Pubkey,
    pub liquidity_token_program: Pubkey,
    pub instruction_sysvar_account: Pubkey,
}

impl KaminoWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::KAMINO_WITHDRAW;
}

impl ToAccountMetas for KaminoWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = Vec::with_capacity(22);
        metas.push(AccountMeta::new_readonly(self.group, false));
        metas.push(AccountMeta::new(self.marginfi_account, false));
        metas.push(AccountMeta::new_readonly(self.authority, true));
        metas.push(AccountMeta::new(self.bank, false));
        metas.push(AccountMeta::new(self.destination_token_account, false));
        metas.push(AccountMeta::new(self.liquidity_vault_authority, false));
        metas.push(AccountMeta::new(self.liquidity_vault, false));
        metas.push(AccountMeta::new(self.integration_acc_2, false));
        metas.push(AccountMeta::new_readonly(self.lending_market, false));
        metas.push(AccountMeta::new_readonly(
            self.lending_market_authority,
            false,
        ));
        metas.push(AccountMeta::new(self.integration_acc_1, false));
        metas.push(AccountMeta::new(self.mint, false));
        metas.push(AccountMeta::new(self.reserve_liquidity_supply, false));
        metas.push(AccountMeta::new(self.reserve_collateral_mint, false));
        metas.push(AccountMeta::new(self.reserve_source_collateral, false));
        match self.obligation_farm_user_state {
            Some(key) => metas.push(AccountMeta::new(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        match self.reserve_farm_state {
            Some(key) => metas.push(AccountMeta::new(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas.push(AccountMeta::new_readonly(self.kamino_program, false));
        metas.push(AccountMeta::new_readonly(self.farms_program, false));
        metas.push(AccountMeta::new_readonly(
            self.collateral_token_program,
            false,
        ));
        metas.push(AccountMeta::new_readonly(
            self.liquidity_token_program,
            false,
        ));
        metas.push(AccountMeta::new_readonly(
            self.instruction_sysvar_account,
            false,
        ));
        metas
    }
}

/// (user) Withdraw from a Kamino pool through a marginfi account
/// * amount - in the collateral token (NOT liquidity token), in native decimals. Must convert
///   from collateral to liquidity token amounts using the current exchange rate.
/// * if group rate limits are enabled, include the withdrawn bank's oracle group in
///   `remaining_accounts`
/// * flags - optional bitflags:
///   - bit 0 (`0x01`): withdraw all
///   - bit 1 (`0x02`): refresh reserve via batch refresh
pub fn kamino_withdraw(accounts: &KaminoWithdraw, amount: u64, flags: Option<u8>) -> Instruction {
    let mut data = KaminoWithdraw::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    flags.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`kamino_harvest_reward`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KaminoHarvestReward {
    pub bank: Pubkey,
    pub fee_state: Pubkey,
    pub destination_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub user_state: Pubkey,
    pub farm_state: Pubkey,
    pub global_config: Pubkey,
    pub reward_mint: Pubkey,
    pub user_reward_ata: Pubkey,
    pub rewards_vault: Pubkey,
    pub rewards_treasury_vault: Pubkey,
    pub farm_vaults_authority: Pubkey,
    pub scope_prices: Option<Pubkey>,
    pub farms_program: Pubkey,
    pub token_program: Pubkey,
}

impl KaminoHarvestReward {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::KAMINO_HARVEST_REWARD;
}

impl ToAccountMetas for KaminoHarvestReward {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = Vec::with_capacity(15);
        metas.push(AccountMeta::new_readonly(self.bank, false));
        metas.push(AccountMeta::new_readonly(self.fee_state, false));
        metas.push(AccountMeta::new(self.destination_token_account, false));
        metas.push(AccountMeta::new(self.liquidity_vault_authority, false));
        metas.push(AccountMeta::new(self.user_state, false));
        metas.push(AccountMeta::new(self.farm_state, false));
        metas.push(AccountMeta::new_readonly(self.global_config, false));
        metas.push(AccountMeta::new_readonly(self.reward_mint, false));
        metas.push(AccountMeta::new(self.user_reward_ata, false));
        metas.push(AccountMeta::new(self.rewards_vault, false));
        metas.push(AccountMeta::new(self.rewards_treasury_vault, false));
        metas.push(AccountMeta::new_readonly(self.farm_vaults_authority, false));
        match self.scope_prices {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas.push(AccountMeta::new_readonly(self.farms_program, false));
        metas.push(AccountMeta::new_readonly(self.token_program, false));
        metas
    }
}

/// (permissionless) Harvest the specified reward index from the Kamino Farm attached to this
/// bank. Rewards are always sent to the global fee wallet's canonical ATA.
///
/// * `reward_index` — index of the reward token in the Kamino Farm's reward list
pub fn kamino_harvest_reward(accounts: &KaminoHarvestReward, reward_index: u64) -> Instruction {
    let mut data = KaminoHarvestReward::DISCRIMINATOR.to_vec();
    reward_index.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_add_bank_kamino`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankKamino {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub fee_payer: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub integration_acc_1: Pubkey,
    pub integration_acc_2: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl LendingPoolAddBankKamino {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_ADD_BANK_KAMINO;
}

impl ToAccountMetas for LendingPoolAddBankKamino {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.integration_acc_1, false),
            AccountMeta::new_readonly(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (group admin only) Add a Kamino bank to the group. `remaining_accounts` must contain the
/// configured oracle feed followed by `integration_acc_1`, the Kamino reserve account.
pub fn lending_pool_add_bank_kamino(
    accounts: &LendingPoolAddBankKamino,
    bank_config: KaminoConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankKamino::DISCRIMINATOR.to_vec();
    bank_config.serialize(&mut data).unwrap();
    bank_seed.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}
