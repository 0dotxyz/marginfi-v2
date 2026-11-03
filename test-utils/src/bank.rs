use super::utils::{latest_blockhash, load_and_deserialize};
use crate::{
    kamino::KaminoFixture,
    prelude::{MintFixture, TokenAccountFixture},
};
use anchor_lang::{
    prelude::{AccountMeta, Pubkey},
    solana_program::{account_info::IntoAccountInfo, clock::Clock, instruction::Instruction},
    InstructionData, ToAccountMetas,
};
use fixed::types::I80F48;
use marginfi::state::price::{OraclePriceFeedAdapter, PriceAdapter};
use marginfi_type_crate::bank_authority_seed;
use marginfi_type_crate::pdas::{derive_bank_vault, derive_bank_vault_authority};
use marginfi_type_crate::types::{
    Bank, BankConfigOpt, BankVaultType, OraclePriceType, OracleSetup,
};
use solana_commitment_config::CommitmentLevel;
use solana_program_test::BanksClientError;
use solana_program_test::ProgramTestContext;
use solana_sdk::{signer::Signer, transaction::Transaction};
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
        derive_bank_vault(&self.key, vault_type, &marginfi::ID)
    }

    pub fn get_vault_authority(&self, vault_type: BankVaultType) -> (Pubkey, u8) {
        derive_bank_vault_authority(&self.key, vault_type, &marginfi::ID)
    }

    pub async fn get_price(&self) -> f64 {
        let bank = self.load().await;
        let oracle_adapter = match bank.config.oracle_setup {
            OracleSetup::Fixed => {
                OraclePriceFeedAdapter::try_from_bank(&bank, &[], &Clock::default(), false).unwrap()
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
                OraclePriceFeedAdapter::try_from_bank(&bank, &[ai], &Clock::default(), false)
                    .unwrap()
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

        let group = self.load().await.group;
        let admin = self.ctx.borrow().payer.pubkey();
        let (fast, gov) = config.split();

        if !fast.is_empty() {
            let accounts = marginfi::accounts::LendingPoolConfigureBank {
                group,
                admin,
                bank: self.key,
                instruction_sysvar: solana_sdk::sysvar::instructions::ID,
            }
            .to_account_metas(Some(true));

            instructions.push(Instruction {
                program_id: marginfi::ID,
                accounts,
                data: marginfi::instruction::LendingPoolConfigureBank {
                    bank_config_opt: fast,
                }
                .data(),
            });
        }

        if !gov.is_empty() {
            let accounts = marginfi::accounts::LendingPoolConfigureBankGov {
                group,
                governance_admin: admin,
                bank: self.key,
                instruction_sysvar: solana_sdk::sysvar::instructions::ID,
            }
            .to_account_metas(Some(true));

            instructions.push(Instruction {
                program_id: marginfi::ID,
                accounts,
                data: marginfi::instruction::LendingPoolConfigureBankGov {
                    bank_config_opt: gov,
                }
                .data(),
            });
        }

        if let Some((setup, oracle)) = oracle_update {
            let mut oracle_accounts = marginfi::accounts::LendingPoolConfigureBankOracle {
                group: self.load().await.group,
                governance_admin: self.ctx.borrow().payer.pubkey(),
                bank: self.key,
                instruction_sysvar: solana_sdk::sysvar::instructions::ID,
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
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(())
    }

    pub async fn try_emissions_deposit(
        &self,
        amount: u64,
        funding_account: Pubkey,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;
        self.try_emissions_deposit_with_mint(amount, funding_account, bank.mint)
            .await
    }

    pub async fn try_emissions_deposit_with_mint(
        &self,
        amount: u64,
        funding_account: Pubkey,
        mint: Pubkey,
    ) -> Result<(), BanksClientError> {
        let bank = self.load().await;

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolEmissionsDeposit {
                group: bank.group,
                bank: self.key,
                mint,
                emissions_funding_account: funding_account,
                depositor: self.ctx.borrow().payer.pubkey(),
                liquidity_vault: bank.liquidity_vault,
                token_program: self.get_token_program(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingPoolEmissionsDeposit { amount }.data(),
        };

        let tx = {
            let ctx = self.ctx.borrow_mut();

            Transaction::new_signed_with_payer(
                &[ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer],
                ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            instruction_sysvar: solana_sdk::sysvar::instructions::ID,
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            instruction_sysvar: solana_sdk::sysvar::instructions::ID,
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            instruction_sysvar: solana_sdk::sysvar::instructions::ID,
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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

    /// Directly mutate the bank's emissions fields in test state.
    pub async fn set_emissions(
        &self,
        emissions_mint: Pubkey,
        emissions_rate: u64,
        emissions_remaining: I80F48,
        flags: u64,
    ) {
        let mut bank_ai = self
            .ctx
            .borrow_mut()
            .banks_client
            .get_account(self.key)
            .await
            .unwrap()
            .unwrap();
        let bank = bytemuck::from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);

        bank.emissions_mint = emissions_mint;
        bank.emissions_rate = emissions_rate;
        bank.emissions_remaining = emissions_remaining.into();
        bank.flags |= flags;

        self.ctx
            .borrow_mut()
            .set_account(&self.key, &bank_ai.into());
    }

    /// Build (but do not send) a `lending_pool_clear_circuit_breaker` ix.
    /// `authority` must be either `group.admin` or `group.risk_admin`.
    pub async fn make_clear_circuit_breaker_ix(
        &self,
        authority: Pubkey,
        reseed_reference: bool,
    ) -> Instruction {
        let bank = self.load().await;
        let accounts = marginfi::accounts::LendingPoolClearCircuitBreaker {
            group: bank.group,
            authority,
            bank: self.key,
            instruction_sysvar: solana_sdk::sysvar::instructions::ID,
        }
        .to_account_metas(Some(true));
        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: marginfi::instruction::LendingPoolClearCircuitBreaker { reseed_reference }.data(),
        }
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

    pub async fn set_liability_share_value(&self, value: I80F48) {
        let mut bank_ai = self
            .ctx
            .borrow_mut()
            .banks_client
            .get_account(self.key)
            .await
            .unwrap()
            .unwrap();
        let bank = bytemuck::from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);

        bank.liability_share_value = value.into();

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

pub async fn arm_and_trip_circuit_breaker(
    test_f: &crate::test::TestFixture,
    bank: &BankFixture,
    feed: Pubkey,
    base_native: i64,
) -> anyhow::Result<()> {
    arm_and_trip_circuit_breakers(test_f, &[(bank, feed, base_native)]).await
}

pub async fn arm_and_trip_circuit_breakers(
    test_f: &crate::test::TestFixture,
    banks: &[(&BankFixture, Pubkey, i64)],
) -> anyhow::Result<()> {
    warm_price_cache_and_enable_cb(test_f, banks).await?;

    let trip_time = CB_WARM_UNIX_TIME + 1;
    test_f.set_clock(CB_WARM_SLOT + 10, trip_time).await;
    for (_, feed, base) in banks {
        test_f
            .set_pyth_oracle_price_native(*feed, base.saturating_mul(2), 0, trip_time)
            .await;
    }
    for (bank, _, _) in banks {
        test_f
            .marginfi_group
            .try_pulse_bank_price_cache(bank)
            .await?;
    }

    for (_, feed, base) in banks {
        test_f
            .set_pyth_oracle_price_native(*feed, *base, 0, trip_time)
            .await;
    }
    refresh_cb_scenario_feeds(test_f, trip_time).await;
    Ok(())
}

pub async fn arm_circuit_breakers(
    test_f: &crate::test::TestFixture,
    banks: &[(&BankFixture, Pubkey, i64)],
) -> anyhow::Result<()> {
    warm_price_cache_and_enable_cb(test_f, banks).await?;
    refresh_cb_scenario_feeds(test_f, CB_WARM_UNIX_TIME).await;
    Ok(())
}

const CB_WARM_UNIX_TIME: i64 = 100;
const CB_WARM_SLOT: u64 = 1_000;

async fn warm_price_cache_and_enable_cb(
    test_f: &crate::test::TestFixture,
    banks: &[(&BankFixture, Pubkey, i64)],
) -> anyhow::Result<()> {
    for (_, feed, base) in banks {
        test_f
            .set_pyth_oracle_price_native(*feed, *base, 0, CB_WARM_UNIX_TIME)
            .await;
    }
    test_f.set_clock(CB_WARM_SLOT, CB_WARM_UNIX_TIME).await;
    for (bank, _, _) in banks {
        test_f
            .marginfi_group
            .try_pulse_bank_price_cache(bank)
            .await?;
    }
    for (bank, _, _) in banks {
        bank.update_config(crate::utils::standard_cb_config(), None)
            .await?;
    }
    Ok(())
}

pub async fn arm_circuit_breaker(
    test_f: &crate::test::TestFixture,
    bank: &BankFixture,
    feed: Pubkey,
    base_native: i64,
) -> anyhow::Result<()> {
    arm_circuit_breakers(test_f, &[(bank, feed, base_native)]).await
}

async fn refresh_cb_scenario_feeds(test_f: &crate::test::TestFixture, timestamp: i64) {
    for feed in [
        crate::test::PYTH_SOL_FEED,
        crate::test::PYTH_USDC_FEED,
        crate::test::PYTH_PYUSD_FEED,
        crate::test::PYTH_SOL_EQUIVALENT_FEED,
    ] {
        test_f.set_pyth_oracle_timestamp(feed, timestamp).await;
    }
}

pub async fn set_oracle_price_now(test_f: &crate::test::TestFixture, feed: Pubkey, native: i64) {
    let now = test_f.get_clock().await.unix_timestamp;
    test_f
        .set_pyth_oracle_price_native(feed, native, 0, now)
        .await;
}

pub const CONFIDENCE_BREACHING_HEALTH_CAP_ONLY: u64 = 1_000_000_000;

pub const CONFIDENCE_BREACHING_ALL_READS: u64 = 60_000_000_000;

pub async fn assert_confidence_exceeds_health_cap(bank: &BankFixture) {
    let raw_cap = bank.load().await.config.oracle_max_confidence;
    let effective_cap = if raw_cap == 0 { u32::MAX / 10 } else { raw_cap };
    assert!(
        u64::from(effective_cap) * 1_000 < u64::from(u32::MAX) * 212,
        "the test oracle confidence must breach the configured health-read cap"
    );
}

impl BankFixture {
    pub async fn set_asset_weights(
        &self,
        init: impl Into<marginfi_type_crate::types::WrappedI80F48>,
        maint: impl Into<marginfi_type_crate::types::WrappedI80F48>,
    ) -> anyhow::Result<()> {
        self.update_config(
            marginfi_type_crate::types::BankConfigOpt {
                asset_weight_init: Some(init.into()),
                asset_weight_maint: Some(maint.into()),
                ..Default::default()
            },
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn set_asset_weights_to_zero(&self) -> anyhow::Result<()> {
        self.set_asset_weights(fixed::types::I80F48::ZERO, fixed::types::I80F48::ZERO)
            .await
    }
}
