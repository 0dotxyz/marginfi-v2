use super::{bank::BankFixture, prelude::*};
use crate::ui_to_native;
use anchor_lang::{prelude::*, system_program, InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use kamino_mocks::kamino_lending::client as kamino;
use marginfi::state::bank::BankVaultType;
use marginfi_type_crate::types::OracleSetup;
use marginfi_type_crate::types::{Bank, MarginfiAccount};
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

fn ctx_parts(ctx: &Rc<RefCell<ProgramTestContext>>) -> (BanksClient, Keypair, Hash) {
    let ctx_ref = ctx.borrow();
    (
        ctx_ref.banks_client.clone(),
        ctx_ref.payer.insecure_clone(),
        ctx_ref.last_blockhash,
    )
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

        let (banks_client, payer, blockhash) = ctx_parts(&ctx_ref);
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

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);
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

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);
        let tx =
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash);

        banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await
    }

    async fn make_bank_withdraw_ix_internal<T: Into<f64>>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        withdraw_all: Option<bool>,
        is_liquidate: bool,
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

        let exclude_vec = match withdraw_all.unwrap_or(false) {
            true => {
                if is_liquidate {
                    vec![]
                } else {
                    vec![bank.key]
                }
            }
            false => vec![],
        };
        ix.accounts.extend_from_slice(
            &self
                .load_observation_account_metas(vec![], exclude_vec)
                .await,
        );

        ix
    }

    pub async fn make_bank_withdraw_ix<T: Into<f64>>(
        &self,
        destination_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        withdraw_all: Option<bool>,
        is_liquidate: bool,
    ) -> Instruction {
        self.make_bank_withdraw_ix_internal(
            destination_account,
            bank,
            ui_amount,
            withdraw_all,
            is_liquidate,
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
            .make_bank_withdraw_ix_internal(
                destination_account,
                bank,
                ui_amount,
                withdraw_all,
                false,
                authority.pubkey(),
            )
            .await;

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);
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

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);
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

    pub async fn make_bank_repay_ix<T: Into<f64>>(
        &self,
        funding_account: Pubkey,
        bank: &BankFixture,
        ui_amount: T,
        repay_all: Option<bool>,
    ) -> Instruction {
        let marginfi_account = self.load().await;
        let ctx = self.ctx.borrow();

        let mut accounts = marginfi::accounts::LendingAccountRepay {
            group: marginfi_account.group,
            marginfi_account: self.key,
            authority: ctx.payer.pubkey(),
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
            data: marginfi::instruction::LendingAccountRepay {
                amount: ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
                repay_all,
            }
            .data(),
        }
    }

    async fn make_bank_repay_ix_internal<T: Into<f64>>(
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

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingAccountRepay {
                amount: ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
                repay_all,
            }
            .data(),
        }
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
            .make_bank_repay_ix_internal(
                funding_account,
                bank,
                ui_amount,
                repay_all,
                authority.pubkey(),
            )
            .await;
        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);
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
        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);

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

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);
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

    pub async fn try_withdraw_emissions(
        &self,
        bank: &BankFixture,
        recv_account: &TokenAccountFixture,
    ) -> std::result::Result<(), BanksClientError> {
        let emissions_mint = bank.load().await.emissions_mint;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingAccountWithdrawEmissions {
                group: self.load().await.group,
                marginfi_account: self.key,
                authority: self.ctx.borrow().payer.pubkey(),
                emissions_mint,
                emissions_auth: get_emissions_authority_address(bank.key, emissions_mint).0,
                emissions_vault: get_emissions_token_account_address(bank.key, emissions_mint).0,
                destination_account: recv_account.key,
                bank: bank.key,
                token_program: recv_account.token_program,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingAccountWithdrawEmissions {}.data(),
        };

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);
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

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);

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
                    is_writable: false,
                }];

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
            ctx.last_blockhash,
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
        let (banks_client, _, _) = ctx_parts(&self.ctx);
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
        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);

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
        ix.accounts
            .extend_from_slice(&self.load_observation_account_metas(vec![], vec![]).await);
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
                .load_observation_account_metas(vec![], exclude_banks)
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
        let reserve = &bank.kamino.as_ref().unwrap().reserve;
        let bank = bank.load().await;

        let accounts = kamino::accounts::RefreshReserve {
            reserve: bank.integration_acc_1,
            lending_market: reserve.lending_market,
            pyth_oracle: None,
            switchboard_price_oracle: None,
            switchboard_twap_oracle: None,
            scope_prices: Some(reserve.config.token_info.scope_configuration.price_feed),
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: kamino_mocks::kamino_lending::ID,
            accounts,
            data: kamino::args::RefreshReserve {}.data(),
        }
    }

    pub async fn make_kamino_refresh_obligation_ix(&self, bank: &BankFixture) -> Instruction {
        let obligation = &bank.kamino.as_ref().unwrap().obligation;
        let bank = bank.load().await;

        let accounts = kamino::accounts::RefreshObligation {
            obligation: bank.integration_acc_2,
            lending_market: obligation.lending_market,
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: kamino_mocks::kamino_lending::ID,
            accounts,
            data: kamino::args::RefreshObligation {}.data(),
        }
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

        let (banks_client, payer, blockhash) = ctx_parts(&self.ctx);
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
        ix.accounts
            .extend_from_slice(&self.load_observation_account_metas(vec![], vec![]).await);
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
                .load_observation_account_metas(vec![], exclude_banks)
                .await,
        );
        ix
    }
}
