use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::DriftConfigCompact;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`drift_init_user`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftInitUser {
    pub fee_payer: Pubkey,
    pub signer_token_account: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub mint: Pubkey,
    pub integration_acc_3: Pubkey,
    pub integration_acc_2: Pubkey,
    pub drift_state: Pubkey,
    pub integration_acc_1: Pubkey,
    pub drift_spot_market_vault: Pubkey,
    pub drift_oracle: Option<Pubkey>,
    pub drift_program: Pubkey,
    pub token_program: Pubkey,
    pub rent: Pubkey,
    pub system_program: Pubkey,
}

impl DriftInitUser {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::DRIFT_INIT_USER;
}

impl ToAccountMetas for DriftInitUser {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = Vec::with_capacity(16);
        metas.push(AccountMeta::new(self.fee_payer, true));
        metas.push(AccountMeta::new(self.signer_token_account, false));
        metas.push(AccountMeta::new_readonly(self.bank, false));
        metas.push(AccountMeta::new_readonly(
            self.liquidity_vault_authority,
            false,
        ));
        metas.push(AccountMeta::new(self.liquidity_vault, false));
        metas.push(AccountMeta::new(self.mint, false));
        metas.push(AccountMeta::new(self.integration_acc_3, false));
        metas.push(AccountMeta::new(self.integration_acc_2, false));
        metas.push(AccountMeta::new(self.drift_state, false));
        metas.push(AccountMeta::new(self.integration_acc_1, false));
        metas.push(AccountMeta::new(self.drift_spot_market_vault, false));
        match self.drift_oracle {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas.push(AccountMeta::new_readonly(self.drift_program, false));
        metas.push(AccountMeta::new_readonly(self.token_program, false));
        metas.push(AccountMeta::new_readonly(self.rent, false));
        metas.push(AccountMeta::new_readonly(self.system_program, false));
        metas
    }
}

/// (permissionless) Initialize a Drift user and user stats for a marginfi bank
/// Creates user with sub_account_id = 0 and empty name
/// Requires a minimum deposit to ensure the account remains active
/// * amount - minimum deposit amount (at least 10 units) in native decimals
pub fn drift_init_user(accounts: &DriftInitUser, amount: u64) -> Instruction {
    let mut data = DriftInitUser::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`drift_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub drift_oracle: Option<Pubkey>,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub signer_token_account: Pubkey,
    pub drift_state: Pubkey,
    pub integration_acc_2: Pubkey,
    pub integration_acc_3: Pubkey,
    pub integration_acc_1: Pubkey,
    pub drift_spot_market_vault: Pubkey,
    pub mint: Pubkey,
    pub drift_program: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl DriftDeposit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::DRIFT_DEPOSIT;
}

impl ToAccountMetas for DriftDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = Vec::with_capacity(17);
        metas.push(AccountMeta::new_readonly(self.group, false));
        metas.push(AccountMeta::new(self.marginfi_account, false));
        metas.push(AccountMeta::new_readonly(self.authority, true));
        metas.push(AccountMeta::new(self.bank, false));
        match self.drift_oracle {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas.push(AccountMeta::new_readonly(
            self.liquidity_vault_authority,
            false,
        ));
        metas.push(AccountMeta::new(self.liquidity_vault, false));
        metas.push(AccountMeta::new(self.signer_token_account, false));
        metas.push(AccountMeta::new_readonly(self.drift_state, false));
        metas.push(AccountMeta::new(self.integration_acc_2, false));
        metas.push(AccountMeta::new(self.integration_acc_3, false));
        metas.push(AccountMeta::new(self.integration_acc_1, false));
        metas.push(AccountMeta::new(self.drift_spot_market_vault, false));
        metas.push(AccountMeta::new_readonly(self.mint, false));
        metas.push(AccountMeta::new_readonly(self.drift_program, false));
        metas.push(AccountMeta::new_readonly(self.token_program, false));
        metas.push(AccountMeta::new_readonly(self.system_program, false));
        metas
    }
}

/// (user) Deposit into a Drift spot market through a marginfi account
/// * amount - in the underlying token (e.g., USDC), in native decimals
pub fn drift_deposit(accounts: &DriftDeposit, amount: u64) -> Instruction {
    let mut data = DriftDeposit::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`drift_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub drift_oracle: Option<Pubkey>,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub destination_token_account: Pubkey,
    pub drift_state: Pubkey,
    pub integration_acc_2: Pubkey,
    pub integration_acc_3: Pubkey,
    pub integration_acc_1: Pubkey,
    pub drift_spot_market_vault: Pubkey,
    pub drift_reward_oracle: Option<Pubkey>,
    pub drift_reward_spot_market: Option<Pubkey>,
    pub drift_reward_mint: Option<Pubkey>,
    pub drift_reward_oracle_2: Option<Pubkey>,
    pub drift_reward_spot_market_2: Option<Pubkey>,
    pub drift_reward_mint_2: Option<Pubkey>,
    pub drift_signer: Pubkey,
    pub mint: Pubkey,
    pub drift_program: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl DriftWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::DRIFT_WITHDRAW;
}

impl ToAccountMetas for DriftWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = Vec::with_capacity(24);
        metas.push(AccountMeta::new_readonly(self.group, false));
        metas.push(AccountMeta::new(self.marginfi_account, false));
        metas.push(AccountMeta::new_readonly(self.authority, true));
        metas.push(AccountMeta::new(self.bank, false));
        match self.drift_oracle {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas.push(AccountMeta::new_readonly(
            self.liquidity_vault_authority,
            false,
        ));
        metas.push(AccountMeta::new(self.liquidity_vault, false));
        metas.push(AccountMeta::new(self.destination_token_account, false));
        metas.push(AccountMeta::new_readonly(self.drift_state, false));
        metas.push(AccountMeta::new(self.integration_acc_2, false));
        metas.push(AccountMeta::new(self.integration_acc_3, false));
        metas.push(AccountMeta::new(self.integration_acc_1, false));
        metas.push(AccountMeta::new(self.drift_spot_market_vault, false));
        match self.drift_reward_oracle {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        match self.drift_reward_spot_market {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        match self.drift_reward_mint {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        match self.drift_reward_oracle_2 {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        match self.drift_reward_spot_market_2 {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        match self.drift_reward_mint_2 {
            Some(key) => metas.push(AccountMeta::new_readonly(key, false)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas.push(AccountMeta::new_readonly(self.drift_signer, false));
        metas.push(AccountMeta::new_readonly(self.mint, false));
        metas.push(AccountMeta::new_readonly(self.drift_program, false));
        metas.push(AccountMeta::new_readonly(self.token_program, false));
        metas.push(AccountMeta::new_readonly(self.system_program, false));
        metas
    }
}

/// (user) Withdraw from a Drift spot market through a marginfi account
/// * amount - in the underlying token (e.g., USDC), in native decimals
/// * if group rate limits are enabled, include the withdrawn bank's oracle group in
///   `remaining_accounts`
/// * withdraw_all - if true, withdraws entire position
pub fn drift_withdraw(
    accounts: &DriftWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    let mut data = DriftWithdraw::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    withdraw_all.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`drift_harvest_reward`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftHarvestReward {
    pub bank: Pubkey,
    pub fee_state: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub intermediary_token_account: Pubkey,
    pub destination_token_account: Pubkey,
    pub drift_state: Pubkey,
    pub integration_acc_2: Pubkey,
    pub integration_acc_3: Pubkey,
    pub harvest_drift_spot_market: Pubkey,
    pub harvest_drift_spot_market_vault: Pubkey,
    pub drift_signer: Pubkey,
    pub reward_mint: Pubkey,
    pub drift_program: Pubkey,
    pub token_program: Pubkey,
}

impl DriftHarvestReward {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::DRIFT_HARVEST_REWARD;
}

impl ToAccountMetas for DriftHarvestReward {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.intermediary_token_account, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.drift_state, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new(self.integration_acc_3, false),
            AccountMeta::new(self.harvest_drift_spot_market, false),
            AccountMeta::new(self.harvest_drift_spot_market_vault, false),
            AccountMeta::new_readonly(self.drift_signer, false),
            AccountMeta::new_readonly(self.reward_mint, false),
            AccountMeta::new_readonly(self.drift_program, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (permissionless) Harvest rewards from admin deposits in Drift spot markets.
/// Rewards are always sent to the global fee wallet's canonical ATA.
/// The harvest spot market must be different from the bank's main drift spot market.
pub fn drift_harvest_reward(accounts: &DriftHarvestReward) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: DriftHarvestReward::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`drift_claim_bad_debt`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriftClaimBadDebt {
    pub payer: Pubkey,
    pub bank: Pubkey,
    pub fee_state: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub integration_acc_2: Pubkey,
    pub integration_acc_3: Pubkey,
    pub distributor: Pubkey,
    pub claim_status: Pubkey,
    pub from: Pubkey,
    pub claim_mint: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub claimant_token_account: Pubkey,
    pub destination_token_account: Pubkey,
    pub merkle_distributor_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl DriftClaimBadDebt {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::DRIFT_CLAIM_BAD_DEBT;
}

impl ToAccountMetas for DriftClaimBadDebt {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new_readonly(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.integration_acc_3, false),
            AccountMeta::new(self.distributor, false),
            AccountMeta::new(self.claim_status, false),
            AccountMeta::new(self.from, false),
            AccountMeta::new_readonly(self.claim_mint, false),
            AccountMeta::new_readonly(self.global_fee_wallet, false),
            AccountMeta::new(self.claimant_token_account, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.merkle_distributor_program, false),
            AccountMeta::new_readonly(self.associated_token_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (permissionless) Claim a Drift bad-debt portal allocation for a Drift bank.
/// The merkle claimant is the bank's liquidity_vault_authority PDA, and claimed tokens are
/// swept to the global fee wallet's canonical ATA.
pub fn drift_claim_bad_debt(
    accounts: &DriftClaimBadDebt,
    amount: u64,
    proof: Vec<[u8; 32]>,
) -> Instruction {
    let mut data = DriftClaimBadDebt::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    proof.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_add_bank_drift`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankDrift {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub fee_payer: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub integration_acc_1: Pubkey,
    pub integration_acc_2: Pubkey,
    pub integration_acc_3: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl LendingPoolAddBankDrift {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_ADD_BANK_DRIFT;
}

impl ToAccountMetas for LendingPoolAddBankDrift {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.integration_acc_1, false),
            AccountMeta::new_readonly(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.integration_acc_3, false),
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

/// (group admin only) Add a Drift bank to the group.
pub fn lending_pool_add_bank_drift(
    accounts: &LendingPoolAddBankDrift,
    bank_config: DriftConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankDrift::DISCRIMINATOR.to_vec();
    bank_config.serialize(&mut data).unwrap();
    bank_seed.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}
