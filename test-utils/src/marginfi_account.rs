use super::{bank::BankFixture, prelude::*};
use crate::ui_to_native;
use crate::utils::find_order_pda;
use anchor_lang::{prelude::*, system_program, InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use kamino_mocks::kamino_lending::client as kamino;
use kamino_mocks::state::{MinimalObligation, MinimalReserve};
use marginfi::state::bank::BankVaultType;
use marginfi_type_crate::types::OracleSetup;
use marginfi_type_crate::types::{Bank, FeeState, MarginfiAccount, Order, OrderTrigger};
use solana_program::{instruction::Instruction, sysvar};
use solana_program_test::{BanksClient, BanksClientError, ProgramTestContext};
use solana_sdk::{
    commitment_config::CommitmentLevel, compute_budget::ComputeBudgetInstruction, hash::Hash,
    signature::Keypair, signer::Signer, transaction::Transaction,
};
use std::{cell::RefCell, mem, rc::Rc};

#[cfg(feature = "transfer-hook")]
use transfer_hook::TEST_HOOK_ID;

#[derive(Default, Clone)]
pub struct MarginfiAccountConfig {}

async fn ctx_parts(ctx: &Rc<RefCell<ProgramTestContext>>) -> (BanksClient, Keypair, Hash) {
    let (banks_client, payer) = {
        let ctx_ref = ctx.borrow();
        (ctx_ref.banks_client.clone(), ctx_ref.payer.insecure_clone())
    };
    let blockhash = banks_client.get_latest_blockhash().await.unwrap();
    (banks_client, payer, blockhash)
}

fn derive_kamino_lending_market_authority(lending_market: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"lma", lending_market.as_ref()],
        &kamino_mocks::kamino_lending::ID,
    )
    .0
}

pub struct MarginfiAccountFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
}

impl MarginfiAccountFixture {
    pub async fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        marginfi_group: &Pubkey,
    ) -> MarginfiAccountFixture {
        let payer = ctx.borrow().payer.insecure_clone();
        Self::new_with_authority(ctx, marginfi_group, &payer).await
    }

    pub async fn new_with_authority(
        ctx: Rc<RefCell<ProgramTestContext>>,
        marginfi_group: &Pubkey,
        authority: &Keypair,
    ) -> MarginfiAccountFixture {
        let ctx_ref = ctx.clone();
        let account_key = Keypair::new();

        let (banks_client, payer, blockhash) = ctx_parts(&ctx_ref).await;
        let accounts = marginfi::accounts::MarginfiAccountInitialize {
            marginfi_account: account_key.pubkey(),
            marginfi_group: *marginfi_group,
            authority: authority.pubkey(),
            fee_payer: payer.pubkey(),
            system_program: system_program::ID,
        };
        let init_marginfi_account_ix = Instruction {
            program_id: marginfi::ID,
            accounts: accounts.to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountInitialize {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[init_marginfi_account_ix],
            Some(&payer.pubkey()),
            &[&payer, &account_key, authority],
            blockhash,
        );
        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
            .unwrap();

        MarginfiAccountFixture {
            ctx: ctx_ref,
            key: account_key.pubkey(),
        }
    }

    async fn make_bank_deposit_ix_internal<T: Into<f64>>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        deposit_up_to_limit: Option<bool>,
        authority: Pubkey,
    ) -> Instruction {
        let marginfi_account = self.load().await;

        let mut accounts = marginfi::accounts::LendingAccountDeposit {
            group: marginfi_account.group,
            marginfi_account: self.key,
            authority,
            bank: bank.key,
            signer_token_account: funding_account,
            liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
            token_program: bank.get_token_program(),
        }
        .to_account_metas(Some(true));
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingAccountDeposit {
                amount: ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
                deposit_up_to_limit,
            }
            .data(),
        }
    }

    pub async fn make_bank_deposit_ix<T: Into<f64>>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        deposit_up_to_limit: Option<bool>,
    ) -> Instruction {
        self.make_bank_deposit_ix_internal(
            funding_account,
            bank,
            ui_amount,
            deposit_up_to_limit,
            self.ctx.borrow().payer.pubkey(),
        )
        .await
    }

    pub async fn make_bank_deposit_ix_with_authority<T: Into<f64>>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        deposit_up_to_limit: Option<bool>,
        authority: Pubkey,
    ) -> Instruction {
        self.make_bank_deposit_ix_internal(
            funding_account,
            bank,
            ui_amount,
            deposit_up_to_limit,
            authority,
        )
        .await
    }

    pub async fn try_bank_deposit<T: Into<f64> + Copy>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        deposit_up_to_limit: Option<bool>,
    ) -> anyhow::Result<(), BanksClientError> {
        self.try_bank_deposit_with_authority(
            funding_account,
            bank,
            ui_amount,
            deposit_up_to_limit,
            &self.ctx.borrow().payer.insecure_clone(),
        )
        .await
    }

    pub async fn try_bank_deposit_with_authority<T: Into<f64> + Copy>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        deposit_up_to_limit: Option<bool>,
        authority: &Keypair,
    ) -> anyhow::Result<(), BanksClientError> {
        #[cfg_attr(not(feature = "transfer-hook"), allow(unused_mut))]
        let mut ix = self
            .make_bank_deposit_ix_with_authority(
                funding_account,
                bank,
                ui_amount,
                deposit_up_to_limit,
                authority.pubkey(),
            )
            .await;

        #[cfg(feature = "transfer-hook")]
        {
            // If t22 with transfer hook, add remaining accounts
            let banks_client = self.ctx.borrow().banks_client.clone();
            let fetch_account_data_fn = move |key| {
                let mut banks_client = banks_client.clone();
                async move {
                    banks_client
                        .get_account(key)
                        .await
                        .map(|acc| acc.map(|a| a.data))
                }
            };
            let payer = self.ctx.borrow().payer.pubkey();
            if bank.mint.token_program == anchor_spl::token_2022::ID {
                // TODO: do that only if hook exists
                println!(
                    "[TODO] Adding extra account metas for execute for mint {:?}",
                    bank.mint.key
                );
                let _ = spl_transfer_hook_interface::offchain::add_extra_account_metas_for_execute(
                    &mut ix,
                    &TEST_HOOK_ID,
                    &funding_account,
                    &bank.mint.key,
                    &bank.get_vault(BankVaultType::Liquidity).0,
                    &payer,
                    ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
                    fetch_account_data_fn,
                )
                .await;
            }
        }

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;
        let mut signers: Vec<&Keypair> = vec![&payer];
        if authority.pubkey() != payer.pubkey() {
            signers.push(authority);
        }
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &signers, blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn try_set_freeze(&self, frozen: bool) -> std::result::Result<(), BanksClientError> {
        let marginfi_account = self.load().await;

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SetAccountFreeze {
                group: marginfi_account.group,
                marginfi_account: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountSetFreeze { frozen }.data(),
        };

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn make_withdraw_ix_with_authority<T: Into<f64>>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        withdraw_all: Option<bool>,
        authority: Pubkey,
    ) -> Instruction {
        let marginfi_account = self.load().await;

        let mut accounts = marginfi::accounts::LendingAccountWithdraw {
            group: marginfi_account.group,
            marginfi_account: self.key,
            authority,
            bank: bank.key,
            destination_token_account: destination_account,
            liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
            bank_liquidity_vault_authority: bank.get_vault_authority(BankVaultType::Liquidity).0,
            token_program: bank.get_token_program(),
        }
        .to_account_metas(Some(true));
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingAccountWithdraw {
                amount: ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
                withdraw_all,
            }
            .data(),
        };

        if withdraw_all.unwrap_or(false) {
            ix.accounts.extend_from_slice(
                &self
                    .load_observation_account_metas_close_last(bank.key, vec![], vec![])
                    .await,
            );
        } else {
            ix.accounts
                .extend_from_slice(&self.load_observation_account_metas(vec![], vec![]).await);
        }

        ix
    }

    pub async fn make_bank_withdraw_ix<T: Into<f64>>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        withdraw_all: Option<bool>,
    ) -> Instruction {
        self.make_withdraw_ix_with_authority(
            destination_account,
            bank,
            ui_amount,
            withdraw_all,
            self.ctx.borrow().payer.pubkey(),
        )
        .await
    }

    pub async fn try_bank_withdraw<T: Into<f64>>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        withdraw_all: Option<bool>,
    ) -> anyhow::Result<(), BanksClientError> {
        self.try_bank_withdraw_with_authority(
            destination_account,
            bank,
            ui_amount,
            withdraw_all,
            &self.ctx.borrow().payer.insecure_clone(),
        )
        .await
    }

    pub async fn try_bank_withdraw_with_authority<T: Into<f64>>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        withdraw_all: Option<bool>,
        authority: &Keypair,
    ) -> anyhow::Result<(), BanksClientError> {
        let ix = self
            .make_withdraw_ix_with_authority(
                destination_account,
                bank,
                ui_amount,
                withdraw_all,
                authority.pubkey(),
            )
            .await;

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;
        let mut signers: Vec<&Keypair> = vec![&payer];
        if authority.pubkey() != payer.pubkey() {
            signers.push(authority);
        }
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &signers, blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    async fn make_bank_borrow_ix_internal<T: Into<f64>>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        authority: Pubkey,
    ) -> Instruction {
        let marginfi_account = self.load().await;

        let mut accounts = marginfi::accounts::LendingAccountBorrow {
            group: marginfi_account.group,
            marginfi_account: self.key,
            authority,
            bank: bank.key,
            destination_token_account: destination_account,
            liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
            bank_liquidity_vault_authority: bank.get_vault_authority(BankVaultType::Liquidity).0,
            token_program: bank.get_token_program(),
        }
        .to_account_metas(Some(true));
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingAccountBorrow {
                amount: ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
            }
            .data(),
        };

        ix.accounts.extend_from_slice(
            &self
                .load_observation_account_metas(vec![bank.key], vec![])
                .await,
        );

        ix
    }

    pub async fn make_bank_borrow_ix<T: Into<f64>>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
    ) -> Instruction {
        self.make_bank_borrow_ix_internal(
            destination_account,
            bank,
            ui_amount,
            self.ctx.borrow().payer.pubkey(),
        )
        .await
    }

    pub async fn try_bank_borrow<T: Into<f64> + Copy>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
    ) -> anyhow::Result<(), BanksClientError> {
        self.try_bank_borrow_with_authority(
            destination_account,
            bank,
            ui_amount,
            100,
            &self.ctx.borrow().payer.insecure_clone(),
        )
        .await
    }

    pub async fn try_bank_borrow_with_nonce<T: Into<f64> + Copy>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        nonce: u64,
    ) -> anyhow::Result<(), BanksClientError> {
        self.try_bank_borrow_with_authority(
            destination_account,
            bank,
            ui_amount,
            nonce,
            &self.ctx.borrow().payer.insecure_clone(),
        )
        .await
    }

    pub async fn try_bank_borrow_with_authority<T: Into<f64> + Copy>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        nonce: u64,
        authority: &Keypair,
    ) -> anyhow::Result<(), BanksClientError> {
        #[cfg_attr(not(feature = "transfer-hook"), allow(unused_mut))]
        let mut ix = self
            .make_bank_borrow_ix_internal(destination_account, bank, ui_amount, authority.pubkey())
            .await;

        #[cfg(feature = "transfer-hook")]
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            let banks_client = self.ctx.borrow().banks_client.clone();
            let fetch_account_data_fn = move |key| {
                let mut banks_client = banks_client.clone();
                async move {
                    banks_client
                        .get_account(key)
                        .await
                        .map(|acc| acc.map(|a| a.data))
                }
            };

            let payer = self.ctx.borrow().payer.pubkey();
            let _ = spl_transfer_hook_interface::offchain::add_extra_account_metas_for_execute(
                &mut ix,
                &TEST_HOOK_ID,
                &bank.get_vault(BankVaultType::Liquidity).0,
                &bank.mint.key,
                &destination_account,
                &payer,
                ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
                fetch_account_data_fn,
            )
            .await;
        }

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let nonce_ix = ComputeBudgetInstruction::set_compute_unit_price(nonce);

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;
        let mut signers: Vec<&Keypair> = vec![&payer];
        if authority.pubkey() != payer.pubkey() {
            signers.push(authority);
        }
        let tx = Transaction::new_signed_with_payer(
            &[compute_budget_ix, nonce_ix, ix],
            Some(&payer.pubkey()),
            &signers,
            blockhash,
        );

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn make_repay_ix<T: Into<f64>>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        repay_all: Option<bool>,
    ) -> Instruction {
        self.make_repay_ix_with_authority(
            funding_account,
            bank,
            ui_amount,
            repay_all,
            self.ctx.borrow().payer.pubkey(),
        )
        .await
    }

    pub async fn make_repay_ix_with_authority<T: Into<f64>>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        repay_all: Option<bool>,
        authority: Pubkey,
    ) -> Instruction {
        let marginfi_account = self.load().await;

        let mut accounts = marginfi::accounts::LendingAccountRepay {
            group: marginfi_account.group,
            marginfi_account: self.key,
            authority,
            bank: bank.key,
            signer_token_account: funding_account,
            liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
            token_program: bank.get_token_program(),
        }
        .to_account_metas(Some(true));
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingAccountRepay {
                amount: ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
                repay_all,
            }
            .data(),
        };

        if repay_all.unwrap_or(false) {
            ix.accounts.extend_from_slice(
                &self
                    .load_observation_account_metas(vec![bank.key], vec![])
                    .await,
            );
        }

        ix
    }

    pub async fn try_bank_repay<T: Into<f64>>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        repay_all: Option<bool>,
    ) -> anyhow::Result<(), BanksClientError> {
        self.try_bank_repay_with_authority(
            funding_account,
            bank,
            ui_amount,
            repay_all,
            &self.ctx.borrow().payer.insecure_clone(),
        )
        .await
    }

    pub async fn try_bank_repay_with_authority<T: Into<f64>>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        repay_all: Option<bool>,
        authority: &Keypair,
    ) -> anyhow::Result<(), BanksClientError> {
        let ix = self
            .make_repay_ix_with_authority(
                funding_account,
                bank,
                ui_amount,
                repay_all,
                authority.pubkey(),
            )
            .await;
        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;
        let mut signers: Vec<&Keypair> = vec![&payer];
        if authority.pubkey() != payer.pubkey() {
            signers.push(authority);
        }
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &signers, blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn try_balance_close(
        &self,
        bank: &BankFixture,
    ) -> anyhow::Result<(), BanksClientError> {
        let marginfi_account = self.load().await;
        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountCloseBalance {
                group: marginfi_account.group,
                marginfi_account: self.key,
                authority: payer.pubkey(),
                bank: bank.key,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingAccountCloseBalance.data(),
        };

        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await?;

        Ok(())
    }

    pub async fn try_liquidate<T: Into<f64> + Copy>(
        &self,
        liquidatee: &MarginfiAccountFixture,
        asset_bank_fixture: &BankFixture,
        asset_ui_amount: T,
        liab_bank_fixture: &BankFixture,
    ) -> std::result::Result<(), BanksClientError> {
        let marginfi_account = self.load().await;

        let asset_bank = asset_bank_fixture.load().await;
        let liab_bank = liab_bank_fixture.load().await;

        let mut accounts = marginfi::accounts::LendingAccountLiquidate {
            group: marginfi_account.group,
            asset_bank: asset_bank_fixture.key,
            liab_bank: liab_bank_fixture.key,
            liquidator_marginfi_account: self.key,
            authority: self.ctx.borrow().payer.pubkey(),
            liquidatee_marginfi_account: liquidatee.key,
            bank_liquidity_vault_authority: liab_bank_fixture
                .get_vault_authority(BankVaultType::Liquidity)
                .0,
            bank_liquidity_vault: liab_bank_fixture.get_vault(BankVaultType::Liquidity).0,
            bank_insurance_vault: liab_bank_fixture.get_vault(BankVaultType::Insurance).0,
            token_program: liab_bank_fixture.get_token_program(),
        }
        .to_account_metas(Some(true));

        if liab_bank_fixture.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(liab_bank_fixture.mint.key, false));
        }

        if asset_bank.config.oracle_setup != OracleSetup::Fixed {
            accounts.push(AccountMeta::new_readonly(
                asset_bank.config.oracle_keys[0],
                false,
            ));
        }
        if liab_bank.config.oracle_setup != OracleSetup::Fixed {
            accounts.push(AccountMeta::new_readonly(
                liab_bank.config.oracle_keys[0],
                false,
            ));
        }

        let liquidator_obs_accounts = &self
            .load_observation_account_metas(
                vec![asset_bank_fixture.key, liab_bank_fixture.key],
                vec![],
            )
            .await;
        let liquidator_accounts = liquidator_obs_accounts.len() as u8;

        let liquidatee_obs_accounts = &liquidatee
            .load_observation_account_metas(vec![], vec![])
            .await;
        let liquidatee_accounts = liquidatee_obs_accounts.len() as u8;

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingAccountLiquidate {
                asset_amount: ui_to_native!(
                    asset_ui_amount.into(),
                    asset_bank_fixture.mint.mint.decimals
                ),
                liquidatee_accounts,
                liquidator_accounts,
            }
            .data(),
        };

        #[cfg(feature = "transfer-hook")]
        if liab_bank_fixture.mint.token_program == anchor_spl::token_2022::ID {
            let payer = self.ctx.borrow().payer.pubkey();
            let fetch_account_data_fn = |key| async move {
                self.ctx
                    .borrow_mut()
                    .banks_client
                    .get_account(key)
                    .await
                    .map(|acc| acc.map(|a| a.data))
            };

            let _ = spl_transfer_hook_interface::offchain::add_extra_account_metas_for_execute(
                &mut ix,
                &TEST_HOOK_ID,
                &liab_bank_fixture.mint.key,
                &liab_bank_fixture.mint.key,
                &liab_bank_fixture.mint.key,
                &payer,
                0,
                fetch_account_data_fn,
            )
            .await;
        }

        ix.accounts.extend_from_slice(liquidator_obs_accounts);
        ix.accounts.extend_from_slice(liquidatee_obs_accounts);

        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;
        let tx = Transaction::new_signed_with_payer(
            &[compute_budget_ix, ix],
            Some(&payer.pubkey()),
            &[&payer],
            blockhash,
        );

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn try_set_emissions_destination(
        &self,
        destination_account: Pubkey,
    ) -> std::result::Result<(), BanksClientError> {
        let ctx = self.ctx.borrow();

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiAccountUpdateEmissionsDestinationAccount {
                marginfi_account: self.key,
                authority: ctx.payer.pubkey(),
                destination_account,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountUpdateEmissionsDestinationAccount {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        drop(ctx);
        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn try_set_emissions_destination_with_authority(
        &self,
        destination_account: Pubkey,
        authority: &Keypair,
    ) -> std::result::Result<(), BanksClientError> {
        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiAccountUpdateEmissionsDestinationAccount {
                marginfi_account: self.key,
                authority: authority.pubkey(),
                destination_account,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountUpdateEmissionsDestinationAccount {}.data(),
        };

        let mut signers: Vec<&Keypair> = vec![&payer];
        if authority.pubkey() != payer.pubkey() {
            signers.push(authority);
        }

        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &signers, blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn try_clear_emissions(
        &self,
        bank: &super::bank::BankFixture,
    ) -> std::result::Result<(), BanksClientError> {
        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountClearEmissions {
                marginfi_account: self.key,
                bank: bank.key,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingAccountClearEmissions {}.data(),
        };

        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn make_lending_account_start_flashloan_ix(&self, end_index: u64) -> Instruction {
        Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountStartFlashloan {
                marginfi_account: self.key,
                authority: self.ctx.borrow().payer.pubkey(),
                ixs_sysvar: sysvar::instructions::id(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingAccountStartFlashloan { end_index }.data(),
        }
    }

    pub async fn make_lending_account_end_flashloan_ix(
        &self,
        include_banks: Vec<Pubkey>,
        exclude_banks: Vec<Pubkey>,
    ) -> Instruction {
        let mut account_metas = marginfi::accounts::LendingAccountEndFlashloan {
            marginfi_account: self.key,
            authority: self.ctx.borrow().payer.pubkey(),
        }
        .to_account_metas(Some(true));

        account_metas.extend(
            self.load_observation_account_metas(include_banks, exclude_banks)
                .await,
        );

        Instruction {
            program_id: marginfi::ID,
            accounts: account_metas,
            data: marginfi::instruction::LendingAccountEndFlashloan {}.data(),
        }
    }

    /// Wrap `ixs` between a start and end flashloan instruction,
    /// automatically sets the end index and send the transaction
    pub async fn try_flashloan(
        &self,
        ixs: Vec<Instruction>,
        exclude_banks: Vec<Pubkey>,
        include_banks: Vec<Pubkey>,
        signer: Option<&Keypair>,
    ) -> std::result::Result<(), BanksClientError> {
        let mut ixs = ixs;
        let start_ix = self
            .make_lending_account_start_flashloan_ix(ixs.len() as u64 + 1)
            .await;
        let end_ix = self
            .make_lending_account_end_flashloan_ix(include_banks, exclude_banks)
            .await;

        ixs.insert(0, start_ix);
        ixs.push(end_ix);

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;

        let signers = if let Some(signer) = signer {
            vec![&payer, signer]
        } else {
            vec![&payer]
        };

        let tx =
            Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &signers, blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn load_observation_account_metas(
        &self,
        include_banks: Vec<Pubkey>,
        exclude_banks: Vec<Pubkey>,
    ) -> Vec<AccountMeta> {
        self.load_observation_account_metas_with_flags(include_banks, exclude_banks, false, false)
            .await
    }

    pub async fn load_observation_account_metas_with_flags(
        &self,
        include_banks: Vec<Pubkey>,
        exclude_banks: Vec<Pubkey>,
        bank_writable: bool,
        banks_only: bool,
    ) -> Vec<AccountMeta> {
        let marginfi_account = self.load().await;
        // Check all active banks in marginfi account balances
        let mut bank_pks = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter_map(|balance| {
                if balance.is_active() {
                    Some(balance.bank_pk)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Add bank pks in include_banks if they are not already in bank_pks
        // (and exclude the ones contained in exclude_banks)
        for bank_pk in include_banks {
            if !bank_pks.contains(&bank_pk) {
                bank_pks.push(bank_pk);
            }
        }
        bank_pks.retain(|bank_pk| !exclude_banks.contains(bank_pk));

        // Sort all bank_pks in descending order
        bank_pks.sort_by(|a, b| b.cmp(a));

        // Load all banks
        let mut banks = vec![];
        for bank_pk in bank_pks.clone() {
            let bank = load_and_deserialize::<Bank>(self.ctx.clone(), &bank_pk).await;
            banks.push(bank);
        }

        // Bank -> AccountMetas
        let account_metas = banks
            .iter()
            .zip(bank_pks.iter())
            .flat_map(|(bank, bank_pk)| {
                // The bank is included for all oracle types
                let mut metas = vec![AccountMeta {
                    pubkey: *bank_pk,
                    is_signer: false,
                    is_writable: bank_writable,
                }];

                if banks_only {
                    return metas;
                }

                // Oracle meta is included for all but fixed-price banks
                if bank.config.oracle_setup != OracleSetup::Fixed {
                    let oracle_key = {
                        let oracle_key = bank.config.oracle_keys[0];
                        get_oracle_id_from_feed_id(oracle_key).unwrap_or(oracle_key)
                    };

                    metas.push(AccountMeta {
                        pubkey: oracle_key,
                        is_signer: false,
                        is_writable: false,
                    });
                }
                metas
            })
            .collect::<Vec<_>>();
        account_metas
    }

    pub async fn load_observation_account_metas_close_last(
        &self,
        close_bank: Pubkey,
        include_banks: Vec<Pubkey>,
        exclude_banks: Vec<Pubkey>,
    ) -> Vec<AccountMeta> {
        let marginfi_account = self.load().await;
        let mut bank_pks = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter_map(|balance| {
                if balance.is_active() {
                    Some(balance.bank_pk)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for bank_pk in include_banks {
            if !bank_pks.contains(&bank_pk) {
                bank_pks.push(bank_pk);
            }
        }
        bank_pks.retain(|bank_pk| !exclude_banks.contains(bank_pk));

        let mut other_banks: Vec<Pubkey> = bank_pks
            .into_iter()
            .filter(|pk| *pk != close_bank)
            .collect();
        other_banks.sort_by(|a, b| b.cmp(a));

        let mut ordered = other_banks;
        if !exclude_banks.contains(&close_bank) {
            ordered.push(close_bank);
        }

        let mut banks = vec![];
        for bank_pk in ordered.clone() {
            let bank = load_and_deserialize::<Bank>(self.ctx.clone(), &bank_pk).await;
            banks.push(bank);
        }

        let account_metas = banks
            .iter()
            .zip(ordered.iter())
            .flat_map(|(bank, bank_pk)| {
                let mut metas = vec![AccountMeta {
                    pubkey: *bank_pk,
                    is_signer: false,
                    is_writable: false,
                }];

                if bank.config.oracle_setup != OracleSetup::Fixed {
                    let oracle_key = {
                        let oracle_key = bank.config.oracle_keys[0];
                        get_oracle_id_from_feed_id(oracle_key).unwrap_or(oracle_key)
                    };

                    metas.push(AccountMeta {
                        pubkey: oracle_key,
                        is_signer: false,
                        is_writable: false,
                    });
                }
                metas
            })
            .collect::<Vec<_>>();

        account_metas
    }

    pub async fn set_account(&self, mfi_account: &MarginfiAccount) -> anyhow::Result<()> {
        let mut ctx = self.ctx.borrow_mut();
        let mut account = ctx.banks_client.get_account(self.key).await?.unwrap();
        let mut discriminator = account.data[..8].to_vec();
        let mut new_data = vec![];
        new_data.append(&mut discriminator);
        new_data.append(&mut bytemuck::bytes_of(mfi_account).to_vec());
        account.data = new_data;
        ctx.set_account(&self.key, &account.into());

        Ok(())
    }

    pub async fn load(&self) -> MarginfiAccount {
        load_and_deserialize::<MarginfiAccount>(self.ctx.clone(), &self.key).await
    }

    pub fn get_size() -> usize {
        mem::size_of::<MarginfiAccount>() + 8
    }

    async fn build_transfer_account(
        &self,
        new_marginfi_account: Pubkey,
        new_authority: Pubkey,
        signer_keypair: Option<Keypair>,
        fee_payer_keypair: Option<Keypair>,
        new_account_keypair: &Keypair,
        global_fee_wallet: Pubkey,
    ) -> Transaction {
        let marginfi_account = self.load().await;
        let ctx = self.ctx.borrow();
        let signer = signer_keypair.unwrap_or_else(|| ctx.payer.insecure_clone());
        let fee_payer = fee_payer_keypair.unwrap_or_else(|| ctx.payer.insecure_clone());

        let transfer_account_ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::TransferToNewAccount {
                old_marginfi_account: self.key,
                new_marginfi_account,
                group: marginfi_account.group,
                authority: signer.pubkey(),
                fee_payer: fee_payer.pubkey(),
                new_authority,
                global_fee_wallet,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: marginfi::instruction::TransferToNewAccount {}.data(),
        };

        let mut signers = vec![new_account_keypair];
        let is_signer_fee_payer = signer.pubkey() == fee_payer.pubkey();

        if is_signer_fee_payer {
            signers.push(&signer);
        } else {
            signers.push(&signer);
            signers.push(&fee_payer);
        }

        Transaction::new_signed_with_payer(
            &[transfer_account_ix],
            Some(&fee_payer.pubkey()),
            &signers,
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        )
    }

    /// Build and send the "transfer TransferToNewAccount transaction.
    /// Pass the new authority as an argument
    /// Optional: use a different signer (for negative test case)
    /// Optional: use a different fee_payer (for testing separate fee payer)
    pub async fn try_transfer_account(
        &self,
        new_marginfi_account: Pubkey,
        new_authority: Pubkey,
        signer_keypair: Option<Keypair>,
        fee_payer_keypair: Option<Keypair>,
        new_account_keypair: &Keypair,
        global_fee_wallet: Pubkey,
    ) -> std::result::Result<(), BanksClientError> {
        let tx = self
            .build_transfer_account(
                new_marginfi_account,
                new_authority,
                signer_keypair,
                fee_payer_keypair,
                new_account_keypair,
                global_fee_wallet,
            )
            .await;
        let (banks_client, _, _) = ctx_parts(&self.ctx).await;
        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    /// Build (but don't send) the "transfer TransferToNewAccount transaction.
    /// Pass the new authority as an argument
    /// Optional: use a different signer (for negative test case)
    /// Optional: use a different fee_payer (for testing separate fee payer)
    pub async fn get_tx_transfer_account(
        &self,
        new_marginfi_account: Pubkey,
        new_authority: Pubkey,
        signer_keypair: Option<Keypair>,
        fee_payer_keypair: Option<Keypair>,
        new_account_keypair: &Keypair,
        global_fee_wallet: Pubkey,
    ) -> Transaction {
        self.build_transfer_account(
            new_marginfi_account,
            new_authority,
            signer_keypair,
            fee_payer_keypair,
            new_account_keypair,
            global_fee_wallet,
        )
        .await
    }

    pub async fn try_close_account(&self, nonce: u64) -> std::result::Result<(), BanksClientError> {
        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiAccountClose {
                marginfi_account: self.key,
                authority: payer.pubkey(),
                fee_payer: payer.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountClose {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ComputeBudgetInstruction::set_compute_unit_price(nonce), ix],
            Some(&payer.pubkey()),
            &[&payer],
            blockhash,
        );

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn nullify_assets_for_bank(&mut self, bank_pk: Pubkey) -> anyhow::Result<()> {
        let mut user_mfi_account: MarginfiAccount = self.load().await;

        let balance_index = user_mfi_account
            .lending_account
            .balances
            .iter()
            .position(|b| b.is_active() && b.bank_pk == bank_pk)
            .unwrap();

        user_mfi_account.lending_account.balances[balance_index].asset_shares = I80F48::ZERO.into();
        self.set_account(&user_mfi_account).await
    }

    pub async fn make_start_liquidation_ix(
        &self,
        liquidation_record: Pubkey,
        liquidation_receiver: Pubkey,
    ) -> Instruction {
        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::StartLiquidation {
                marginfi_account: self.key,
                liquidation_record,
                liquidation_receiver,
                instruction_sysvar: sysvar::instructions::id(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::StartLiquidation {}.data(),
        };
        ix.accounts.extend_from_slice(
            &self
                .load_observation_account_metas_with_flags(vec![], vec![], true, false)
                .await,
        );
        ix
    }

    pub async fn make_end_liquidation_ix(
        &self,
        liquidation_record: Pubkey,
        liquidation_receiver: Pubkey,
        fee_state: Pubkey,
        global_fee_wallet: Pubkey,
        exclude_banks: Vec<Pubkey>,
    ) -> Instruction {
        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EndLiquidation {
                marginfi_account: self.key,
                liquidation_record,
                liquidation_receiver,
                fee_state,
                global_fee_wallet,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::EndLiquidation {}.data(),
        };
        ix.accounts.extend_from_slice(
            &self
                .load_observation_account_metas_with_flags(vec![], exclude_banks, true, true)
                .await,
        );
        ix
    }

    pub async fn make_init_liquidation_record_ix(
        &self,
        liquidation_record: Pubkey,
        payer: Pubkey,
    ) -> Instruction {
        Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::InitLiquidationRecord {
                marginfi_account: self.key,
                fee_payer: payer,
                liquidation_record,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountInitLiqRecord {}.data(),
        }
    }

    pub async fn make_kamino_refresh_reserve_ix(&self, bank: &BankFixture) -> Instruction {
        let bank_state = bank.load().await;
        let (lending_market, pyth_oracle, scope_prices) = if let Some(kamino) = &bank.kamino {
            let pyth = kamino.reserve.config.token_info.pyth_configuration.price;
            (
                kamino.reserve.lending_market,
                (pyth != Pubkey::default()).then_some(pyth),
                None,
            )
        } else {
            let reserve: MinimalReserve =
                load_and_deserialize(self.ctx.clone(), &bank_state.integration_acc_1).await;
            let oracle_key = get_oracle_id_from_feed_id(bank_state.config.oracle_keys[0])
                .unwrap_or(bank_state.config.oracle_keys[0]);
            (reserve.lending_market, Some(oracle_key), None)
        };

        let accounts = kamino::accounts::RefreshReserve {
            reserve: bank_state.integration_acc_1,
            lending_market,
            pyth_oracle,
            switchboard_price_oracle: None,
            switchboard_twap_oracle: None,
            scope_prices,
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: kamino_mocks::kamino_lending::ID,
            accounts,
            data: kamino::args::RefreshReserve {}.data(),
        }
    }

    pub async fn make_kamino_refresh_obligation_ix(&self, bank: &BankFixture) -> Instruction {
        let bank_state = bank.load().await;
        let lending_market = if let Some(kamino) = &bank.kamino {
            kamino.obligation.lending_market
        } else {
            let obligation: MinimalObligation =
                load_and_deserialize(self.ctx.clone(), &bank_state.integration_acc_2).await;
            obligation.lending_market
        };

        let mut accounts = kamino::accounts::RefreshObligation {
            obligation: bank_state.integration_acc_2,
            lending_market,
        }
        .to_account_metas(Some(true));
        accounts.push(AccountMeta::new_readonly(
            bank_state.integration_acc_1,
            false,
        ));

        Instruction {
            program_id: kamino_mocks::kamino_lending::ID,
            accounts,
            data: kamino::args::RefreshObligation {}.data(),
        }
    }

    pub async fn make_kamino_deposit_ix(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        amount: u64,
    ) -> Instruction {
        self.make_kamino_deposit_ix_with_authority(
            funding_account,
            bank,
            amount,
            self.ctx.borrow().payer.pubkey(),
        )
        .await
    }

    pub async fn make_kamino_deposit_ix_with_authority(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        amount: u64,
        authority: Pubkey,
    ) -> Instruction {
        let marginfi_account = self.load().await;
        let bank_state = bank.load().await;
        let reserve: MinimalReserve =
            load_and_deserialize(self.ctx.clone(), &bank_state.integration_acc_1).await;
        let lending_market_authority =
            derive_kamino_lending_market_authority(reserve.lending_market);

        Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoDeposit {
                group: marginfi_account.group,
                marginfi_account: self.key,
                authority,
                bank: bank.key,
                signer_token_account: funding_account,
                liquidity_vault_authority: bank.get_vault_authority(BankVaultType::Liquidity).0,
                liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
                integration_acc_2: bank_state.integration_acc_2,
                lending_market: reserve.lending_market,
                lending_market_authority,
                integration_acc_1: bank_state.integration_acc_1,
                mint: reserve.mint_pubkey,
                reserve_liquidity_supply: reserve.supply_vault,
                reserve_collateral_mint: reserve.collateral_mint_pubkey,
                reserve_destination_deposit_collateral: reserve.collateral_supply_vault,
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: kamino_mocks::kamino_lending::ID,
                farms_program: kamino_mocks::kamino_farms::ID,
                collateral_token_program: anchor_spl::token::spl_token::ID,
                liquidity_token_program: bank.get_token_program(),
                instruction_sysvar_account: sysvar::instructions::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::KaminoDeposit { amount }.data(),
        }
    }

    pub async fn make_kamino_withdraw_ix(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        amount: u64,
        withdraw_all: Option<bool>,
        include_health_accounts: bool,
    ) -> Instruction {
        self.make_kamino_withdraw_ix_with_authority(
            destination_account,
            bank,
            amount,
            withdraw_all,
            include_health_accounts,
            self.ctx.borrow().payer.pubkey(),
        )
        .await
    }

    pub async fn make_kamino_withdraw_ix_with_authority(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        amount: u64,
        withdraw_all: Option<bool>,
        include_health_accounts: bool,
        authority: Pubkey,
    ) -> Instruction {
        let marginfi_account = self.load().await;
        let bank_state = bank.load().await;
        let reserve: MinimalReserve =
            load_and_deserialize(self.ctx.clone(), &bank_state.integration_acc_1).await;
        let lending_market_authority =
            derive_kamino_lending_market_authority(reserve.lending_market);

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KaminoWithdraw {
                group: marginfi_account.group,
                marginfi_account: self.key,
                authority,
                bank: bank.key,
                destination_token_account: destination_account,
                liquidity_vault_authority: bank.get_vault_authority(BankVaultType::Liquidity).0,
                liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
                integration_acc_2: bank_state.integration_acc_2,
                lending_market: reserve.lending_market,
                lending_market_authority,
                integration_acc_1: bank_state.integration_acc_1,
                reserve_liquidity_mint: reserve.mint_pubkey,
                reserve_liquidity_supply: reserve.supply_vault,
                reserve_collateral_mint: reserve.collateral_mint_pubkey,
                reserve_source_collateral: reserve.collateral_supply_vault,
                obligation_farm_user_state: None,
                reserve_farm_state: None,
                kamino_program: kamino_mocks::kamino_lending::ID,
                farms_program: kamino_mocks::kamino_farms::ID,
                collateral_token_program: anchor_spl::token::spl_token::ID,
                liquidity_token_program: bank.get_token_program(),
                instruction_sysvar_account: sysvar::instructions::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::KaminoWithdraw {
                amount,
                withdraw_all,
            }
            .data(),
        };

        if include_health_accounts {
            let oracle_key = get_oracle_id_from_feed_id(bank_state.config.oracle_keys[0])
                .unwrap_or(bank_state.config.oracle_keys[0]);
            ix.accounts.push(AccountMeta::new_readonly(bank.key, false));
            ix.accounts
                .push(AccountMeta::new_readonly(oracle_key, false));
            ix.accounts.push(AccountMeta::new_readonly(
                bank_state.integration_acc_1,
                false,
            ));
        }

        ix
    }

    pub async fn try_lending_account_pulse_health(
        &self,
    ) -> std::result::Result<(), BanksClientError> {
        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PulseHealth {
                marginfi_account: self.key,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingAccountPulseHealth {}.data(),
        };

        // Add bank and oracle accounts for pulse_health (need to pass banks and oracles for all active balances)
        ix.accounts
            .extend_from_slice(&self.load_observation_account_metas(vec![], vec![]).await);

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx).await;
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn make_start_deleverage_ix(
        &self,
        liquidation_record: Pubkey,
        risk_admin: Pubkey,
    ) -> Instruction {
        let marginfi_account = self.load().await;

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::StartDeleverage {
                marginfi_account: self.key,
                liquidation_record,
                group: marginfi_account.group,
                risk_admin,
                instruction_sysvar: sysvar::instructions::id(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::StartDeleverage {}.data(),
        };
        ix.accounts.extend_from_slice(
            &self
                .load_observation_account_metas_with_flags(vec![], vec![], true, false)
                .await,
        );
        ix
    }

    pub async fn make_end_deleverage_ix(
        &self,
        liquidation_record: Pubkey,
        risk_admin: Pubkey,
        exclude_banks: Vec<Pubkey>,
    ) -> Instruction {
        let marginfi_account = self.load().await;

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EndDeleverage {
                marginfi_account: self.key,
                liquidation_record,
                group: marginfi_account.group,
                risk_admin,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::EndDeleverage {}.data(),
        };
        ix.accounts.extend_from_slice(
            &self
                .load_observation_account_metas_with_flags(vec![], exclude_banks, true, true)
                .await,
        );
        ix
    }

    pub async fn try_place_order(
        &self,
        bank_keys: Vec<Pubkey>,
        trigger: OrderTrigger,
    ) -> std::result::Result<Pubkey, BanksClientError> {
        let marginfi_account = self.load().await;
        // Compute fee_state PDA and fetch the global_fee_wallet from it so we can pass both
        // accounts to the PlaceOrder instruction.
        let (fee_state_key, _bump) = Pubkey::find_program_address(
            &[marginfi_type_crate::constants::FEE_STATE_SEED.as_bytes()],
            &marginfi::ID,
        );

        // Clone banks_client so we don't hold the RefCell borrow across await points.
        let banks_client = {
            let ctx = self.ctx.borrow();
            ctx.banks_client.clone()
        };
        let fee_state_account = banks_client
            .get_account(fee_state_key)
            .await?
            .expect("fee_state account must exist for tests");
        let fee_state_data: FeeState =
            FeeState::try_deserialize(&mut &fee_state_account.data[..]).expect("invalid fee_state");
        let global_fee_wallet = fee_state_data.global_fee_wallet;

        let ctx = self.ctx.borrow();

        let (order_pda, _) = find_order_pda(&self.key, &bank_keys);

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PlaceOrder {
                group: marginfi_account.group,
                marginfi_account: self.key,
                fee_payer: ctx.payer.pubkey(),
                authority: ctx.payer.pubkey(),
                order: order_pda,
                fee_state: fee_state_key,
                global_fee_wallet,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountPlaceOrder { bank_keys, trigger }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        drop(ctx);
        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await?;

        Ok(order_pda)
    }

    pub async fn try_close_order(
        &self,
        order: Pubkey,
        fee_recipient: Pubkey,
    ) -> std::result::Result<(), BanksClientError> {
        let ctx = self.ctx.borrow();

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::CloseOrder {
                marginfi_account: self.key,
                authority: ctx.payer.pubkey(),
                order,
                fee_recipient,
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountCloseOrder {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        drop(ctx);
        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn try_keeper_close_order(
        &self,
        order: Pubkey,
        keeper: &Keypair,
        fee_recipient: Pubkey,
    ) -> std::result::Result<(), BanksClientError> {
        let ctx = self.ctx.borrow();

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::KeeperCloseOrder {
                marginfi_account: self.key,
                order,
                fee_recipient,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountKeeperCloseOrder {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&keeper.pubkey()),
            &[keeper],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        drop(ctx);
        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn try_set_keeper_close_flags(
        &self,
        bank_keys_opt: Option<Vec<Pubkey>>,
    ) -> std::result::Result<(), BanksClientError> {
        let ctx = self.ctx.borrow();

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::SetKeeperCloseFlags {
                marginfi_account: self.key,
                authority: ctx.payer.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountSetKeeperCloseFlags { bank_keys_opt }
                .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        drop(ctx);
        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    pub async fn load_order(&self, order: Pubkey) -> Order {
        load_and_deserialize::<Order>(self.ctx.clone(), &order).await
    }

    pub async fn make_start_execute_ix(
        &self,
        order: Pubkey,
        executor: Pubkey,
    ) -> (Instruction, Pubkey) {
        self.make_start_execute_ix_with_metas(order, executor, None)
            .await
    }

    pub async fn make_start_execute_ix_with_metas(
        &self,
        order: Pubkey,
        executor: Pubkey,
        observation_metas: Option<Vec<AccountMeta>>,
    ) -> (Instruction, Pubkey) {
        let marginfi_account = self.load().await;
        let (execute_record, _) = find_execute_order_pda(&order);

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::StartExecuteOrder {
                group: marginfi_account.group,
                marginfi_account: self.key,
                fee_payer: executor,
                executor,
                order,
                execute_record,
                instruction_sysvar: sysvar::instructions::id(),
                system_program: system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountStartExecuteOrder {}.data(),
        };

        let observation_metas = match observation_metas {
            Some(metas) => metas,
            None => self.load_observation_account_metas(vec![], vec![]).await,
        };

        ix.accounts.extend_from_slice(&observation_metas);

        (ix, execute_record)
    }

    pub async fn make_end_execute_ix(
        &self,
        order: Pubkey,
        execute_record: Pubkey,
        executor: Pubkey,
        fee_recipient: Pubkey,
        exclude_banks: Vec<Pubkey>,
    ) -> Instruction {
        let marginfi_account = self.load().await;

        let mut ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::EndExecuteOrder {
                group: marginfi_account.group,
                marginfi_account: self.key,
                executor,
                fee_recipient,
                order,
                execute_record,
                fee_state: Pubkey::find_program_address(
                    &[marginfi_type_crate::constants::FEE_STATE_SEED.as_bytes()],
                    &marginfi::ID,
                )
                .0,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountEndExecuteOrder {}.data(),
        };

        ix.accounts.extend_from_slice(
            &self
                .load_observation_account_metas(vec![], exclude_banks)
                .await,
        );

        ix
    }
}
