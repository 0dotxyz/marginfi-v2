use super::{bank::BankFixture, marginfi_account::MarginfiAccountFixture};
use crate::kamino::KaminoFixture;
use crate::prelude::{get_oracle_id_from_feed_id, MintFixture};
use crate::utils::*;
use anchor_lang::{prelude::*, solana_program::system_program, InstructionData};

use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anyhow::Result;
use bytemuck::bytes_of;
use fixed::types::I80F48;
use marginfi::{
    constants::{
        INIT_BANK_ORIGINATION_FEE_DEFAULT, LIQUIDATION_BONUS_FEE_MINIMUM,
        LIQUIDATION_FLAT_FEE_DEFAULT,
    },
    instruction::*,
    state::bank::BankVaultType,
};
use marginfi_type_crate::constants::{
    FEE_STATE_SEED, PROTOCOL_FEE_FIXED_DEFAULT, PROTOCOL_FEE_RATE_DEFAULT,
};
use marginfi_type_crate::types::WrappedI80F48;
use marginfi_type_crate::types::{
    BankConfig, BankConfigCompact, BankConfigOpt, EmodeEntry, FeeState, InterestRateConfigOpt,
    MarginfiGroup, OracleSetup, MAX_EMODE_ENTRIES,
};
use solana_program_test::*;
use solana_sdk::system_transaction;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, signature::Keypair,
    signer::Signer, transaction::Transaction,
};
use std::{cell::RefCell, mem, rc::Rc};

async fn airdrop_sol(context: &mut ProgramTestContext, key: &Pubkey, amount: u64) {
    let recent_blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let tx = system_transaction::transfer(&context.payer, key, amount, recent_blockhash);
    context.banks_client.process_transaction(tx).await.unwrap();
}

pub struct MarginfiGroupFixture {
    ctx: Rc<RefCell<ProgramTestContext>>,
    pub key: Pubkey,
    pub fee_state: Pubkey,
    pub fee_wallet: Pubkey,
}

impl MarginfiGroupFixture {
    pub async fn new(ctx: Rc<RefCell<ProgramTestContext>>) -> MarginfiGroupFixture {
        let ctx_ref = ctx.clone();

        let group_key = Keypair::new();
        let fee_wallet_key: Pubkey;
        let (fee_state_key, _bump) =
            Pubkey::find_program_address(&[FEE_STATE_SEED.as_bytes()], &marginfi::ID);

        {
            let mut ctx = ctx.borrow_mut();
            let admin = ctx.payer.pubkey();

            let initialize_marginfi_group_ix = Instruction {
                program_id: marginfi::ID,
                accounts: marginfi::accounts::MarginfiGroupInitialize {
                    marginfi_group: group_key.pubkey(),
                    admin,
                    fee_state: fee_state_key,
                    system_program: system_program::id(),
                }
                .to_account_metas(Some(true)),
                data: marginfi::instruction::MarginfiGroupInitialize {}.data(),
            };

            let configure_marginfi_group_ix = Instruction {
                program_id: marginfi::ID,
                accounts: marginfi::accounts::MarginfiGroupConfigure {
                    marginfi_group: group_key.pubkey(),
                    admin,
                }
                .to_account_metas(Some(true)),
                data: MarginfiGroupConfigure {
                    // Payer is all admins in most test cases for simplicity, generally this is not
                    // true in production - the MS is the main admin and others are lower-impact
                    // wallets with a smaller threshold.
                    new_admin: admin,
                    new_emode_admin: admin,
                    new_curve_admin: admin,
                    new_limit_admin: admin,
                    new_emissions_admin: admin,
                    new_metadata_admin: admin,
                    new_risk_admin: admin,
                    emode_max_init_leverage: None,
                    emode_max_maint_leverage: None,
                }
                .data(),
            };

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
                        ctx.last_blockhash,
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

                let init_fee_state_ix = Instruction {
                    program_id: marginfi::ID,
                    accounts: marginfi::accounts::InitFeeState {
                        payer: ctx.payer.pubkey(),
                        fee_state: fee_state_key,
                        system_program: system_program::id(),
                    }
                    .to_account_metas(Some(true)),
                    data: InitGlobalFeeState {
                        admin: ctx.payer.pubkey(),
                        fee_wallet: fee_wallet.pubkey(),
                        bank_init_flat_sol_fee: INIT_BANK_ORIGINATION_FEE_DEFAULT,
                        liquidation_flat_sol_fee: LIQUIDATION_FLAT_FEE_DEFAULT,
                        program_fee_fixed: PROTOCOL_FEE_FIXED_DEFAULT.into(),
                        program_fee_rate: PROTOCOL_FEE_RATE_DEFAULT.into(),
                        liquidation_max_fee: LIQUIDATION_BONUS_FEE_MINIMUM.into(),
                    }
                    .data(),
                };

                let tx = Transaction::new_signed_with_payer(
                    &[
                        init_fee_state_ix,
                        initialize_marginfi_group_ix,
                        configure_marginfi_group_ix,
                    ],
                    Some(&ctx.payer.pubkey().clone()),
                    &[&ctx.payer, &group_key],
                    ctx.last_blockhash,
                );
                ctx.banks_client.process_transaction(tx).await.unwrap();
            }
        }

        MarginfiGroupFixture {
            ctx: ctx_ref.clone(),
            key: group_key.pubkey(),
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

        let accounts = marginfi::accounts::LendingPoolAddBank {
            marginfi_group: self.key,
            admin: self.ctx.borrow().payer.pubkey(),
            fee_payer: self.ctx.borrow().payer.pubkey(),
            fee_state: self.fee_state,
            global_fee_wallet: self.fee_wallet,
            bank_mint,
            bank: bank_key.pubkey(),
            liquidity_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Liquidity).0,
            liquidity_vault: bank_fixture.get_vault(BankVaultType::Liquidity).0,
            insurance_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Insurance).0,
            insurance_vault: bank_fixture.get_vault(BankVaultType::Insurance).0,
            fee_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Fee).0,
            fee_vault: bank_fixture.get_vault(BankVaultType::Fee).0,
            token_program: bank_asset_mint_fixture.token_program,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true));

        let init_ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolAddBank {
                bank_config: config_compact,
            }
            .data(),
        };

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
            self.ctx.borrow().last_blockhash,
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

        let accounts = marginfi::accounts::LendingPoolAddBankWithSeed {
            marginfi_group: self.key,
            admin: self.ctx.borrow().payer.pubkey(),
            fee_payer: self.ctx.borrow().payer.pubkey(),
            fee_state: self.fee_state,
            global_fee_wallet: self.fee_wallet,
            bank_mint,
            bank: pda,
            liquidity_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Liquidity).0,
            liquidity_vault: bank_fixture.get_vault(BankVaultType::Liquidity).0,
            insurance_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Insurance).0,
            insurance_vault: bank_fixture.get_vault(BankVaultType::Insurance).0,
            fee_vault_authority: bank_fixture.get_vault_authority(BankVaultType::Fee).0,
            fee_vault: bank_fixture.get_vault(BankVaultType::Fee).0,
            token_program: bank_fixture.get_token_program(),
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true));

        let init_ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolAddBankWithSeed {
                bank_config: config_compact,
                bank_seed,
            }
            .data(),
        };

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
            self.ctx.borrow().last_blockhash,
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
        let accounts = marginfi::accounts::LendingPoolConfigureBank {
            bank: bank.key,
            group: self.key,
            admin: self.ctx.borrow().payer.pubkey(),
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolConfigureBank { bank_config_opt }.data(),
        }
    }

    pub fn make_lending_pool_configure_bank_oracle_ix(
        &self,
        bank: &BankFixture,
        setup: u8,
        oracle: Pubkey,
        feed_oracle: Option<Pubkey>,
    ) -> Instruction {
        let mut accounts = marginfi::accounts::LendingPoolConfigureBankOracle {
            bank: bank.key,
            group: self.key,
            admin: self.ctx.borrow().payer.pubkey(),
        }
        .to_account_metas(Some(true));

        accounts.push(AccountMeta::new_readonly(
            feed_oracle.unwrap_or(oracle),
            false,
        ));

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolConfigureBankOracle { setup, oracle }.data(),
        }
    }

    pub fn make_lending_pool_set_fixed_oracle_price_ix(
        &self,
        bank: &BankFixture,
        price: WrappedI80F48,
    ) -> Instruction {
        let accounts = marginfi::accounts::LendingPoolSetFixedOraclePrice {
            group: self.key,
            admin: self.ctx.borrow().payer.pubkey(),
            bank: bank.key,
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolSetFixedOraclePrice { price }.data(),
        }
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
            self.ctx.borrow().last_blockhash,
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
        let accounts = marginfi::accounts::LendingPoolConfigureBankInterestOnly {
            group: self.key,
            delegate_curve_admin: self.ctx.borrow().payer.pubkey(),
            bank: bank.key,
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolConfigureBankInterestOnly {
                interest_rate_config,
            }
            .data(),
        }
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
            self.ctx.borrow().last_blockhash,
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
        let accounts = marginfi::accounts::LendingPoolConfigureBankLimitsOnly {
            group: self.key,
            delegate_limit_admin: self.ctx.borrow().payer.pubkey(),
            bank: bank.key,
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolConfigureBankLimitsOnly {
                deposit_limit,
                borrow_limit,
                total_asset_value_init_limit,
            }
            .data(),
        }
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
            self.ctx.borrow().last_blockhash,
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
        let accounts = marginfi::accounts::LendingPoolConfigureBankEmode {
            bank: bank.key,
            group: self.key,
            emode_admin: self.ctx.borrow().payer.pubkey(),
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolConfigureBankEmode { emode_tag, entries }.data(),
        }
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
            self.ctx.borrow().last_blockhash,
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
        let accounts = marginfi::accounts::LendingPoolCloneEmode {
            group: self.key,
            signer,
            copy_from_bank,
            copy_to_bank,
        }
        .to_account_metas(Some(true));

        Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolCloneEmode {}.data(),
        }
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
            ctx.last_blockhash,
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
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await?;

        Ok(())
    }

    pub async fn try_accrue_interest(&self, bank: &BankFixture) -> Result<()> {
        let ctx = self.ctx.borrow_mut();

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolAccrueBankInterest {
                group: self.key,
                bank: bank.key,
            }
            .to_account_metas(Some(true)),
            data: LendingPoolAccrueBankInterest {}.data(),
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

    pub async fn try_pulse_bank_price_cache(
        &self,
        bank: &BankFixture,
    ) -> Result<(), BanksClientError> {
        let bank_state = bank.load().await;

        let mut accounts = marginfi::accounts::LendingPoolPulseBankPriceCache {
            group: self.key,
            bank: bank.key,
        }
        .to_account_metas(Some(true));

        // For non-fixed oracle setups, add the primary oracle account as remaining
        if bank_state.config.oracle_setup != OracleSetup::Fixed {
            let oracle_key = bank_state.config.oracle_keys[0];
            accounts.push(AccountMeta::new_readonly(oracle_key, false));
        }

        let ctx = self.ctx.borrow_mut();

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolPulseBankPriceCache {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey().clone()),
            &[&ctx.payer],
            ctx.last_blockhash,
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
        self.try_update_with_emode_leverage(
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
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
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::MarginfiGroupConfigure {
                marginfi_group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: MarginfiGroupConfigure {
                new_admin,
                new_emode_admin,
                new_curve_admin,
                new_limit_admin,
                new_emissions_admin,
                new_metadata_admin,
                new_risk_admin,
                emode_max_init_leverage,
                emode_max_maint_leverage,
            }
            .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_update_deleverage_withdrawal_limit(
        &self,
        limit: u32,
    ) -> Result<(), BanksClientError> {
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ConfigureDeleverageWithdrawalLimit {
                marginfi_group: self.key,
                admin: self.ctx.borrow().payer.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: ConfigureDeleverageWithdrawalLimit { limit }.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey().clone()),
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

    pub async fn try_collect_fees(&self, bank: &BankFixture) -> Result<()> {
        let ctx = self.ctx.borrow_mut();

        let fee_ata = get_associated_token_address_with_program_id(
            &self.fee_wallet,
            &bank.mint.key,
            &bank.get_token_program(),
        );

        let mut accounts = marginfi::accounts::LendingPoolCollectBankFees {
            group: self.key,
            bank: bank.key,
            liquidity_vault_authority: bank.get_vault_authority(BankVaultType::Liquidity).0,
            liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
            insurance_vault: bank.get_vault(BankVaultType::Insurance).0,
            fee_vault: bank.get_vault(BankVaultType::Fee).0,
            token_program: bank.get_token_program(),
            fee_state: self.fee_state,
            fee_ata,
        }
        .to_account_metas(Some(true));
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolCollectBankFees {}.data(),
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
        let mut accounts = marginfi::accounts::LendingPoolHandleBankruptcy {
            group: self.key,
            signer: self.ctx.borrow().payer.pubkey(),
            bank: bank.key,
            marginfi_account: marginfi_account.key,
            liquidity_vault: bank.get_vault(BankVaultType::Liquidity).0,
            insurance_vault: bank.get_vault(BankVaultType::Insurance).0,
            insurance_vault_authority: bank.get_vault_authority(BankVaultType::Insurance).0,
            token_program: bank.get_token_program(),
        }
        .to_account_metas(Some(true));
        if bank.mint.token_program == anchor_spl::token_2022::ID {
            accounts.push(AccountMeta::new_readonly(bank.mint.key, false));
        }

        accounts.append(
            &mut marginfi_account
                .load_observation_account_metas(vec![], vec![])
                .await,
        );

        let ctx = self.ctx.borrow_mut();

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts,
            data: LendingPoolHandleBankruptcy {}.data(),
        };

        let nonce_ix = ComputeBudgetInstruction::set_compute_unit_price(nonce);

        let tx = Transaction::new_signed_with_payer(
            &[ix, nonce_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await
    }

    pub fn get_size() -> usize {
        8 + mem::size_of::<MarginfiGroup>()
    }

    pub async fn load(&self) -> MarginfiGroup {
        load_and_deserialize::<MarginfiGroup>(self.ctx.clone(), &self.key).await
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

        account.data[8..].copy_from_slice(data);

        ctx.set_account(&self.key, &account.into())
    }

    pub async fn try_panic_pause(&self) -> Result<(), BanksClientError> {
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PanicPause {
                global_fee_admin: self.ctx.borrow().payer.pubkey(),
                fee_state: self.fee_state,
            }
            .to_account_metas(Some(true)),
            data: PanicPause {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            self.ctx.borrow().last_blockhash,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }

    pub async fn try_panic_unpause(&self) -> Result<(), BanksClientError> {
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PanicUnpause {
                global_fee_admin: self.ctx.borrow().payer.pubkey(),
                fee_state: self.fee_state,
            }
            .to_account_metas(Some(true)),
            data: PanicUnpause {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            self.ctx.borrow().last_blockhash,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }

    pub async fn try_propagate_fee_state(&self) -> Result<(), BanksClientError> {
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PropagateFee {
                fee_state: self.fee_state,
                marginfi_group: self.key,
            }
            .to_account_metas(Some(true)),
            data: PropagateFeeState {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&self.ctx.borrow().payer.pubkey()),
            &[&self.ctx.borrow().payer],
            self.ctx.borrow().last_blockhash,
        );

        self.ctx
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }
}
