//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::types::KaminoConfigCompact;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`kamino_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KaminoDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    /// Owned by authority, the source account for the token deposit.
    pub signer_token_account: Pubkey,
    /// The bank's liquidity vault authority, which owns the Kamino obligation. Note: Kamino needs
    /// this to be mut because `deposit` might return the rent here
    pub liquidity_vault_authority: Pubkey,
    /// Used as an intermediary to deposit token into Kamino
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    /// The Kamino reserve that holds liquidity
    pub integration_acc_1: Pubkey,
    /// Bank's liquidity token mint (e.g., USDC). Kamino calls this the `reserve_liquidity_mint`
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    /// The reserve's mint for tokenized representations of Kamino deposits.
    pub reserve_collateral_mint: Pubkey,
    /// The reserve's destination for tokenized representations of deposits. Note: the
    /// `reserve_collateral_mint` will mint tokens directly to this account.
    pub reserve_destination_deposit_collateral: Pubkey,
    /// Required if the Kamino reserve has an active farm.
    pub obligation_farm_user_state: Option<Pubkey>,
    /// Required if the Kamino reserve has an active farm.
    pub reserve_farm_state: Option<Pubkey>,
    pub kamino_program: Pubkey,
    /// Farms program for Kamino staking functionality
    pub farms_program: Pubkey,
    pub collateral_token_program: Pubkey,
    pub liquidity_token_program: Pubkey,
    pub instruction_sysvar_account: Pubkey,
}
impl KaminoDeposit {
    pub const DISCRIMINATOR: [u8; 8] = [237, 8, 188, 187, 115, 99, 49, 85];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.lending_market, false),
            AccountMeta::new_readonly(self.lending_market_authority, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.reserve_liquidity_supply, false),
            AccountMeta::new(self.reserve_collateral_mint, false),
            AccountMeta::new(self.reserve_destination_deposit_collateral, false),
            match self.obligation_farm_user_state {
                Some(key) => AccountMeta::new(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            match self.reserve_farm_state {
                Some(key) => AccountMeta::new(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new_readonly(self.kamino_program, false),
            AccountMeta::new_readonly(self.farms_program, false),
            AccountMeta::new_readonly(self.collateral_token_program, false),
            AccountMeta::new_readonly(self.liquidity_token_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar_account, false),
        ]
    }
}
impl ToAccountMetas for KaminoDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Deposit into a Kamino pool through a marginfi account
/// * amount - in the liquidity token (e.g. if there is a Kamino USDC bank, pass the amount of
/// USDC desired), in native decimals.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn kamino_deposit(
    accounts: &KaminoDeposit,
    amount: u64,
    refresh_reserve: Option<bool>,
) -> Instruction {
    kamino_deposit_with_program_id(crate::ID, accounts, amount, refresh_reserve)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn kamino_deposit_with_program_id(
    program_id: Pubkey,
    accounts: &KaminoDeposit,
    amount: u64,
    refresh_reserve: Option<bool>,
) -> Instruction {
    let mut data = KaminoDeposit::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    refresh_reserve
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`kamino_harvest_reward`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KaminoHarvestReward {
    pub bank: Pubkey,
    /// Global fee state that contains the global_fee_admin
    pub fee_state: Pubkey,
    /// Destination token account must be owned by the global fee admin
    pub destination_token_account: Pubkey,
    /// The bank's liquidity vault authority, which owns the Kamino obligation.
    pub liquidity_vault_authority: Pubkey,
    pub user_state: Pubkey,
    pub farm_state: Pubkey,
    pub global_config: Pubkey,
    pub reward_mint: Pubkey,
    /// An initialized ATA of type reward mint owned by liquidity vault
    pub user_reward_ata: Pubkey,
    pub rewards_vault: Pubkey,
    pub rewards_treasury_vault: Pubkey,
    pub farm_vaults_authority: Pubkey,
    pub scope_prices: Option<Pubkey>,
    pub farms_program: Pubkey,
    pub token_program: Pubkey,
}
impl KaminoHarvestReward {
    pub const DISCRIMINATOR: [u8; 8] = [163, 202, 248, 141, 106, 20, 116, 5];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new(self.user_state, false),
            AccountMeta::new(self.farm_state, false),
            AccountMeta::new_readonly(self.global_config, false),
            AccountMeta::new_readonly(self.reward_mint, false),
            AccountMeta::new(self.user_reward_ata, false),
            AccountMeta::new(self.rewards_vault, false),
            AccountMeta::new(self.rewards_treasury_vault, false),
            AccountMeta::new_readonly(self.farm_vaults_authority, false),
            match self.scope_prices {
                Some(key) => AccountMeta::new_readonly(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new_readonly(self.farms_program, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}
impl ToAccountMetas for KaminoHarvestReward {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Harvest the specified reward index from the Kamino Farm attached to this
/// bank. Rewards are always sent to the global fee wallet's canonical ATA.
///
/// * `reward_index` — index of the reward token in the Kamino Farm's reward list
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn kamino_harvest_reward(accounts: &KaminoHarvestReward, reward_index: u64) -> Instruction {
    kamino_harvest_reward_with_program_id(crate::ID, accounts, reward_index)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn kamino_harvest_reward_with_program_id(
    program_id: Pubkey,
    accounts: &KaminoHarvestReward,
    reward_index: u64,
) -> Instruction {
    let mut data = KaminoHarvestReward::DISCRIMINATOR.to_vec();
    reward_index
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`kamino_init_obligation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KaminoInitObligation {
    /// Pays to init the obligation and pays a nominal amount to ensure the obligation has a
    /// non-zero balance.
    pub fee_payer: Pubkey,
    pub bank: Pubkey,
    /// The fee payer must provide a nominal amount of bank tokens so the obligation is not empty.
    /// This amount is irrecoverable and and will prevent the obligation from ever being closed,
    /// even if the bank is otherwise empty (Kamino normally closes empty obligations automatically)
    pub signer_token_account: Pubkey,
    /// The liquidity vault authority (PDA that will own the Kamino obligation). Note: Kamino needs
    /// this to be mut because `deposit` might return the rent here
    pub liquidity_vault_authority: Pubkey,
    /// Used as an intermediary to deposit a nominal amount of token into the obligation.
    pub liquidity_vault: Pubkey,
    /// The obligation account to be created. Note that the key was already derived when
    /// initializing the bank, and this must match the obligation recorded at that time.
    pub integration_acc_2: Pubkey,
    pub user_metadata: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub integration_acc_1: Pubkey,
    /// Bank's liquidity token mint (e.g., USDC). Kamino calls this the `reserve_liquidity_mint`
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    /// The reserve's mint for tokenized representations of Kamino deposits.
    pub reserve_collateral_mint: Pubkey,
    /// The reserve's destination for tokenized representations of deposits. Note: the
    /// `reserve_collateral_mint` will mint tokens directly to this account.
    pub reserve_destination_deposit_collateral: Pubkey,
    /// Required if the Kamino reserve has an active farm.
    pub obligation_farm_user_state: Option<Pubkey>,
    /// Required if the Kamino reserve has an active farm.
    pub reserve_farm_state: Option<Pubkey>,
    pub kamino_program: Pubkey,
    /// Farms program for Kamino staking functionality
    pub farms_program: Pubkey,
    /// Note: the collateral token always uses Token classic, never Token22.
    pub collateral_token_program: Pubkey,
    /// Note: Kamino does not have full Token22 support, certain Token22 features are disallowed.
    /// Expect this to update over time. Check with the Kamino source.
    pub liquidity_token_program: Pubkey,
    pub instruction_sysvar_account: Pubkey,
    pub rent: Pubkey,
    pub system_program: Pubkey,
}
impl KaminoInitObligation {
    pub const DISCRIMINATOR: [u8; 8] = [253, 177, 160, 225, 70, 156, 217, 109];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new(self.user_metadata, false),
            AccountMeta::new_readonly(self.lending_market, false),
            AccountMeta::new_readonly(self.lending_market_authority, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.mint, false),
            AccountMeta::new(self.reserve_liquidity_supply, false),
            AccountMeta::new(self.reserve_collateral_mint, false),
            AccountMeta::new(self.reserve_destination_deposit_collateral, false),
            match self.obligation_farm_user_state {
                Some(key) => AccountMeta::new(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            match self.reserve_farm_state {
                Some(key) => AccountMeta::new(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new_readonly(self.kamino_program, false),
            AccountMeta::new_readonly(self.farms_program, false),
            AccountMeta::new_readonly(self.collateral_token_program, false),
            AccountMeta::new_readonly(self.liquidity_token_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar_account, false),
            AccountMeta::new_readonly(self.rent, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for KaminoInitObligation {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Initialize a Kamino obligation for a marginfi bank
/// * amount - In token, in native decimals. Must be >10 (i.e. 10 lamports, not 10 tokens). Lost
/// forever. Generally, try to make this the equivalent of around $1, in case Kamino ever
/// rounds small balances down to zero.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn kamino_init_obligation(accounts: &KaminoInitObligation, amount: u64) -> Instruction {
    kamino_init_obligation_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn kamino_init_obligation_with_program_id(
    program_id: Pubkey,
    accounts: &KaminoInitObligation,
    amount: u64,
) -> Instruction {
    let mut data = KaminoInitObligation::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`kamino_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KaminoWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    /// Token account that will receive the withdrawn tokens. Mint/owner are validated by the
    /// SPL transfer; the caller controls the destination.
    pub destination_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    /// The Kamino lending market
    pub lending_market: Pubkey,
    /// The Kamino lending market authority
    pub lending_market_authority: Pubkey,
    /// The Kamino reserve that holds liquidity
    pub integration_acc_1: Pubkey,
    /// The liquidity token mint (e.g., USDC)
    /// Needs serde to get the mint decimals for transfer checked
    pub mint: Pubkey,
    /// The reserve's liquidity supply account
    pub reserve_liquidity_supply: Pubkey,
    /// The reserve's collateral mint
    pub reserve_collateral_mint: Pubkey,
    /// The reserve's source for collateral tokens
    pub reserve_source_collateral: Pubkey,
    /// Optional farms accounts for Kamino staking functionality
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
    pub kamino_program: Pubkey,
    /// Farms program for Kamino staking functionality
    pub farms_program: Pubkey,
    /// The token program for the collateral token
    pub collateral_token_program: Pubkey,
    /// The token program for the liquidity token
    pub liquidity_token_program: Pubkey,
    /// Used by kamino validate CPI calls
    pub instruction_sysvar_account: Pubkey,
}
impl KaminoWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = [199, 101, 41, 45, 213, 98, 224, 200];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.lending_market, false),
            AccountMeta::new_readonly(self.lending_market_authority, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.mint, false),
            AccountMeta::new(self.reserve_liquidity_supply, false),
            AccountMeta::new(self.reserve_collateral_mint, false),
            AccountMeta::new(self.reserve_source_collateral, false),
            match self.obligation_farm_user_state {
                Some(key) => AccountMeta::new(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            match self.reserve_farm_state {
                Some(key) => AccountMeta::new(key, false),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new_readonly(self.kamino_program, false),
            AccountMeta::new_readonly(self.farms_program, false),
            AccountMeta::new_readonly(self.collateral_token_program, false),
            AccountMeta::new_readonly(self.liquidity_token_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar_account, false),
        ]
    }
}
impl ToAccountMetas for KaminoWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Withdraw from a Kamino pool through a marginfi account
/// * amount - in the collateral token (NOT liquidity token), in native decimals. Must convert
/// from collateral to liquidity token amounts using the current exchange rate.
/// * if group rate limits are enabled, include the withdrawn bank's oracle group in
/// `remaining_accounts`
/// * flags - optional bitflags:
/// - bit 0 (`0x01`): withdraw all
/// - bit 1 (`0x02`): refresh reserve via batch refresh
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn kamino_withdraw(accounts: &KaminoWithdraw, amount: u64, flags: Option<u8>) -> Instruction {
    kamino_withdraw_with_program_id(crate::ID, accounts, amount, flags)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn kamino_withdraw_with_program_id(
    program_id: Pubkey,
    accounts: &KaminoWithdraw,
    amount: u64,
    flags: Option<u8>,
) -> Instruction {
    let mut data = KaminoWithdraw::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    flags
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_add_bank_kamino`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankKamino {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub fee_payer: Pubkey,
    /// Must match the mint used by the Kamino reserve (integration_acc_1), Kamino calls this the
    /// `reserve_liquidity_mint` aka `liquidity.mint_pubkey`
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub integration_acc_1: Pubkey,
    /// Note: not yet initialized in this instruction, run `init_obligation` after.
    pub integration_acc_2: Pubkey,
    /// Will be authority of the bank's Kamino obligation (integration_acc_2). Note: When
    /// depositing/withdrawing Kamino assets, the source/destination must also be owned by the
    /// obligation authority.
    /// Kamino assets, the source/destination must also be owned by the obligation authority. This
    /// account owns the `liquidity_vault`, and thus acts as intermediary for deposits/withdraws
    pub liquidity_vault_authority: Pubkey,
    /// For Kamino banks, the `liquidity_vault` never holds assets, but is instead used as an
    /// intermediary when depositing/withdrawing, e.g., withdrawn funds move from Kamino -> here ->
    /// the user's token account.
    pub liquidity_vault: Pubkey,
    /// Note: Currently does nothing.
    pub insurance_vault_authority: Pubkey,
    /// Note: Currently does nothing.
    pub insurance_vault: Pubkey,
    /// Note: Currently does nothing.
    pub fee_vault_authority: Pubkey,
    /// Note: Currently does nothing.
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolAddBankKamino {
    pub const DISCRIMINATOR: [u8; 8] = [118, 53, 16, 243, 255, 245, 149, 241];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
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
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolAddBankKamino {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (governance_admin only) Add a Kamino bank to the group. Pass the oracle and reserve in remaining
/// accounts 0 and 1 respectively.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_kamino(
    accounts: &LendingPoolAddBankKamino,
    bank_config: KaminoConfigCompact,
    bank_seed: u64,
) -> Instruction {
    lending_pool_add_bank_kamino_with_program_id(crate::ID, accounts, bank_config, bank_seed)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_kamino_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolAddBankKamino,
    bank_config: KaminoConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankKamino::DISCRIMINATOR.to_vec();
    bank_config
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    bank_seed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`propagate_kamino_market_emergency`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropagateKaminoMarketEmergency {
    /// The reserve `bank` deposits into. Read only to reach its market.
    pub reserve: Pubkey,
    /// The market that owns `reserve`, and the source of the propagated flag.
    pub lending_market: Pubkey,
    pub bank: Pubkey,
}
impl PropagateKaminoMarketEmergency {
    pub const DISCRIMINATOR: [u8; 8] = [252, 182, 246, 73, 108, 118, 102, 142];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.reserve, false),
            AccountMeta::new_readonly(self.lending_market, false),
            AccountMeta::new(self.bank, false),
        ]
    }
}
impl ToAccountMetas for PropagateKaminoMarketEmergency {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Copy the emergency flag of a Kamino lending market onto a bank in that
/// market. While set, the bank backs no new borrowing, exactly as a bank on a reserve in
/// emergency mode does. Clearing the flag works the same way, once Kamino resumes the market.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn propagate_kamino_market_emergency(accounts: &PropagateKaminoMarketEmergency) -> Instruction {
    propagate_kamino_market_emergency_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn propagate_kamino_market_emergency_with_program_id(
    program_id: Pubkey,
    accounts: &PropagateKaminoMarketEmergency,
) -> Instruction {
    let data = PropagateKaminoMarketEmergency::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
