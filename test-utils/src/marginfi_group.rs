use super::{bank::BankFixture, marginfi_account::MarginfiAccountFixture};
use crate::kamino::KaminoFixture;
use crate::prelude::{get_oracle_id_from_feed_id, MintFixture};
use crate::ui_to_native;
use crate::utils::*;
use anchor_lang::{prelude::*, solana_program::system_program};

use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use anyhow::Result;
use bytemuck::bytes_of;
use fixed::types::I80F48;
use marginfi::constants::{
    INIT_BANK_ORIGINATION_FEE_DEFAULT, LIQUIDATION_BONUS_FEE_MINIMUM, LIQUIDATION_FLAT_FEE_DEFAULT,
    ORDER_EXECUTION_MAX_FEE, ORDER_INIT_FLAT_FEE_DEFAULT,
};
use marginfi_type_crate::constants::{
    FEE_STATE_SEED, PROTOCOL_FEE_FIXED_DEFAULT, PROTOCOL_FEE_RATE_DEFAULT,
    SAME_ASSET_EMODE_REGISTRY_SEED, STAKED_SETTINGS_SEED,
};
use marginfi_type_crate::ix_builders;
use marginfi_type_crate::types::WrappedI80F48;
use marginfi_type_crate::types::{
    BankConfig, BankConfigCompact, BankConfigOpt, BankVaultType, EmodeEntry, FeeState,
    InterestRateConfigOpt, MarginfiGroup, OracleSetup, StakedSettings, StakedSettingsConfig,
    MAX_EMODE_ENTRIES,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction, signature::Keypair, signer::Signer, transaction::Transaction,
};
use solana_system_transaction as system_transaction;
use std::{cell::RefCell, mem, rc::Rc};

async fn airdrop_sol(context: &mut ProgramTestContext, key: &Pubkey, amount: u64) {
    let recent_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let tx = system_transaction::transfer(&context.payer, key, amount, recent_blockhash);
    context.banks_client.process_transaction(tx).await.unwrap();
}

pub struct MarginfiGroupFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
    pub same_asset_emode_registry: Pubkey,
    pub staked_settings: Pubkey,
    pub fee_state: Pubkey,
    pub fee_wallet: Pubkey,
}

impl MarginfiGroupFixture {
    pub async fn new(ctx: Rc<RefCell<ProgramTestContext>>) -> MarginfiGroupFixture {
        let ctx_ref = ctx.clone();

        let group_key = Keypair::new();
        let (same_asset_emode_registry, _registry_bump) = Pubkey::find_program_address(
            &[
                SAME_ASSET_EMODE_REGISTRY_SEED.as_bytes(),
                group_key.pubkey().as_ref(),
            ],
            &marginfi::ID,
        );
        let fee_wallet_key: Pubkey;
        let (fee_state_key, _bump) =
            Pubkey::find_program_address(&[FEE_STATE_SEED.as_bytes()], &marginfi::ID);
        let (staked_settings_key, _bump) = Pubkey::find_program_address(
            &[STAKED_SETTINGS_SEED.as_bytes(), group_key.pubkey().as_ref()],
            &marginfi::ID,
        );

        {
            let mut ctx = ctx.borrow_mut();
            let admin = ctx.payer.pubkey();

            let initialize_marginfi_group_ix = ix_builders::pool::marginfi_group_initialize(
                &ix_builders::pool::MarginfiGroupInitialize {
                    marginfi_group: group_key.pubkey(),
                    admin,
                    fee_state: fee_state_key,
                    system_program: system_program::id(),
                },
            );

            // Payer is all admins in most test cases for simplicity, generally this is not
            // true in production - the MS is the main admin and others are lower-impact
            // wallets with a smaller threshold.
            let configure_marginfi_group_ix = ix_builders::pool::marginfi_group_configure(
                &ix_builders::pool::MarginfiGroupConfigure {
                    marginfi_group: group_key.pubkey(),
                    admin,
                },
                Some(admin),
                Some(admin),
                Some(admin),
                Some(admin),
                Some(admin),
                Some(admin),
                Some(admin),
                Some(admin),
                None,
                None,
                None,
                None,
            );

            // Check if the fee state account already exists
            let fee_state_account = ctx.banks_client.get_account(fee_state_key).await.unwrap();

            // Account exists, read it and proceed with group initialization
            if let Some(account) = fee_state_account {
                if !account.data.is_empty() {
                    // Deserialize the account data to extract the fee_wallet public key
                    let fee_state_data: FeeState =
                        FeeState::try_deserialize(&mut &account.data[..]).unwrap();
                    fee_wallet_key = fee_state_data.global_fee_wallet;

                    let tx = Transaction::new_signed_with_payer(
                        &[initialize_marginfi_group_ix, configure_marginfi_group_ix],
                        Some(&ctx.payer.pubkey().clone()),
                        &[&ctx.payer, &group_key],
                        ctx.banks_client.get_latest_blockhash().await.unwrap(),
                    );
                    ctx.banks_client.process_transaction(tx).await.unwrap();
                } else {
                    panic!("Fee state exists but is empty")
                }
            } else {
                // Account does not exist, proceed with group and fee state initialization
                let fee_wallet = Keypair::new();
                // The wallet needs some sol to be rent exempt
                airdrop_sol(&mut ctx, &fee_wallet.pubkey(), 1_000_000).await;
                fee_wallet_key = fee_wallet.pubkey();

                let init_fee_state_ix = ix_builders::admin::init_global_fee_state(
                    &ix_builders::admin::InitGlobalFeeState {
                        payer: ctx.payer.pubkey(),
                        fee_state: fee_state_key,
                        system_program: system_program::id(),
                    },
                    ctx.payer.pubkey(),
                    fee_wallet.pubkey(),
                    INIT_BANK_ORIGINATION_FEE_DEFAULT,
                    LIQUIDATION_FLAT_FEE_DEFAULT,
                    ORDER_INIT_FLAT_FEE_DEFAULT,
                    PROTOCOL_FEE_FIXED_DEFAULT.into(),
                    PROTOCOL_FEE_RATE_DEFAULT.into(),
                    LIQUIDATION_BONUS_FEE_MINIMUM.into(),
                    ORDER_EXECUTION_MAX_FEE.into(),
                );

                let tx = Transaction::new_signed_with_payer(
                    &[
                        init_fee_state_ix,
                        initialize_marginfi_group_ix,
                        configure_marginfi_group_ix,
                    ],
                    Some(&ctx.payer.pubkey().clone()),
                    &[&ctx.payer, &group_key],
                    ctx.banks_client.get_latest_blockhash().await.unwrap(),
                );
                ctx.banks_client.process_transaction(tx).await.unwrap();
            }
        }

        {
            let ctx = ctx.borrow_mut();
            let settings = StakedSettingsConfig {
                oracle: Pubkey::default(),
                asset_weight_init: I80F48::from_num(0.8).into(),
                asset_weight_maint: I80F48::from_num(0.9).into(),
                deposit_limit: 1_000_000,
                total_asset_value_init_limit: 1_000_000,
                oracle_max_age: 10,
                risk_tier: marginfi_type_crate::types::RiskTier::Collateral,
            };
            let ix = ix_builders::admin::init_staked_settings(
                &ix_builders::admin::InitStakedSettings {
                    marginfi_group: group_key.pubkey(),
                    admin: ctx.payer.pubkey(),
                    fee_payer: ctx.payer.pubkey(),
                    staked_settings: staked_settings_key,
                    system_program: system_program::id(),
                },
                settings,
            );

            let tx = Transaction::new_signed_with_payer(
                &[ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer],
                ctx.banks_client.get_latest_blockhash().await.unwrap(),
            );
            ctx.banks_client.process_transaction(tx).await.unwrap();
        }

        MarginfiGroupFixture {
            ctx: ctx_ref.clone(),
            key: group_key.pubkey(),
            same_asset_emode_registry,
            staked_settings: staked_settings_key,
            fee_state: fee_state_key,
            fee_wallet: fee_wallet_key,
        }
    }

    /// Adds bank and configures the oracle.
    ///
    /// Note: AddBank and LendingPoolConfigureBankOracle were separated to handle a tx size issue in
    /// squads. This test fixture packs both ixes into one tx as is typical outside of squads.
    pub async fn try_lending_pool_add_bank(
        &self,
        bank_asset_mint_fixture: &MintFixture,
        kamino_fixture: Option<KaminoFixture>,
        bank_config: BankConfig,
        fixed_price: Option<I80F48>,
    ) -> Result<BankFixture, BanksClientError> {
        let bank_key = Keypair::new();
        let bank_mint = bank_asset_mint_fixture.key;
        let bank_fixture = BankFixture::new(
            self.ctx.clone(),
            bank_key.pubkey(),
            bank_asset_mint_fixture,
            kamino_fixture,
        );
        let config_compact: BankConfigCompact = bank_config.into();

        let init_ix = ix_builders::pool::lending_pool_add_bank(
            &ix_builders::pool::LendingPoolAddBank {
                marginfi_group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
                fee_payer: self.ctx.borrow().payer.pubkey(),
                fee_state: self.fee_state,
                global_fee_wallet: self.fee_wallet,
                bank_mint,
                bank: bank_key.pubkey(),
                liquidity_vault_authority: bank_fixture
                    .get_vault_authority(BankVaultType::Liquidity)
                    .0,
                liquidity_vault: bank_fixture.get_vault(BankVaultType::Liquidity).0,
                insurance_vault_authority: bank_fixture
                    .get_vault_authority(BankVaultType::Insurance)
                    .0,
                insurance_vault: bank_fixture.get_vault(BankVaultType::Insurance).0,
                fee_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Fee).0,
                fee_vault: bank_fixture.get_vault(BankVaultType::Fee).0,
                token_program: bank_asset_mint_fixture.token_program,
                system_program: system_program::id(),
            },
            config_compact,
        );

        let feed_oracle = {
            if bank_config.oracle_setup == OracleSetup::PythPushOracle
                || bank_config.oracle_setup == OracleSetup::StakedWithPythPush
            {
                Some(
                    get_oracle_id_from_feed_id(bank_config.oracle_keys[0])
                        .unwrap_or(bank_config.oracle_keys[0]),
                )
            } else {
                None
            }
        };

        let config_oracle_ix = if bank_config.oracle_setup == OracleSetup::Fixed {
            let price: I80F48 = fixed_price.unwrap();
            println!("mint: {:?} price {:?}", bank_mint, price);

            self.make_lending_pool_set_fixed_oracle_price_ix(
                &bank_fixture,
                fixed_price.unwrap().into(),
            )
        } else {
            self.make_lending_pool_configure_bank_oracle_ix(
                &bank_fixture,
                bank_config.oracle_setup as u8,
                bank_config.oracle_keys[0],
                feed_oracle,
            )
        };

        let tx = Transaction::new_signed_with_payer(
            &[init_ix, config_oracle_ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
            &[&self.ctx.borrow().payer, &bank_key],
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(bank_fixture)
    }

    /// Adds bank and configures the oracle.
    ///
    /// Note: AddBank and LendingPoolConfigureBankOracle were separated to handle a tx size issue in
    /// squads. This test fixture packs both ixes into one tx as is typical outside of squads.
    pub async fn try_lending_pool_add_bank_with_seed(
        &self,
        bank_asset_mint_fixture: &MintFixture,
        kamino_fixture: Option<KaminoFixture>,
        bank_config: BankConfig,
        bank_seed: u64,
    ) -> Result<BankFixture, BanksClientError> {
        let bank_mint = bank_asset_mint_fixture.key;

        // Create PDA account from seeds
        let (pda, _bump) = Pubkey::find_program_address(
            [
                self.key.as_ref(),
                bank_mint.as_ref(),
                &bank_seed.to_le_bytes(),
            ]
            .as_slice(),
            &marginfi::ID,
        );

        let bank_mint = bank_asset_mint_fixture.key;
        let bank_fixture = BankFixture::new(
            self.ctx.clone(),
            pda,
            bank_asset_mint_fixture,
            kamino_fixture,
        );
        let config_compact: BankConfigCompact = bank_config.into();

        let init_ix = ix_builders::pool::lending_pool_add_bank_with_seed(
            &ix_builders::pool::LendingPoolAddBankWithSeed {
                marginfi_group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
                fee_payer: self.ctx.borrow().payer.pubkey(),
                fee_state: self.fee_state,
                global_fee_wallet: self.fee_wallet,
                bank_mint,
                bank: pda,
                liquidity_vault_authority: bank_fixture
                    .get_vault_authority(BankVaultType::Liquidity)
                    .0,
                liquidity_vault: bank_fixture.get_vault(BankVaultType::Liquidity).0,
                insurance_vault_authority: bank_fixture
                    .get_vault_authority(BankVaultType::Insurance)
                    .0,
                insurance_vault: bank_fixture.get_vault(BankVaultType::Insurance).0,
                fee_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Fee).0,
                fee_vault: bank_fixture.get_vault(BankVaultType::Fee).0,
                token_program: bank_fixture.get_token_program(),
                system_program: system_program::id(),
            },
            config_compact,
            bank_seed,
        );

        let feed_oracle = {
            if bank_config.oracle_setup == OracleSetup::PythPushOracle
                || bank_config.oracle_setup == OracleSetup::StakedWithPythPush
            {
                get_oracle_id_from_feed_id(bank_config.oracle_keys[0])
                    .or(Some(bank_config.oracle_keys[0]))
            } else {
                None
            }
        };

        let config_oracle_ix = self.make_lending_pool_configure_bank_oracle_ix(
            &bank_fixture,
            bank_config.oracle_setup as u8,
            bank_config.oracle_keys[0],
            feed_oracle,
        );

        let tx = Transaction::new_signed_with_payer(
            &[init_ix, config_oracle_ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
            &[&self.ctx.borrow().payer],
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(bank_fixture)
    }

    pub fn make_lending_pool_configure_bank_ix(
        &self,
        bank: &BankFixture,
        bank_config_opt: BankConfigOpt,
    ) -> Instruction {
        ix_builders::pool::lending_pool_configure_bank(
            &ix_builders::pool::LendingPoolConfigureBank {
                bank: bank.key,
                group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            },
            bank_config_opt,
        )
    }

    pub fn make_lending_pool_configure_bank_oracle_ix(
        &self,
        bank: &BankFixture,
        setup: u8,
        oracle: Pubkey,
        feed_oracle: Option<Pubkey>,
    ) -> Instruction {
        let mut ix = ix_builders::pool::lending_pool_configure_bank_oracle(
            &ix_builders::pool::LendingPoolConfigureBankOracle {
                bank: bank.key,
                group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            },
            setup,
            oracle,
        );

        ix.accounts.push(AccountMeta::new_readonly(
            feed_oracle.unwrap_or(oracle),
            false,
        ));

        ix
    }

    pub fn make_lending_pool_set_fixed_oracle_price_ix(
        &self,
        bank: &BankFixture,
        price: WrappedI80F48,
    ) -> Instruction {
        ix_builders::pool::lending_pool_set_fixed_oracle_price(
            &ix_builders::pool::LendingPoolSetFixedOraclePrice {
                group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
                bank: bank.key,
            },
            price,
        )
    }

    pub fn make_lending_pool_set_bank_same_asset_emode_eligibility_ix(
        &self,
        bank: &BankFixture,
        enabled: bool,
    ) -> Instruction {
        ix_builders::pool::lending_pool_set_bank_same_asset_emode_eligibility(
            &ix_builders::pool::LendingPoolSetBankSameAssetEmodeEligibility {
                group: self.key,
                signer: self.ctx.borrow().payer.pubkey(),
                bank: bank.key,
                same_asset_emode_registry: self.same_asset_emode_registry,
            },
            enabled,
        )
    }

    pub fn make_lending_pool_init_same_asset_emode_registry_ix(&self) -> Instruction {
        ix_builders::pool::lending_pool_init_same_asset_emode_registry(
            &ix_builders::pool::LendingPoolInitSameAssetEmodeRegistry {
                group: self.key,
                signer: self.ctx.borrow().payer.pubkey(),
                same_asset_emode_registry: self.same_asset_emode_registry,
                system_program: system_program::id(),
            },
        )
    }

    pub async fn try_lending_pool_init_same_asset_emode_registry(
        &self,
    ) -> Result<(), BanksClientError> {
        let ix = self.make_lending_pool_init_same_asset_emode_registry_ix();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_lending_pool_configure_bank(
        &self,
        bank: &BankFixture,
        bank_config_opt: BankConfigOpt,
    ) -> Result<(), BanksClientError> {
        let ix = self.make_lending_pool_configure_bank_ix(bank, bank_config_opt);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_lending_pool_set_bank_same_asset_emode_eligibility(
        &self,
        bank: &BankFixture,
        enabled: bool,
    ) -> Result<(), BanksClientError> {
        let registry_account = self
            .ctx
            .borrow_mut()
            .banks_client
            .get_account(self.same_asset_emode_registry)
            .await?;
        if registry_account.is_none() {
            self.try_lending_pool_init_same_asset_emode_registry()
                .await?;
        }

        let ix = self.make_lending_pool_set_bank_same_asset_emode_eligibility_ix(bank, enabled);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub fn make_lending_pool_configure_bank_interest_only_ix(
        &self,
        bank: &BankFixture,
        interest_rate_config: InterestRateConfigOpt,
    ) -> Instruction {
        ix_builders::pool::lending_pool_configure_bank_interest_only(
            &ix_builders::pool::LendingPoolConfigureBankInterestOnly {
                group: self.key,
                delegate_curve_admin: self.ctx.borrow().payer.pubkey(),
                bank: bank.key,
            },
            interest_rate_config,
        )
    }

    pub async fn try_lending_pool_configure_bank_interest_only(
        &self,
        bank: &BankFixture,
        interest_rate_config: InterestRateConfigOpt,
    ) -> Result<(), BanksClientError> {
        let ix = self.make_lending_pool_configure_bank_interest_only_ix(bank, interest_rate_config);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
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

    pub fn make_lending_pool_configure_bank_limits_only_ix(
        &self,
        bank: &BankFixture,
        deposit_limit: Option<u64>,
        borrow_limit: Option<u64>,
        total_asset_value_init_limit: Option<u64>,
    ) -> Instruction {
        ix_builders::pool::lending_pool_configure_bank_limits_only(
            &ix_builders::pool::LendingPoolConfigureBankLimitsOnly {
                group: self.key,
                delegate_limit_admin: self.ctx.borrow().payer.pubkey(),
                bank: bank.key,
            },
            deposit_limit,
            borrow_limit,
            total_asset_value_init_limit,
        )
    }

    pub async fn try_lending_pool_configure_bank_limits_only(
        &self,
        bank: &BankFixture,
        deposit_limit: Option<u64>,
        borrow_limit: Option<u64>,
        total_asset_value_init_limit: Option<u64>,
    ) -> Result<(), BanksClientError> {
        let ix = self.make_lending_pool_configure_bank_limits_only_ix(
            bank,
            deposit_limit,
            borrow_limit,
            total_asset_value_init_limit,
        );
        let tx = Transaction::new_signed_with_payer(
            &[ix],
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

    #[allow(clippy::result_large_err)]
    pub fn pad_emode_entries(
        entries: &[EmodeEntry],
    ) -> Result<[EmodeEntry; MAX_EMODE_ENTRIES], BanksClientError> {
        if entries.len() > MAX_EMODE_ENTRIES {
            return Err(BanksClientError::ClientError(
                "wrong number of entries (max: 10)",
            ));
        }

        let mut result = [EmodeEntry {
            collateral_bank_emode_tag: 0,
            flags: 0,
            pad0: [0; 5],
            asset_weight_init: I80F48::ZERO.into(),
            asset_weight_maint: I80F48::ZERO.into(),
        }; MAX_EMODE_ENTRIES];

        result[..entries.len()].copy_from_slice(entries);

        Ok(result)
    }

    pub fn make_lending_pool_configure_bank_emode_ix(
        &self,
        bank: &BankFixture,
        emode_tag: u16,
        entries: [EmodeEntry; MAX_EMODE_ENTRIES],
    ) -> Instruction {
        ix_builders::pool::lending_pool_configure_bank_emode(
            &ix_builders::pool::LendingPoolConfigureBankEmode {
                bank: bank.key,
                group: self.key,
                emode_admin: self.ctx.borrow().payer.pubkey(),
            },
            emode_tag,
            entries,
        )
    }

    pub async fn try_lending_pool_configure_bank_emode(
        &self,
        bank: &BankFixture,
        emode_tag: u16,
        entries: &[EmodeEntry],
    ) -> Result<(), BanksClientError> {
        let padded_entries = Self::pad_emode_entries(entries)?;
        let ix = self.make_lending_pool_configure_bank_emode_ix(bank, emode_tag, padded_entries);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub fn make_lending_pool_clone_emode_ix(
        &self,
        signer: Pubkey,
        copy_from_bank: Pubkey,
        copy_to_bank: Pubkey,
    ) -> Instruction {
        ix_builders::pool::lending_pool_clone_emode(&ix_builders::pool::LendingPoolCloneEmode {
            group: self.key,
            signer,
            copy_from_bank,
            copy_to_bank,
        })
    }

    pub async fn try_lending_pool_clone_emode_with_signer(
        &self,
        signer: &Keypair,
        copy_from_bank: &BankFixture,
        copy_to_bank: &BankFixture,
    ) -> Result<(), BanksClientError> {
        let ctx = self.ctx.borrow_mut();

        let ix = self.make_lending_pool_clone_emode_ix(
            signer.pubkey(),
            copy_from_bank.key,
            copy_to_bank.key,
        );

        let mut signers: Vec<&dyn Signer> = vec![&ctx.payer];
        if signer.pubkey() != ctx.payer.pubkey() {
            signers.push(signer);
        }

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &signers,
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn try_lending_pool_clone_emode(
        &self,
        copy_from_bank: &BankFixture,
        copy_to_bank: &BankFixture,
    ) -> Result<(), BanksClientError> {
        let ctx = self.ctx.borrow_mut();

        let ix = self.make_lending_pool_clone_emode_ix(
            ctx.payer.pubkey(),
            copy_from_bank.key,
            copy_to_bank.key,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn try_accrue_interest(&self, bank: &BankFixture) -> Result<(), BanksClientError> {
        let ctx = self.ctx.borrow_mut();

        let ix = ix_builders::pool::lending_pool_accrue_bank_interest(
            &ix_builders::pool::LendingPoolAccrueBankInterest {
                group: self.key,
                bank: bank.key,
            },
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        ctx.banks_client.process_transaction(tx).await
    }

    pub async fn try_pulse_bank_price_cache(
        &self,
        bank: &BankFixture,
    ) -> Result<(), BanksClientError> {
        let bank_state = bank.load().await;

        let mut ix = ix_builders::pool::lending_pool_pulse_bank_price_cache(
            &ix_builders::pool::LendingPoolPulseBankPriceCache {
                group: self.key,
                bank: bank.key,
            },
        );

        // For non-fixed oracle setups, add the primary oracle account as remaining
        if bank_state.config.oracle_setup != OracleSetup::Fixed {
            let oracle_key = bank_state.config.oracle_keys[0];
            ix.accounts
                .push(AccountMeta::new_readonly(oracle_key, false));
        }

        let mut ctx = self.ctx.borrow_mut();

        // Consecutive pulses build byte-identical messages; a forced-fresh blockhash keeps
        // their signatures distinct so the banks server doesn't dedup them as replays.
        let blockhash = ctx.get_new_latest_blockhash().await.unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            blockhash,
        );

        ctx.banks_client.process_transaction(tx).await
    }

    pub async fn try_update(
        &self,
        new_admin: Pubkey,
        new_emode_admin: Pubkey,
        new_curve_admin: Pubkey,
        new_limit_admin: Pubkey,
        new_emissions_admin: Pubkey,
        new_metadata_admin: Pubkey,
        new_risk_admin: Pubkey,
    ) -> Result<(), BanksClientError> {
        let group = self.load().await;
        self.try_update_with_emode_leverage_and_flow_admin(
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            group.delegate_flow_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
            None,
            None,
        )
        .await
    }

    pub async fn try_update_with_emode_leverage(
        &self,
        new_admin: Pubkey,
        new_emode_admin: Pubkey,
        new_curve_admin: Pubkey,
        new_limit_admin: Pubkey,
        new_emissions_admin: Pubkey,
        new_metadata_admin: Pubkey,
        new_risk_admin: Pubkey,
        emode_max_init_leverage: Option<WrappedI80F48>,
        emode_max_maint_leverage: Option<WrappedI80F48>,
    ) -> Result<(), BanksClientError> {
        let group = self.load().await;
        self.try_update_with_emode_leverage_and_flow_admin(
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            group.delegate_flow_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        )
        .await
    }

    pub async fn try_update_with_same_asset_emode_leverage(
        &self,
        new_admin: Pubkey,
        new_emode_admin: Pubkey,
        new_curve_admin: Pubkey,
        new_limit_admin: Pubkey,
        new_emissions_admin: Pubkey,
        new_metadata_admin: Pubkey,
        new_risk_admin: Pubkey,
        same_asset_emode_init_leverage: Option<WrappedI80F48>,
        same_asset_emode_maint_leverage: Option<WrappedI80F48>,
    ) -> Result<(), BanksClientError> {
        let group = self.load().await;
        let ix = ix_builders::pool::marginfi_group_configure(
            &ix_builders::pool::MarginfiGroupConfigure {
                marginfi_group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            },
            Some(new_admin),
            Some(new_emode_admin),
            Some(new_curve_admin),
            Some(new_limit_admin),
            Some(group.delegate_flow_admin),
            Some(new_emissions_admin),
            Some(new_metadata_admin),
            Some(new_risk_admin),
            None,
            None,
            same_asset_emode_init_leverage,
            same_asset_emode_maint_leverage,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_update_with_flow_admin(
        &self,
        new_admin: Pubkey,
        new_emode_admin: Pubkey,
        new_curve_admin: Pubkey,
        new_limit_admin: Pubkey,
        new_flow_admin: Pubkey,
        new_emissions_admin: Pubkey,
        new_metadata_admin: Pubkey,
        new_risk_admin: Pubkey,
    ) -> Result<(), BanksClientError> {
        self.try_update_with_emode_leverage_and_flow_admin(
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            new_flow_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
            None,
            None,
        )
        .await
    }

    async fn try_update_with_emode_leverage_and_flow_admin(
        &self,
        new_admin: Pubkey,
        new_emode_admin: Pubkey,
        new_curve_admin: Pubkey,
        new_limit_admin: Pubkey,
        new_flow_admin: Pubkey,
        new_emissions_admin: Pubkey,
        new_metadata_admin: Pubkey,
        new_risk_admin: Pubkey,
        emode_max_init_leverage: Option<WrappedI80F48>,
        emode_max_maint_leverage: Option<WrappedI80F48>,
    ) -> Result<(), BanksClientError> {
        let ix = ix_builders::pool::marginfi_group_configure(
            &ix_builders::pool::MarginfiGroupConfigure {
                marginfi_group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            },
            Some(new_admin),
            Some(new_emode_admin),
            Some(new_curve_admin),
            Some(new_limit_admin),
            Some(new_flow_admin),
            Some(new_emissions_admin),
            Some(new_metadata_admin),
            Some(new_risk_admin),
            emode_max_init_leverage,
            emode_max_maint_leverage,
            None,
            None,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_update_deleverage_withdrawal_limit(
        &self,
        limit: u32,
    ) -> Result<(), BanksClientError> {
        let ix = ix_builders::admin::configure_deleverage_withdrawal_limit(
            &ix_builders::admin::ConfigureDeleverageWithdrawalLimit {
                marginfi_group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            },
            limit,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_admin_update_deleverage_withdrawals(
        &self,
        outflow_usd: u32,
        update_seq: u64,
        event_start_slot: u64,
        event_end_slot: u64,
    ) -> Result<(), BanksClientError> {
        let ix = ix_builders::admin::update_deleverage_withdrawals(
            &ix_builders::admin::UpdateDeleverageWithdrawals {
                marginfi_group: self.key,
                delegate_flow_admin: self.ctx.borrow().payer.pubkey(),
            },
            outflow_usd,
            update_seq,
            event_start_slot,
            event_end_slot,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_collect_fees(&self, bank: &BankFixture) -> Result<()> {
        let ctx = self.ctx.borrow_mut();

        let fee_ata = get_associated_token_address_with_program_id(
            &self.fee_wallet,
            &bank.mint.key,
            &bank.get_token_program(),
        );

        let mut ix = ix_builders::pool::lending_pool_collect_bank_fees(
            &ix_builders::pool::LendingPoolCollectBankFees {
                group: self.key,
                bank: bank.key,
                liquidity_vault_authority: bank.get_vault_authority(BankVaultType::Liquidity).0,
                liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
                insurance_vault: bank.get_vault(BankVaultType::Insurance).0,
                fee_vault: bank.get_vault(BankVaultType::Fee).0,
                token_program: bank.get_token_program(),
                fee_state: self.fee_state,
                fee_ata,
            },
        );
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            ix.accounts
                .push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn try_handle_bankruptcy(
        &self,
        bank: &BankFixture,
        marginfi_account: &MarginfiAccountFixture,
    ) -> Result<(), BanksClientError> {
        self.try_handle_bankruptcy_with_nonce(bank, marginfi_account, 100)
            .await
    }

    pub async fn try_handle_bankruptcy_with_nonce(
        &self,
        bank: &BankFixture,
        marginfi_account: &MarginfiAccountFixture,
        nonce: u64,
    ) -> Result<(), BanksClientError> {
        let mut ix = ix_builders::pool::lending_pool_handle_bankruptcy(
            &ix_builders::pool::LendingPoolHandleBankruptcy {
                group: self.key,
                signer: self.ctx.borrow().payer.pubkey(),
                bank: bank.key,
                marginfi_account: marginfi_account.key,
                liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
                insurance_vault: bank.get_vault(BankVaultType::Insurance).0,
                insurance_vault_authority: bank.get_vault_authority(BankVaultType::Insurance).0,
                token_program: bank.get_token_program(),
            },
        );
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            ix.accounts
                .push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        ix.accounts.append(
            &mut marginfi_account
                .load_observation_account_metas(vec![], vec![])
                .await,
        );

        let ctx = self.ctx.borrow_mut();

        let nonce_ix = ComputeBudgetInstruction::set_compute_unit_price(nonce);

        let tx = Transaction::new_signed_with_payer(
            &[ix, nonce_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );

        ctx.banks_client.process_transaction(tx).await
    }

    pub async fn try_lending_pool_backfill_bank_is_t22_flag(
        &self,
        bank: &BankFixture,
        bank_seed: Option<u64>,
    ) -> Result<(), BanksClientError> {
        let ix = ix_builders::pool::lending_pool_backfill_bank_is_t22_flag(
            &ix_builders::pool::LendingPoolBackfillBankIsT22Flag {
                bank: bank.key,
                group: self.key,
                mint: bank.mint.key,
            },
            bank_seed,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }

    pub fn make_super_admin_withdraw_ix_native(
        &self,
        bank: &BankFixture,
        destination_token_account: Pubkey,
        amount: u64,
    ) -> Instruction {
        let mut ix = ix_builders::admin::super_admin_withdraw(
            &ix_builders::admin::SuperAdminWithdraw {
                group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
                bank: bank.key,
                destination_token_account,
                liquidity_vault_authority: bank.get_vault_authority(BankVaultType::Liquidity).0,
                liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
                token_program: bank.get_token_program(),
            },
            amount,
        );
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            ix.accounts
                .push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        ix
    }

    /// Withdraws from bank vault to the hardcoded DESTINATION_WALLET's ATA.
    /// Creates the ATA if it doesn't exist. Returns the ATA pubkey.
    pub async fn try_super_admin_withdraw_native(
        &self,
        bank: &BankFixture,
        amount: u64,
    ) -> std::result::Result<Pubkey, BanksClientError> {
        let destination_wallet =
            Pubkey::try_from("AnGdBvg8VmVHq7zyUYmC7mgjZ5pW6odwFsh6eharbzLu").unwrap();
        let token_program = bank.get_token_program();
        let ata = get_associated_token_address_with_program_id(
            &destination_wallet,
            &bank.mint.key,
            &token_program,
        );

        let create_ata_ix = create_associated_token_account_idempotent(
            &self.ctx.borrow().payer.pubkey(),
            &destination_wallet,
            &bank.mint.key,
            &token_program,
        );
        let withdraw_ix = self.make_super_admin_withdraw_ix_native(bank, ata, amount);

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix, withdraw_ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;

        Ok(ata)
    }

    pub async fn try_super_admin_withdraw<T: Into<f64>>(
        &self,
        bank: &BankFixture,
        ui_amount: T,
    ) -> std::result::Result<Pubkey, BanksClientError> {
        self.try_super_admin_withdraw_native(
            bank,
            ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
        )
        .await
    }

    pub fn make_super_admin_deposit_ix_native(
        &self,
        bank: &BankFixture,
        admin_token_account: Pubkey,
        amount: u64,
    ) -> Instruction {
        let mut ix = ix_builders::admin::super_admin_deposit(
            &ix_builders::admin::SuperAdminDeposit {
                group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
                bank: bank.key,
                admin_token_account,
                liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
                token_program: bank.get_token_program(),
            },
            amount,
        );
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            ix.accounts
                .push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        ix
    }

    pub async fn try_super_admin_deposit_native(
        &self,
        bank: &BankFixture,
        admin_token_account: Pubkey,
        amount: u64,
    ) -> Result<(), BanksClientError> {
        let ix = self.make_super_admin_deposit_ix_native(bank, admin_token_account, amount);

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_super_admin_deposit<T: Into<f64>>(
        &self,
        bank: &BankFixture,
        admin_token_account: Pubkey,
        ui_amount: T,
    ) -> Result<(), BanksClientError> {
        self.try_super_admin_deposit_native(
            bank,
            admin_token_account,
            ui_to_native!(ui_amount.into(), bank.mint.mint.decimals),
        )
        .await
    }

    pub fn get_size() -> usize {
        8 + mem::size_of::<MarginfiGroup>()
    }

    pub async fn load(&self) -> MarginfiGroup {
        load_and_deserialize::<MarginfiGroup>(self.ctx.clone(), &self.key).await
    }

    pub async fn load_staked_settings(&self) -> StakedSettings {
        load_and_deserialize::<StakedSettings>(self.ctx.clone(), &self.staked_settings).await
    }

    /// Shrink the group account to the v1 (8 + struct) size, simulating a mainnet account
    /// created before groups were allocated at the v2 size.
    pub async fn truncate_group_account_to_v1(&self) {
        Self::truncate_account_to(&self.ctx, self.key, 8 + MarginfiGroup::V1_LEN).await
    }

    /// Shrink the fee-state account to the v1 (8 + struct) size, simulating the mainnet
    /// account created before it was allocated at the v2 size.
    pub async fn truncate_fee_state_to_v1(&self) {
        Self::truncate_account_to(&self.ctx, self.fee_state, 8 + FeeState::V1_LEN).await
    }

    async fn truncate_account_to(ctx: &Rc<RefCell<ProgramTestContext>>, key: Pubkey, len: usize) {
        let mut ctx = ctx.borrow_mut();
        let mut account = ctx.banks_client.get_account(key).await.unwrap().unwrap();
        account.data.truncate(len);
        ctx.set_account(&key, &account.into())
    }

    pub async fn try_resize_group_account(&self) -> Result<(), BanksClientError> {
        self.try_resize_account_key(self.key).await
    }

    pub async fn try_resize_fee_state(&self) -> Result<(), BanksClientError> {
        let payer = clone_keypair(&self.ctx.borrow().payer);
        let ix = ix_builders::admin::resize_global_fee_state(
            &ix_builders::admin::ResizeGlobalFeeState {
                fee_state: self.fee_state,
                payer: payer.pubkey(),
                system_program: system_program::ID,
            },
        );
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            latest_blockhash(&self.ctx).await,
        );
        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;
        Ok(())
    }

    /// Send the group resize ix against an arbitrary account key (for negative tests).
    pub async fn try_resize_account_key(&self, key: Pubkey) -> Result<(), BanksClientError> {
        let payer = clone_keypair(&self.ctx.borrow().payer);
        let ix = ix_builders::pool::lending_pool_resize_group_account(
            &ix_builders::pool::LendingPoolResizeGroupAccount {
                group: key,
                payer: payer.pubkey(),
                system_program: system_program::ID,
            },
        );
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            latest_blockhash(&self.ctx).await,
        );
        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await?;
        Ok(())
    }

    pub async fn set_protocol_fees_flag(&self, enabled: bool) {
        let mut group = self.load().await;
        let mut ctx = self.ctx.borrow_mut();
        let mut account = ctx
            .banks_client
            .get_account(self.key)
            .await
            .unwrap()
            .unwrap();

        group.group_flags = if enabled { 1 } else { 0 };

        let data = bytes_of(&group);

        // The account may be larger than the struct (v2 allocation); write the prefix only.
        account.data[8..8 + data.len()].copy_from_slice(data);

        ctx.set_account(&self.key, &account.into())
    }

    pub async fn try_panic_pause(&self) -> Result<(), BanksClientError> {
        let payer = clone_keypair(&self.ctx.borrow().payer);
        self.try_panic_pause_with_authority(&payer).await
    }

    pub async fn try_panic_pause_with_authority(
        &self,
        pause_authority: &Keypair,
    ) -> Result<(), BanksClientError> {
        let ix = ix_builders::admin::panic_pause(&ix_builders::admin::PanicPause {
            pause_authority: pause_authority.pubkey(),
            fee_state: self.fee_state,
        });

        let payer = clone_keypair(&self.ctx.borrow().payer);
        let blockhash = latest_blockhash(&self.ctx).await;
        let tx = if payer.pubkey() == pause_authority.pubkey() {
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash)
        } else {
            Transaction::new_signed_with_payer(
                &[ix],
                Some(&payer.pubkey()),
                &[&payer, pause_authority],
                blockhash,
            )
        };

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }

    pub async fn try_panic_unpause(&self) -> Result<(), BanksClientError> {
        let payer = clone_keypair(&self.ctx.borrow().payer);
        self.try_panic_unpause_with_authority(&payer).await
    }

    pub async fn try_panic_unpause_with_authority(
        &self,
        pause_authority: &Keypair,
    ) -> Result<(), BanksClientError> {
        let ix = ix_builders::admin::panic_unpause(&ix_builders::admin::PanicUnpause {
            global_fee_admin: pause_authority.pubkey(),
            fee_state: self.fee_state,
        });

        let payer = clone_keypair(&self.ctx.borrow().payer);
        let blockhash = latest_blockhash(&self.ctx).await;
        let tx = if payer.pubkey() == pause_authority.pubkey() {
            Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], blockhash)
        } else {
            Transaction::new_signed_with_payer(
                &[ix],
                Some(&payer.pubkey()),
                &[&payer, pause_authority],
                blockhash,
            )
        };

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }

    pub async fn try_set_pause_delegate_admin(
        &self,
        new_pause_delegate_admin: Option<Pubkey>,
    ) -> Result<(), BanksClientError> {
        let ix = ix_builders::admin::edit_global_fee_state(
            &ix_builders::admin::EditGlobalFeeState {
                global_fee_admin: self.ctx.borrow().payer.pubkey(),
                fee_state: self.fee_state,
            },
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(new_pause_delegate_admin.unwrap_or_default()),
            None,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }

    pub async fn try_propagate_fee_state(&self) -> Result<(), BanksClientError> {
        let ix = ix_builders::admin::propagate_fee_state(&ix_builders::admin::PropagateFeeState {
            fee_state: self.fee_state,
            marginfi_group: self.key,
        });

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }

    pub async fn try_disable_staked_oracles(&self) -> Result<(), BanksClientError> {
        let ix =
            ix_builders::admin::disable_staked_oracles(&ix_builders::admin::DisableStakedOracles {
                group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
                staked_settings: self.staked_settings,
            });

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }

    pub async fn try_enable_staked_oracle_onramp(&self) -> Result<(), BanksClientError> {
        let ix = ix_builders::admin::enable_staked_oracle_onramp(
            &ix_builders::admin::EnableStakedOracleOnramp {
                group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
                staked_settings: self.staked_settings,
            },
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            latest_blockhash(&self.ctx).await,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }
}
