use super::utils::load_and_deserialize;
use crate::{
    kamino::KaminoFixture,
    prelude::{
        get_emissions_authority_address, get_emissions_token_account_address, MintFixture,
        TokenAccountFixture,
    },
};
use anchor_lang::{
    prelude::{AccountMeta, Pubkey},
    InstructionData, ToAccountMetas,
};
use fixed::types::I80F48;
use marginfi::{
    bank_authority_seed,
    state::{
        bank::BankVaultType,
        price::{OraclePriceFeedAdapter, OraclePriceType, PriceAdapter},
    },
    utils::{find_bank_vault_authority_pda, find_bank_vault_pda},
};
use marginfi_type_crate::types::{Bank, BankConfigOpt, OracleSetup};
use solana_program::{
    account_info::IntoAccountInfo, instruction::Instruction, sysvar::clock::Clock,
};
use solana_program_test::BanksClientError;
use solana_program_test::ProgramTestContext;
use solana_sdk::{commitment_config::CommitmentLevel, signer::Signer, transaction::Transaction};
use std::{cell::RefCell, fmt::Debug, rc::Rc};

#[derive(Clone)]
pub struct BankFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
    pub mint: MintFixture,
    pub kamino: Option<KaminoFixture>,
}

impl BankFixture {
    pub fn new(
        ctx: Rc<RefCell<ProgramTestContext>>,
        key: Pubkey,
        mint_fixture: &MintFixture,
        kamino: Option<KaminoFixture>,
    ) -> Self {
        Self {
            ctx,
            key,
            mint: mint_fixture.clone(),
            kamino,
        }
    }

    pub fn get_token_program(&self) -> Pubkey {
        self.mint.token_program
    }

    pub fn get_vault(&self, vault_type: BankVaultType) -> (Pubkey, u8) {
        find_bank_vault_pda(&self.key, vault_type)
    }

    pub fn get_vault_authority(&self, vault_type: BankVaultType) -> (Pubkey, u8) {
        find_bank_vault_authority_pda(&self.key, vault_type)
    }

    pub async fn get_price(&self) -> f64 {
        let bank = self.load().await;
        let oracle_adapter = match bank.config.oracle_setup {
            OracleSetup::Fixed => {
                OraclePriceFeedAdapter::try_from_bank(&bank, &[], &Clock::default()).unwrap()
            }
            _ => {
                let oracle_key = bank.config.oracle_keys[0];
                let mut oracle_account = self
                    .ctx
                    .borrow_mut()
                    .banks_client
                    .get_account(oracle_key)
                    .await
                    .unwrap()
                    .unwrap();
                let ai = (&oracle_key, &mut oracle_account).into_account_info();
                OraclePriceFeedAdapter::try_from_bank(&bank, &[ai], &Clock::default()).unwrap()
            }
        };

        oracle_adapter
            .get_price_of_type(
                OraclePriceType::RealTime,
                None,
                bank.config.oracle_max_confidence,
            )
            .unwrap()
            .to_num()
    }

    pub async fn load(&self) -> Bank {
        load_and_deserialize::<Bank>(self.ctx.clone(), &self.key).await
    }

    pub async fn update_config(
        &self,
        config: BankConfigOpt,
        oracle_update: Option<(u8, Pubkey)>,
    ) -> anyhow::Result<()> {
        let mut instructions = Vec::new();

        let accounts = marginfi::accounts::LendingPoolConfigureBank {
            group: self.load().await.group,
            admin: self.ctx.borrow().payer.pubkey(),
            bank: self.key,
        }
        .to_account_metas(Some(true));

        let config_ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingPoolConfigureBank {
                bank_config_opt: config,
            }
            .data(),
        };

        instructions.push(config_ix);

        if let Some((setup, oracle)) = oracle_update {
            let mut oracle_accounts = marginfi::accounts::LendingPoolConfigureBank {
                group: self.load().await.group,
                admin: self.ctx.borrow().payer.pubkey(),
                bank: self.key,
            }
            .to_account_metas(Some(true));

            oracle_accounts.push(AccountMeta::new_readonly(oracle, false));

            let oracle_ix = Instruction {
                program_id: marginfi::ID,
                accounts: oracle_accounts,
                data: marginfi::instruction::LendingPoolConfigureBankOracle { setup, oracle }
                    .data(),
            };

            instructions.push(oracle_ix);
        }

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            self.ctx.borrow().last_blockhash,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(())
    }

    pub async fn try_setup_emissions(
        &self,
        flags: u64,
        rate: u64,
        total_emissions: u64,
        emissions_mint: Pubkey,
        funding_account: Pubkey,
        token_program: Pubkey,
    ) -> Result<(), BanksClientError> {
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolSetupEmissions {
                group: self.load().await.group,
                delegate_emissions_admin: self.ctx.borrow().payer.pubkey(),
                bank: self.key,
                emissions_mint,
                emissions_funding_account: funding_account,
                emissions_auth: get_emissions_authority_address(self.key, emissions_mint).0,
                emissions_token_account: get_emissions_token_account_address(
                    self.key,
                    emissions_mint,
                )
                .0,
                token_program,
                system_program: solana_program::system_program::id(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingPoolSetupEmissions {
                rate,
                flags,
                total_emissions,
            }
            .data(),
        };

        let tx = {
            let ctx = self.ctx.borrow_mut();

            Transaction::new_signed_with_payer(
                &[ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer],
                ctx.last_blockhash,
            )
        };

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(())
    }

    pub async fn try_update_emissions(
        &self,
        emissions_flags: Option<u64>,
        emissions_rate: Option<u64>,
        additional_emissions: Option<(u64, Pubkey)>,
        token_program: Pubkey,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolUpdateEmissionsParameters {
                group: self.load().await.group,
                delegate_emissions_admin: self.ctx.borrow().payer.pubkey(),
                bank: self.key,
                emissions_mint: bank.emissions_mint,
                emissions_funding_account: additional_emissions.map(|(_, f)| f).unwrap_or_default(),
                emissions_token_account: get_emissions_token_account_address(
                    self.key,
                    bank.emissions_mint,
                )
                .0,
                token_program,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingPoolUpdateEmissionsParameters {
                emissions_flags,
                emissions_rate,
                additional_emissions: additional_emissions.map(|(a, _)| a),
            }
            .data(),
        };

        let tx = {
            let ctx = self.ctx.borrow_mut();

            Transaction::new_signed_with_payer(
                &[ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer],
                ctx.last_blockhash,
            )
        };

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(())
    }

    pub async fn try_withdraw_fees(
        &self,
        receiving_account: &TokenAccountFixture,
        amount: u64,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;
        let ctx = self.ctx.borrow_mut();
        let signer_pk = ctx.payer.pubkey();
        let (fee_vault_authority, _) = Pubkey::find_program_address(
            bank_authority_seed!(BankVaultType::Fee, self.key),
            &marginfi::ID,
        );

        let mut accounts = marginfi::accounts::LendingPoolWithdrawFees {
            group: bank.group,
            token_program: receiving_account.token_program,
            bank: self.key,
            admin: signer_pk,
            fee_vault: bank.fee_vault,
            fee_vault_authority,
            dst_token_account: receiving_account.key,
        }
        .to_account_metas(Some(true));
        if self.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(self.mint.key, false));
        }

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingPoolWithdrawFees { amount }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn try_withdraw_fees_permissionless(
        &self,
        receiving_account: &TokenAccountFixture,
        amount: u64,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;
        let ctx = self.ctx.borrow_mut();
        let (fee_vault_authority, _) = Pubkey::find_program_address(
            bank_authority_seed!(BankVaultType::Fee, self.key),
            &marginfi::ID,
        );

        let mut accounts = marginfi::accounts::LendingPoolWithdrawFeesPermissionless {
            group: bank.group,
            token_program: receiving_account.token_program,
            bank: self.key,
            fee_vault: bank.fee_vault,
            fee_vault_authority,
            fees_destination_account: receiving_account.key,
        }
        .to_account_metas(Some(true));
        if self.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(self.mint.key, false));
        }

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingPoolWithdrawFeesPermissionless { amount }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client
            .process_transaction_with_preflight_and_commitment(tx, CommitmentLevel::Confirmed)
            .await?;

        Ok(())
    }

    pub async fn try_set_fees_destination_account(
        &self,
        destination_account: &TokenAccountFixture,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;
        let ctx = self.ctx.borrow_mut();
        let signer_pk = ctx.payer.pubkey();

        let mut accounts = marginfi::accounts::LendingPoolUpdateFeesDestinationAccount {
            group: bank.group,
            bank: self.key,
            admin: signer_pk,
            destination_account: destination_account.key,
        }
        .to_account_metas(Some(true));
        if self.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(self.mint.key, false));
        }

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingPoolUpdateFeesDestinationAccount.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn try_withdraw_insurance(
        &self,
        receiving_account: &TokenAccountFixture,
        amount: u64,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;
        let ctx = self.ctx.borrow_mut();
        let signer_pk = ctx.payer.pubkey();
        let (insurance_vault_authority, _) = Pubkey::find_program_address(
            bank_authority_seed!(BankVaultType::Insurance, self.key),
            &marginfi::ID,
        );

        let mut accounts = marginfi::accounts::LendingPoolWithdrawInsurance {
            group: bank.group,
            token_program: receiving_account.token_program,
            bank: self.key,
            admin: signer_pk,
            insurance_vault: bank.insurance_vault,
            insurance_vault_authority,
            dst_token_account: receiving_account.key,
        }
        .to_account_metas(Some(true));
        if self.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(self.mint.key, false));
        }

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingPoolWithdrawInsurance { amount }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn get_vault_token_account(&self, vault_type: BankVaultType) -> TokenAccountFixture {
        let (vault, _) = self.get_vault(vault_type);

        TokenAccountFixture::fetch(self.ctx.clone(), vault).await
    }

    pub async fn set_cache_price_and_confidence(&self, price: I80F48, confidence: I80F48) {
        let mut bank_ai = self
            .ctx
            .borrow_mut()
            .banks_client
            .get_account(self.key)
            .await
            .unwrap()
            .unwrap();
        let bank = bytemuck::from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);

        bank.cache.last_oracle_price = price.into();
        bank.cache.last_oracle_price_confidence = confidence.into();

        self.ctx
            .borrow_mut()
            .set_account(&self.key, &bank_ai.into());
    }

    pub async fn set_asset_share_value(&self, value: I80F48) {
        let mut bank_ai = self
            .ctx
            .borrow_mut()
            .banks_client
            .get_account(self.key)
            .await
            .unwrap()
            .unwrap();
        let bank = bytemuck::from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);

        bank.asset_share_value = value.into();

        self.ctx
            .borrow_mut()
            .set_account(&self.key, &bank_ai.into());
    }
}

impl Debug for BankFixture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BankFixture")
            .field("key", &self.key)
            .finish()
    }
}
