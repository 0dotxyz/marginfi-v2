use account_state::{AccountInfoCache, AccountsState};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_lang::{
    accounts::{interface::Interface, interface_account::InterfaceAccount},
    prelude::{AccountInfo, AccountLoader, Context, Program, Pubkey, Rent, Signer},
    Key,
};
use anchor_spl::token_2022::spl_token_2022::error::TokenError;
use arbitrary_helpers::{
    AccountIdx, AssetAmount, BankAndOracleConfig, BankIdx, PriceChange, TokenType,
};
use bank_accounts::{get_bank_map, BankAccounts};
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use marginfi::instructions::LendingPoolConfigureBankOracleBumps;
use marginfi::{
    errors::MarginfiError, instructions::LendingPoolAddBankBumps, state::bank::BankVaultType,
};
use marginfi_type_crate::types::{
    centi_to_u32, make_points, milli_to_u32, RatePoint, INTEREST_CURVE_SEVEN_POINT,
};
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{
        Bank, BankConfigCompact, BankOperationalState, InterestRateConfig, MarginfiAccount,
        RiskTier,
    },
};
use metrics::{MetricAction, Metrics};
use setup::{initialize_fee_state, initialize_marginfi_group, set_discriminator, sort_balances};
use std::{
    collections::HashMap,
    mem::size_of,
    ops::AddAssign,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};
use stubs::test_syscall_stubs;
use user_accounts::UserAccount;
use utils::{
    account_info_lifetime_shortener as ails, account_info_ref_lifetime_shortener as airls,
    account_info_slice_lifetime_shortener as aisls,
    unchecked_account_info_lifetime_shortener as uails,
};

pub mod account_state;
pub mod arbitrary_helpers;
pub mod bank_accounts;
pub mod metrics;
pub mod setup;
pub mod stubs;
pub mod tests;
pub mod user_accounts;
pub mod utils;

pub struct MarginfiFuzzContext<'info> {
    pub marginfi_group: AccountInfo<'info>,
    pub fee_state: AccountInfo<'info>,
    pub fee_state_wallet: AccountInfo<'info>,
    pub banks: Vec<BankAccounts<'info>>,
    pub marginfi_accounts: Vec<UserAccount<'info>>,
    pub admin: AccountInfo<'info>,
    pub bank_admin: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub last_sysvar_current_timestamp: RwLock<u64>,
    pub metrics: Arc<RwLock<Metrics>>,
    pub state: &'info AccountsState,
}

impl<'state> MarginfiFuzzContext<'state> {
    pub fn setup(
        state: &'state AccountsState,
        bank_configs: &[BankAndOracleConfig],
        n_users: u8,
    ) -> Self {
        let system_program = state.new_program(system_program::ID);
        let admin = state.new_sol_account(1_000_000, true, true);
        let bank_admin = state.new_sol_account(1_000_000, true, true);
        let fee_state_wallet = state.new_sol_account(1_000_000, true, true);
        let fee_state = initialize_fee_state(
            state,
            admin.clone(),
            fee_state_wallet.clone(),
            system_program.clone(),
        );
        let marginfi_group = initialize_marginfi_group(
            state,
            admin.clone(),
            bank_admin.clone(),
            fee_state.clone(),
            system_program.clone(),
        );

        let mut marginfi_state = MarginfiFuzzContext {
            marginfi_group,
            fee_state,
            fee_state_wallet,
            banks: vec![],
            admin: admin.clone(),
            bank_admin,
            system_program,
            marginfi_accounts: vec![],
            last_sysvar_current_timestamp: RwLock::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
            metrics: Arc::new(RwLock::new(Metrics::default())),
            state,
        };
        marginfi_state.advance_time(0);

        bank_configs
            .iter()
            .for_each(|config| marginfi_state.setup_bank(state, Rent::free(), config));

        let token_vec = marginfi_state
            .banks
            .iter()
            .map(|b| b.mint.clone())
            .collect();

        (0..n_users).into_iter().for_each(|_| {
            marginfi_state
                .create_marginfi_account(state, Rent::free(), &token_vec)
                .unwrap()
        });

        // Create an extra account for seeding the banks
        marginfi_state
            .create_marginfi_account(state, Rent::free(), &token_vec)
            .unwrap();

        // Seed the banks
        for bank_idx in 0..marginfi_state.banks.len() {
            marginfi_state
                .process_action_deposit(
                    &AccountIdx(marginfi_state.marginfi_accounts.len() as u8 - 1),
                    &BankIdx(bank_idx as u8),
                    &AssetAmount(
                        1_000
                            * 10_u64
                                .pow(marginfi_state.banks[bank_idx as usize].mint_decimals.into()),
                    ),
                    None,
                )
                .unwrap();
        }

        marginfi_state
    }

    fn get_bank_map<'a>(&'a self) -> HashMap<Pubkey, &'a BankAccounts<'state>> {
        get_bank_map(&self.banks)
    }

    fn refresh_oracle_accounts(&self) {
        self.banks.iter().for_each(|bank| {
            bank.refresh_oracle(
                self.last_sysvar_current_timestamp
                    .read()
                    .unwrap()
                    .to_owned() as i64,
            )
            .unwrap()
        });
    }

    pub fn advance_time(&self, time: u64) {
        self.last_sysvar_current_timestamp
            .write()
            .unwrap()
            .add_assign(time);

        test_syscall_stubs(Some(
            *self.last_sysvar_current_timestamp.read().unwrap() as i64
        ));
    }

    pub fn setup_bank<'a>(
        &'a mut self,
        state: &'state AccountsState,
        rent: Rent,
        initial_bank_config: &BankAndOracleConfig,
    ) {
        log!("Setting up bank with config {:#?}", initial_bank_config);
        let bank = state.new_owned_account(size_of::<Bank>(), marginfi::ID, rent.clone());

        let mint = state.new_token_mint(
            rent.clone(),
            initial_bank_config.mint_decimals,
            initial_bank_config.token_type,
        );
        let (liquidity_vault_authority, liquidity_vault_authority_bump) =
            state.new_vault_authority(BankVaultType::Liquidity, bank.key);
        let (liquidity_vault, liquidity_vault_bump) = state.new_vault_account(
            BankVaultType::Liquidity,
            mint.clone(),
            liquidity_vault_authority.key,
            bank.key,
        );

        let (insurance_vault_authority, insurance_vault_authority_bump) =
            state.new_vault_authority(BankVaultType::Insurance, bank.key);
        let (insurance_vault, insurance_vault_bump) = state.new_vault_account(
            BankVaultType::Insurance,
            mint.clone(),
            insurance_vault_authority.key,
            bank.key,
        );

        let (fee_vault_authority, fee_vault_authority_bump) =
            state.new_vault_authority(BankVaultType::Fee, bank.key);
        let (fee_vault, fee_vault_bump) = state.new_vault_account(
            BankVaultType::Fee,
            mint.clone(),
            fee_vault_authority.key,
            bank.key,
        );
        let (_fee_state_key, fee_state_bump) =
            Pubkey::find_program_address(&[FEE_STATE_SEED.as_bytes()], &marginfi::ID);

        let oracle = state.new_oracle_account(
            rent.clone(),
            initial_bank_config.oracle_native_price as i64,
            initial_bank_config.mint_decimals as i32,
        );

        let add_bank_bumps = LendingPoolAddBankBumps {
            liquidity_vault_authority: liquidity_vault_authority_bump,
            liquidity_vault: liquidity_vault_bump,
            insurance_vault_authority: insurance_vault_authority_bump,
            insurance_vault: insurance_vault_bump,
            fee_vault_authority: fee_vault_authority_bump,
            fee_vault: fee_vault_bump,
            fee_state: fee_state_bump,
        };
        let configure_bumps = LendingPoolConfigureBankOracleBumps {};

        let token_program = match initial_bank_config.token_type {
            TokenType::Tokenkeg => state.new_program(spl_token::id()),
            TokenType::Token22 | TokenType::Token22WithFee { .. } => {
                state.new_program(anchor_spl::token_2022::ID)
            }
        };

        {
            marginfi::instructions::marginfi_group::lending_pool_add_bank(
                Context::new(
                    &marginfi::ID,
                    &mut marginfi::instructions::LendingPoolAddBank {
                        marginfi_group: AccountLoader::try_from(airls(&self.marginfi_group))
                            .unwrap(),
                        bank_admin: Signer::try_from(airls(&self.bank_admin)).unwrap(),
                        fee_payer: Signer::try_from(airls(&self.admin)).unwrap(),
                        fee_state: AccountLoader::try_from(airls(&self.fee_state)).unwrap(),
                        global_fee_wallet: uails(&self.fee_state_wallet),
                        bank_mint: Box::new(InterfaceAccount::try_from(airls(&mint)).unwrap()),
                        bank: AccountLoader::try_from_unchecked(&marginfi::ID, airls(&bank))
                            .unwrap(),
                        liquidity_vault_authority: uails(&liquidity_vault_authority),
                        liquidity_vault: Box::new(
                            InterfaceAccount::try_from(airls(&liquidity_vault)).unwrap(),
                        ),
                        insurance_vault_authority: uails(&insurance_vault_authority),
                        insurance_vault: Box::new(
                            InterfaceAccount::try_from(airls(&insurance_vault)).unwrap(),
                        ),
                        fee_vault_authority: uails(&fee_vault_authority),
                        fee_vault: Box::new(InterfaceAccount::try_from(airls(&fee_vault)).unwrap()),
                        token_program: Interface::try_from(airls(&token_program)).unwrap(),
                        system_program: Program::try_from(airls(&self.system_program)).unwrap(),
                    },
                    &[],
                    add_bank_bumps,
                ),
                BankConfigCompact {
                    asset_weight_init: initial_bank_config.asset_weight_init,
                    asset_weight_maint: initial_bank_config.asset_weight_maint,
                    liability_weight_init: initial_bank_config.liability_weight_init,
                    liability_weight_maint: initial_bank_config.liability_weight_maint,
                    deposit_limit: initial_bank_config.deposit_limit,
                    borrow_limit: initial_bank_config.borrow_limit,
                    interest_rate_config: InterestRateConfig {
                        placeholder0: I80F48::ZERO.into(),
                        placeholder1: I80F48::ZERO.into(),
                        placeholder2: I80F48::ZERO.into(),

                        insurance_fee_fixed_apr: I80F48!(0.01).into(),
                        insurance_ir_fee: I80F48!(0.05).into(),
                        protocol_fixed_fee_apr: I80F48!(0.01).into(),
                        protocol_ir_fee: I80F48!(0.1).into(),
                        protocol_origination_fee: I80F48::ZERO.into(),

                        zero_util_rate: 0,
                        hundred_util_rate: milli_to_u32(I80F48!(4)),
                        points: make_points(&vec![RatePoint::new(
                            centi_to_u32(I80F48!(0.5)),
                            milli_to_u32(I80F48!(0.5)),
                        )]),
                        curve_type: INTEREST_CURVE_SEVEN_POINT,
                        ..Default::default()
                    }
                    .into(),
                    operational_state: BankOperationalState::Operational,
                    risk_tier: if !initial_bank_config.risk_tier_isolated {
                        RiskTier::Collateral
                    } else {
                        RiskTier::Isolated
                    },
                    oracle_max_age: 100,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        set_discriminator::<Bank>(bank.clone());

        {
            marginfi::instructions::marginfi_group::lending_pool_configure_bank_oracle(
                Context::new(
                    &marginfi::ID,
                    &mut marginfi::instructions::LendingPoolConfigureBankOracle {
                        group: AccountLoader::try_from(airls(&self.marginfi_group)).unwrap(),
                        bank_admin: Signer::try_from(airls(&self.bank_admin)).unwrap(),
                        bank: AccountLoader::try_from_unchecked(&marginfi::ID, airls(&bank))
                            .unwrap(),
                    },
                    &[ails(oracle.clone())],
                    configure_bumps,
                ),
                3,
                oracle.key(),
            )
            .unwrap();
        }

        self.banks.push(BankAccounts {
            bank,
            oracle,
            liquidity_vault,
            insurance_vault,
            fee_vault,
            mint,
            liquidity_vault_authority,
            insurance_vault_authority,
            fee_vault_authority,
            mint_decimals: initial_bank_config.mint_decimals,
            token_program,
        });
    }

    fn create_marginfi_account<'a>(
        &'a mut self,
        state: &'state AccountsState,
        rent: Rent,
        token_mints: &Vec<AccountInfo<'state>>,
    ) -> anyhow::Result<()> {
        let marginfi_account =
            state.new_owned_account(size_of::<MarginfiAccount>(), marginfi::ID, rent.clone());

        marginfi::instructions::marginfi_account::initialize_account(Context::new(
            &marginfi::ID,
            &mut marginfi::instructions::marginfi_account::MarginfiAccountInitialize {
                marginfi_group: AccountLoader::try_from(airls(&self.marginfi_group))?,
                marginfi_account: AccountLoader::try_from_unchecked(
                    &marginfi::ID,
                    airls(&marginfi_account),
                )?,
                authority: Signer::try_from(airls(&self.admin))?,
                fee_payer: Signer::try_from(airls(&self.admin))?,
                system_program: Program::try_from(airls(&self.system_program))?,
            },
            &[],
            Default::default(),
        ))?;

        let token_accounts = token_mints
            .iter()
            .map(|token| {
                state.new_token_account(
                    token.clone(),
                    self.admin.key,
                    100_000_000_000_000_000,
                    rent.clone(),
                )
            })
            .collect();

        set_discriminator::<MarginfiAccount>(marginfi_account.clone());

        self.marginfi_accounts
            .push(UserAccount::new(marginfi_account, token_accounts));

        Ok(())
    }

    pub fn process_action_deposit(
        &self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
        asset_amount: &AssetAmount,
        deposit_up_to_limit: Option<bool>,
    ) -> anyhow::Result<()> {
        if account_idx.0 as usize >= self.marginfi_accounts.len()
            || bank_idx.0 as usize >= self.banks.len()
        {
            return Ok(());
        }

        let marginfi_account = &self.marginfi_accounts[account_idx.0 as usize];
        if bank_idx.0 as usize >= marginfi_account.token_accounts.len() {
            return Ok(());
        }

        let bank = &self.banks[bank_idx.0 as usize];
        sort_balances(airls(&marginfi_account.margin_account));

        let cache = AccountInfoCache::new(&[
            marginfi_account.margin_account.clone(),
            bank.bank.clone(),
            marginfi_account.token_accounts[bank_idx.0 as usize].clone(),
            bank.liquidity_vault.clone(),
        ]);

        let mut remaining_accounts: Vec<AccountInfo> = vec![];
        if bank.token_program.key() == anchor_spl::token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }

        let res = marginfi::instructions::marginfi_account::lending_account_deposit(
            Context::new(
                &marginfi::ID,
                &mut marginfi::instructions::LendingAccountDeposit {
                    group: AccountLoader::try_from(airls(&self.marginfi_group))?,
                    marginfi_account: AccountLoader::try_from(airls(
                        &marginfi_account.margin_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.admin))?,
                    bank: AccountLoader::try_from(airls(&bank.bank))?,
                    signer_token_account: uails(
                        &marginfi_account.token_accounts[bank_idx.0 as usize],
                    ),
                    liquidity_vault: InterfaceAccount::try_from(airls(
                        &bank.liquidity_vault.clone(),
                    ))?,
                    token_program: Interface::try_from(airls(&bank.token_program))?,
                },
                aisls(&remaining_accounts),
                Default::default(),
            ),
            asset_amount.0,
            deposit_up_to_limit,
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                [
                    MarginfiError::AccountDisabled.into(),
                    MarginfiError::OperationDepositOnly.into(),
                ]
                .contains(&error),
                "Unexpected deposit error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Deposit, success);

        Ok(())
    }

    pub fn process_action_repay(
        &'state self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
        asset_amount: &AssetAmount,
        repay_all: bool,
    ) -> anyhow::Result<()> {
        if account_idx.0 as usize >= self.marginfi_accounts.len()
            || bank_idx.0 as usize >= self.banks.len()
        {
            return Ok(());
        }

        let marginfi_account = &self.marginfi_accounts[account_idx.0 as usize];
        if bank_idx.0 as usize >= marginfi_account.token_accounts.len() {
            return Ok(());
        }

        let bank = &self.banks[bank_idx.0 as usize];
        sort_balances(airls(&marginfi_account.margin_account));

        let cache = AccountInfoCache::new(&[
            marginfi_account.margin_account.clone(),
            bank.bank.clone(),
            marginfi_account.token_accounts[bank_idx.0 as usize].clone(),
            bank.liquidity_vault.clone(),
        ]);

        let mut remaining_accounts = vec![];
        if bank.token_program.key() == anchor_spl::token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }
        if repay_all {
            remaining_accounts.extend(marginfi_account.get_remaining_accounts(
                &self.get_bank_map(),
                vec![],
                vec![],
                None,
            ));
        }

        let res = marginfi::instructions::marginfi_account::lending_account_repay(
            Context::new(
                &marginfi::ID,
                &mut marginfi::instructions::LendingAccountRepay {
                    group: AccountLoader::try_from(airls(&self.marginfi_group))?,
                    marginfi_account: AccountLoader::try_from(airls(
                        &marginfi_account.margin_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.admin))?,
                    bank: AccountLoader::try_from(airls(&bank.bank))?,
                    signer_token_account: uails(
                        &marginfi_account.token_accounts[bank_idx.0 as usize],
                    ),
                    liquidity_vault: InterfaceAccount::try_from(airls(
                        &bank.liquidity_vault.clone(),
                    ))?,
                    token_program: Interface::try_from(airls(&bank.token_program))?,
                },
                aisls(&remaining_accounts),
                Default::default(),
            ),
            asset_amount.0,
            Some(repay_all),
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                vec![
                    MarginfiError::NoLiabilityFound.into(),
                    MarginfiError::OperationRepayOnly.into(),
                    MarginfiError::BankAccountNotFound.into(),
                    MarginfiError::AccountDisabled.into(),
                ]
                .contains(&error),
                "Unexpected repay error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Repay, success);

        Ok(())
    }

    pub fn process_action_withdraw(
        &'state self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
        asset_amount: &AssetAmount,
        withdraw_all: Option<bool>,
    ) -> anyhow::Result<()> {
        self.refresh_oracle_accounts();

        if account_idx.0 as usize >= self.marginfi_accounts.len()
            || bank_idx.0 as usize >= self.banks.len()
        {
            return Ok(());
        }

        let marginfi_account = &self.marginfi_accounts[account_idx.0 as usize];
        if bank_idx.0 as usize >= marginfi_account.token_accounts.len() {
            return Ok(());
        }

        let bank = &self.banks[bank_idx.0 as usize];
        sort_balances(airls(&marginfi_account.margin_account));

        let cache = AccountInfoCache::new(&[
            marginfi_account.margin_account.clone(),
            bank.bank.clone(),
            marginfi_account.token_accounts[bank_idx.0 as usize].clone(),
            bank.liquidity_vault.clone(),
        ]);

        let mut remaining_accounts = vec![];
        if bank.token_program.key() == anchor_spl::token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }
        let close_bank_last = withdraw_all.and_then(|withdraw_all| {
            if withdraw_all {
                Some(bank.bank.key())
            } else {
                None
            }
        });
        remaining_accounts.extend(marginfi_account.get_remaining_accounts(
            &self.get_bank_map(),
            vec![],
            vec![],
            close_bank_last,
        ));
        let res = marginfi::instructions::marginfi_account::lending_account_withdraw(
            Context::new(
                &marginfi::ID,
                &mut marginfi::instructions::LendingAccountWithdraw {
                    group: AccountLoader::try_from(airls(&self.marginfi_group))?,
                    marginfi_account: AccountLoader::try_from(airls(
                        &marginfi_account.margin_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.admin))?,
                    bank: AccountLoader::try_from(airls(&bank.bank))?,
                    token_program: Interface::try_from(airls(&bank.token_program))?,
                    destination_token_account: InterfaceAccount::try_from(airls(
                        &marginfi_account.token_accounts[bank_idx.0 as usize],
                    ))?,
                    bank_liquidity_vault_authority: uails(&bank.liquidity_vault_authority),
                    liquidity_vault: InterfaceAccount::try_from(airls(&bank.liquidity_vault))?,
                },
                aisls(&remaining_accounts),
                Default::default(),
            ),
            asset_amount.0,
            withdraw_all,
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                [
                    MarginfiError::OperationWithdrawOnly.into(),
                    MarginfiError::IllegalUtilizationRatio.into(),
                    MarginfiError::RiskEngineInitRejected.into(),
                    MarginfiError::NoAssetFound.into(),
                    MarginfiError::BankAccountNotFound.into(),
                    MarginfiError::InvalidBankAccount.into(),
                    MarginfiError::AccountDisabled.into(),
                ]
                .contains(&error),
                "Unexpected withdraw error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Withdraw, success);

        Ok(())
    }

    pub fn process_action_borrow(
        &'state self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
        asset_amount: &AssetAmount,
    ) -> anyhow::Result<()> {
        self.refresh_oracle_accounts();

        if account_idx.0 as usize >= self.marginfi_accounts.len()
            || bank_idx.0 as usize >= self.banks.len()
        {
            return Ok(());
        }

        let marginfi_account = &self.marginfi_accounts[account_idx.0 as usize];
        if bank_idx.0 as usize >= marginfi_account.token_accounts.len() {
            return Ok(());
        }

        let bank = &self.banks[bank_idx.0 as usize];

        let cache = AccountInfoCache::new(&[
            marginfi_account.margin_account.clone(),
            bank.bank.clone(),
            marginfi_account.token_accounts[bank_idx.0 as usize].clone(),
            bank.liquidity_vault.clone(),
        ]);
        sort_balances(airls(&marginfi_account.margin_account));

        let mut remaining_accounts = vec![];
        if bank.token_program.key() == anchor_spl::token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }
        remaining_accounts.extend(marginfi_account.get_remaining_accounts(
            &self.get_bank_map(),
            vec![bank.bank.key()],
            vec![],
            None,
        ));
        let res = marginfi::instructions::marginfi_account::lending_account_borrow(
            Context::new(
                &marginfi::ID,
                &mut marginfi::instructions::LendingAccountBorrow {
                    group: AccountLoader::try_from(airls(&self.marginfi_group))?,
                    marginfi_account: AccountLoader::try_from(airls(
                        &marginfi_account.margin_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.admin))?,
                    bank: AccountLoader::try_from(airls(&bank.bank))?,
                    token_program: Interface::try_from(airls(&bank.token_program))?,
                    destination_token_account: InterfaceAccount::try_from(airls(
                        &marginfi_account.token_accounts[bank_idx.0 as usize],
                    ))?,
                    bank_liquidity_vault_authority: uails(&bank.liquidity_vault_authority),
                    liquidity_vault: InterfaceAccount::try_from(airls(&bank.liquidity_vault))?,
                },
                aisls(&remaining_accounts),
                Default::default(),
            ),
            asset_amount.0,
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                vec![
                    MarginfiError::RiskEngineInitRejected.into(),
                    MarginfiError::IsolatedAccountIllegalState.into(),
                    MarginfiError::IllegalUtilizationRatio.into(),
                    MarginfiError::AccountDisabled.into(),
                    MarginfiError::OperationBorrowOnly.into(),
                ]
                .contains(&error),
                "Unexpected borrow error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Borrow, success);

        Ok(())
    }

    pub fn process_liquidate_account(
        &'state self,
        liquidator_idx: &AccountIdx,
        liquidatee_idx: &AccountIdx,
        asset_amount: &AssetAmount,
    ) -> anyhow::Result<()> {
        self.refresh_oracle_accounts();

        if liquidator_idx.0 as usize >= self.marginfi_accounts.len()
            || liquidatee_idx.0 as usize >= self.marginfi_accounts.len()
        {
            return Ok(());
        }

        let liquidator_account = &self.marginfi_accounts[liquidator_idx.0 as usize];
        let liquidatee_account = &self.marginfi_accounts[liquidatee_idx.0 as usize];
        sort_balances(airls(&liquidator_account.margin_account));
        sort_balances(airls(&liquidatee_account.margin_account));

        if liquidator_account.margin_account.key() == liquidatee_account.margin_account.key() {
            self.metrics
                .write()
                .unwrap()
                .update_metric(MetricAction::Liquidate, false);

            return Ok(());
        }

        let (asset_bank_idx, liab_bank_idx) =
            if let Some(a) = liquidatee_account.get_liquidation_banks(&self.banks) {
                if a.0 == a.1 {
                    self.metrics
                        .write()
                        .unwrap()
                        .update_metric(MetricAction::Liquidate, false);

                    return Ok(());
                } else {
                    a
                }
            } else {
                self.metrics
                    .write()
                    .unwrap()
                    .update_metric(MetricAction::Liquidate, false);

                return Ok(());
            };

        let asset_bank = &self.banks[asset_bank_idx.0 as usize];
        let liab_bank = &self.banks[liab_bank_idx.0 as usize];

        let account_cache = AccountInfoCache::new(&[
            liquidator_account.margin_account.clone(),
            liquidatee_account.margin_account.clone(),
            asset_bank.bank.clone(),
            asset_bank.liquidity_vault.clone(),
            liab_bank.bank.clone(),
            liab_bank.liquidity_vault.clone(),
            liab_bank.insurance_vault.clone(),
        ]);

        let mut remaining_accounts = vec![];
        if liab_bank.token_program.key() == anchor_spl::token_2022::ID {
            remaining_accounts.push(ails(liab_bank.mint.clone()));
        }
        remaining_accounts.extend(vec![asset_bank.oracle.clone(), liab_bank.oracle.clone()]);

        let mut liquidator_remaining_accounts = liquidator_account.get_remaining_accounts(
            &self.get_bank_map(),
            vec![liab_bank.bank.key(), asset_bank.bank.key()],
            vec![],
            None,
        );
        let mut liquidatee_remaining_accounts =
            liquidatee_account.get_remaining_accounts(&self.get_bank_map(), vec![], vec![], None);

        // Note: this must happen before append because it mutably drains the source vec
        let liquidator_accounts_num = liquidator_remaining_accounts.len() as u8;
        let liquidatee_accounts_num = liquidatee_remaining_accounts.len() as u8;
        remaining_accounts.append(&mut liquidator_remaining_accounts);
        remaining_accounts.append(&mut liquidatee_remaining_accounts);

        let res = marginfi::instructions::lending_account_liquidate(
            Context::new(
                &marginfi::ID,
                &mut marginfi::instructions::LendingAccountLiquidate {
                    group: AccountLoader::try_from(airls(&self.marginfi_group))?,
                    asset_bank: AccountLoader::try_from(airls(&asset_bank.bank))?,
                    liab_bank: AccountLoader::try_from(airls(&liab_bank.bank))?,
                    liquidator_marginfi_account: AccountLoader::try_from(airls(
                        &liquidator_account.margin_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.admin))?,
                    liquidatee_marginfi_account: AccountLoader::try_from(airls(
                        &liquidatee_account.margin_account,
                    ))?,
                    bank_liquidity_vault_authority: uails(&liab_bank.liquidity_vault_authority),
                    bank_liquidity_vault: Box::new(InterfaceAccount::try_from(airls(
                        &liab_bank.liquidity_vault,
                    ))?),
                    bank_insurance_vault: uails(&liab_bank.insurance_vault),
                    token_program: Interface::try_from(airls(&liab_bank.token_program))?,
                },
                aisls(&remaining_accounts),
                Default::default(),
            ),
            asset_amount.0,
            liquidatee_accounts_num,
            liquidator_accounts_num,
        );

        let success = if let Err(error) = res {
            let allowed_errors = &[
                MarginfiError::RiskEngineInitRejected.into(),
                MarginfiError::IsolatedAccountIllegalState.into(),
                MarginfiError::IllegalUtilizationRatio.into(),
                MarginfiError::ZeroLiquidationAmount.into(),
                MarginfiError::OverliquidationAttempt.into(),
                MarginfiError::HealthyAccount.into(),
                MarginfiError::ExhaustedLiability.into(),
                MarginfiError::TooSevereLiquidation.into(),
                MarginfiError::AccountDisabled.into(),
                MarginfiError::ZeroAssetPrice.into(),
                MarginfiError::ZeroLiabilityPrice.into(),
                MarginfiError::OperationRepayOnly.into(),
                // Note: because updates in 1.5 allow liquidation of underwater banks, it is now
                // possible for a bank's liquidity value to become empty in the fuzz suite, which
                // leads to the `liquidatee_liab_bank_account.withdraw_spl_transfer` failing. This
                // is probably benign but certainly rare-or-nonexistent in prod.
                ProgramError::Custom(TokenError::InsufficientFunds as u32).into(),
            ];

            // Log full context on unexpected error
            if !allowed_errors.contains(&error) {
                match &error {
                    Error::ProgramError(boxed_pe) => {
                        // Note: non-program errors from CPI calls (like Token Error) may look like:
                        // program_error: Custom(1), error_origin: None, compared_values: None,
                        let pe = &**boxed_pe;
                        if let ProgramError::Custom(code) = pe.program_error {
                            eprintln!("🚨 raw custom error code: {}", code);
                        } else {
                            eprintln!("🚨 program_error variant: {:?}", pe.program_error);
                        }
                        eprintln!("🚨 error_origin:   {:?}", pe.error_origin);
                        eprintln!("🚨 compared_vals: {:?}", pe.compared_values);
                    }
                    Error::AnchorError(anchor_err) => {
                        eprintln!("🚨 anchor error: {:?}", anchor_err);
                    }
                }

                eprintln!(
                    "❌ unexpected liquidate error:\n\
                     → liquidator_idx: {:?}\n\
                     → liquidatee_idx: {:?}\n\
                     → asset_bank_idx: {:?}\n\
                     → liab_bank_idx: {:?}\n\
                     → asset_amount: {:?}\n\
                     → error: {:?}\n",
                    liquidator_idx,
                    liquidatee_idx,
                    asset_bank_idx,
                    liab_bank_idx,
                    asset_amount,
                    error,
                );
            }

            // Assert and fail if unexpected error
            assert!(
                allowed_errors.contains(&error),
                "Unexpected liquidate error: {:?}",
                error
            );

            account_cache.revert();
            false
        } else {
            self.process_handle_bankruptcy(liquidatee_idx, &liab_bank_idx)?;
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Liquidate, success);

        Ok(())
    }

    pub fn process_handle_bankruptcy(
        &'state self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
    ) -> anyhow::Result<()> {
        log!("Action: Handle Bankruptcy");

        if account_idx.0 as usize >= self.marginfi_accounts.len()
            || bank_idx.0 as usize >= self.banks.len()
        {
            return Ok(());
        }

        let marginfi_account = &self.marginfi_accounts[account_idx.0 as usize];
        if bank_idx.0 as usize >= marginfi_account.token_accounts.len() {
            return Ok(());
        }

        let bank = &self.banks[bank_idx.0 as usize];

        let cache = AccountInfoCache::new(&[
            bank.bank.clone(),
            marginfi_account.margin_account.clone(),
            bank.liquidity_vault.clone(),
            bank.insurance_vault.clone(),
        ]);

        let mut remaining_accounts = vec![];
        if bank.token_program.key() == anchor_spl::token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }
        remaining_accounts.extend(marginfi_account.get_remaining_accounts(
            &self.get_bank_map(),
            vec![],
            vec![],
            None,
        ));
        let res = marginfi::instructions::lending_pool_handle_bankruptcy(Context::new(
            &marginfi::ID,
            &mut marginfi::instructions::LendingPoolHandleBankruptcy {
                group: AccountLoader::try_from(airls(&self.marginfi_group))?,
                signer: Signer::try_from(airls(&self.admin))?,
                bank: AccountLoader::try_from(airls(&bank.bank))?,
                marginfi_account: AccountLoader::try_from(airls(&marginfi_account.margin_account))?,
                liquidity_vault: uails(&bank.liquidity_vault),
                insurance_vault: Box::new(InterfaceAccount::try_from(airls(
                    &bank.insurance_vault,
                ))?),
                insurance_vault_authority: uails(&bank.insurance_vault_authority),
                token_program: Interface::try_from(airls(&bank.token_program))?,
            },
            aisls(&remaining_accounts),
            Default::default(),
        ));

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                vec![
                    MarginfiError::AccountDisabled.into(),
                    MarginfiError::AccountNotBankrupt.into(),
                ]
                .contains(&error),
                "Unexpected handle bankruptcy error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Bankruptcy, success);

        Ok(())
    }

    pub fn process_update_oracle(
        &self,
        bank_idx: &BankIdx,
        price_change: &PriceChange,
    ) -> anyhow::Result<()> {
        log!("Action: Update Oracle");

        if bank_idx.0 as usize >= self.banks.len() {
            return Ok(());
        }

        let bank = &self.banks[bank_idx.0 as usize];

        bank.update_oracle(price_change.0)?;

        self.metrics.write().unwrap().price_update += 1;

        Ok(())
    }
}
