#[cfg(not(feature = "client"))]
use crate::events::{GroupEventHeader, LendingPoolBankAccrueInterestEvent};
use crate::{
    check,
    constants::DRIFT_SCALED_BALANCE_DECIMALS,
    debug,
    errors::MarginfiError,
    math_error,
    prelude::MarginfiResult,
    set_if_some,
    state::{
        bank_cache::update_interest_rates,
        bank_config::BankConfigImpl,
        interest_rate::{
            calc_interest_rate_accrual_state_changes, InterestRateConfigImpl,
            InterestRateStateChanges,
        },
        marginfi_account::calc_value,
        price::OraclePriceWithConfidence,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::{
    err,
    prelude::{AccountInfo, CpiContext, InterfaceAccount},
    ToAccountInfo,
};
use anchor_spl::{
    token::{transfer, Transfer},
    token_2022::spl_token_2022,
    token_interface::Mint,
};
use bytemuck::Zeroable;
use drift_mocks::constants::scale_drift_deposit_limit;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DRIFT, CLOSE_ENABLED_FLAG, EMISSION_FLAGS, FEE_VAULT_AUTHORITY_SEED,
        FEE_VAULT_SEED, FREEZE_SETTINGS, GROUP_FLAGS, INSURANCE_VAULT_AUTHORITY_SEED,
        INSURANCE_VAULT_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED,
        PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG, TOKENLESS_REPAYMENTS_ALLOWED,
    },
    types::{Bank, BankConfig, BankConfigOpt, BankOperationalState, EmodeSettings, MarginfiGroup},
};

pub trait BankImpl {
    const LEN: usize = std::mem::size_of::<Bank>();

    #[allow(clippy::too_many_arguments)]
    fn new(
        marginfi_group_pk: Pubkey,
        config: BankConfig,
        mint: Pubkey,
        mint_decimals: u8,
        liquidity_vault: Pubkey,
        insurance_vault: Pubkey,
        fee_vault: Pubkey,
        current_timestamp: i64,
        liquidity_vault_bump: u8,
        liquidity_vault_authority_bump: u8,
        insurance_vault_bump: u8,
        insurance_vault_authority_bump: u8,
        fee_vault_bump: u8,
        fee_vault_authority_bump: u8,
    ) -> Self;
    fn get_liability_amount(&self, shares: I80F48) -> MarginfiResult<I80F48>;
    fn get_asset_amount(&self, shares: I80F48) -> MarginfiResult<I80F48>;
    fn get_liability_shares(&self, value: I80F48) -> MarginfiResult<I80F48>;
    fn get_asset_shares(&self, value: I80F48) -> MarginfiResult<I80F48>;
    fn get_balance_decimals(&self) -> u8;
    fn get_remaining_deposit_capacity(&self) -> MarginfiResult<u64>;
    fn change_asset_shares(&mut self, shares: I80F48, bypass_deposit_limit: bool)
        -> MarginfiResult;
    fn maybe_get_asset_weight_init_discount(&self, price: I80F48)
        -> MarginfiResult<Option<I80F48>>;
    fn change_liability_shares(
        &mut self,
        shares: I80F48,
        bypass_borrow_limit: bool,
    ) -> MarginfiResult;
    fn check_utilization_ratio(&self) -> MarginfiResult;
    fn configure(&mut self, config: &BankConfigOpt) -> MarginfiResult;
    fn configure_unfrozen_fields_only(&mut self, config: &BankConfigOpt) -> MarginfiResult;
    fn accrue_interest(
        &mut self,
        current_timestamp: i64,
        group: &MarginfiGroup,
        #[cfg(not(feature = "client"))] bank: Pubkey,
    ) -> MarginfiResult<()>;
    fn update_bank_cache(&mut self, group: &MarginfiGroup) -> MarginfiResult<()>;
    fn update_cache_price(
        &mut self,
        oracle_price: Option<OraclePriceWithConfidence>,
    ) -> MarginfiResult<()>;
    fn deposit_spl_transfer<'info>(
        &self,
        amount: u64,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        maybe_mint: Option<&InterfaceAccount<'info, Mint>>,
        program: AccountInfo<'info>,
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult;
    fn withdraw_spl_transfer<'info>(
        &self,
        amount: u64,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        maybe_mint: Option<&InterfaceAccount<'info, Mint>>,
        program: AccountInfo<'info>,
        signer_seeds: &[&[&[u8]]],
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult;
    fn socialize_loss(&mut self, loss_amount: I80F48) -> MarginfiResult<bool>;
    fn get_flag(&self, flag: u64) -> bool;
    fn override_emissions_flag(&mut self, flag: u64);
    fn update_flag(&mut self, value: bool, flag: u64);
    fn verify_emissions_flags(flags: u64) -> bool;
    fn verify_group_flags(flags: u64) -> bool;
    fn increment_lending_position_count(&mut self);
    fn decrement_lending_position_count(&mut self);
    fn increment_borrowing_position_count(&mut self);
    fn decrement_borrowing_position_count(&mut self);
}

impl BankImpl for Bank {
    #[allow(clippy::too_many_arguments)]
    fn new(
        marginfi_group_pk: Pubkey,
        config: BankConfig,
        mint: Pubkey,
        mint_decimals: u8,
        liquidity_vault: Pubkey,
        insurance_vault: Pubkey,
        fee_vault: Pubkey,
        current_timestamp: i64,
        liquidity_vault_bump: u8,
        liquidity_vault_authority_bump: u8,
        insurance_vault_bump: u8,
        insurance_vault_authority_bump: u8,
        fee_vault_bump: u8,
        fee_vault_authority_bump: u8,
    ) -> Self {
        Self {
            mint,
            mint_decimals,
            group: marginfi_group_pk,
            asset_share_value: I80F48::ONE.into(),
            liability_share_value: I80F48::ONE.into(),
            liquidity_vault,
            liquidity_vault_bump,
            liquidity_vault_authority_bump,
            insurance_vault,
            insurance_vault_bump,
            insurance_vault_authority_bump,
            collected_insurance_fees_outstanding: I80F48::ZERO.into(),
            fee_vault,
            fee_vault_bump,
            fee_vault_authority_bump,
            collected_group_fees_outstanding: I80F48::ZERO.into(),
            total_liability_shares: I80F48::ZERO.into(),
            total_asset_shares: I80F48::ZERO.into(),
            last_update: current_timestamp,
            config,
            flags: CLOSE_ENABLED_FLAG,
            emissions_rate: 0,
            emissions_remaining: I80F48::ZERO.into(),
            emissions_mint: Pubkey::default(),
            collected_program_fees_outstanding: I80F48::ZERO.into(),
            emode: EmodeSettings::zeroed(),
            fees_destination_account: Pubkey::default(),
            lending_position_count: 0,
            borrowing_position_count: 0,
            _padding_0: [0; 16],
            integration_acc_1: Pubkey::default(),
            integration_acc_2: Pubkey::default(),
            ..Default::default()
        }
    }

    fn get_liability_amount(&self, shares: I80F48) -> MarginfiResult<I80F48> {
        Ok(shares
            .checked_mul(self.liability_share_value.into())
            .ok_or_else(math_error!())?)
    }

    fn get_asset_amount(&self, shares: I80F48) -> MarginfiResult<I80F48> {
        Ok(shares
            .checked_mul(self.asset_share_value.into())
            .ok_or_else(math_error!())?)
    }

    fn get_liability_shares(&self, value: I80F48) -> MarginfiResult<I80F48> {
        Ok(value
            .checked_div(self.liability_share_value.into())
            .ok_or_else(math_error!())?)
    }

    fn get_asset_shares(&self, value: I80F48) -> MarginfiResult<I80F48> {
        if self.asset_share_value == I80F48::ZERO.into() {
            return Ok(I80F48::ZERO);
        }
        Ok(value
            .checked_div(self.asset_share_value.into())
            .ok_or_else(math_error!())?)
    }

    fn get_balance_decimals(&self) -> u8 {
        if self.config.asset_tag == ASSET_TAG_DRIFT {
            DRIFT_SCALED_BALANCE_DECIMALS
        } else {
            self.mint_decimals
        }
    }

    fn get_remaining_deposit_capacity(&self) -> MarginfiResult<u64> {
        if !self.config.is_deposit_limit_active() {
            return Ok(u64::MAX);
        }

        let current_assets = self.get_asset_amount(self.total_asset_shares.into())?;

        let limit = if self.config.asset_tag == ASSET_TAG_DRIFT {
            scale_drift_deposit_limit(self.config.deposit_limit, self.mint_decimals)?
        } else {
            I80F48::from_num(self.config.deposit_limit)
        };

        if current_assets >= limit {
            return Ok(0);
        }

        let remaining = limit
            .checked_sub(current_assets)
            .ok_or_else(math_error!())?
            .checked_sub(I80F48::ONE) // Subtract 1 to ensure we stay under limit
            .ok_or_else(math_error!())?
            .checked_floor()
            .ok_or_else(math_error!())?
            .checked_to_num::<u64>()
            .ok_or_else(math_error!())?;

        Ok(remaining)
    }

    fn change_asset_shares(
        &mut self,
        shares: I80F48,
        bypass_deposit_limit: bool,
    ) -> MarginfiResult {
        let total_asset_shares: I80F48 = self.total_asset_shares.into();
        self.total_asset_shares = total_asset_shares
            .checked_add(shares)
            .ok_or_else(math_error!())?
            .into();

        if shares.is_positive() && self.config.is_deposit_limit_active() && !bypass_deposit_limit {
            let total_deposits_amount = self.get_asset_amount(self.total_asset_shares.into())?;

            // For Drift banks, deposit_limit is in native decimals but total_deposits_amount
            // is in 9-decimal (DRIFT_SCALED_BALANCE_DECIMALS). We Scale deposit_limit to match.
            let deposit_limit = if self.config.asset_tag == ASSET_TAG_DRIFT {
                scale_drift_deposit_limit(self.config.deposit_limit, self.mint_decimals)?
            } else {
                I80F48::from_num(self.config.deposit_limit)
            };

            if total_deposits_amount >= deposit_limit {
                let deposits_num: f64 = total_deposits_amount.to_num();
                let limit_num: f64 = deposit_limit.to_num();
                msg!("deposits: {:?} deposit lim: {:?}", deposits_num, limit_num);
                return err!(MarginfiError::BankAssetCapacityExceeded);
            }
        }

        Ok(())
    }

    fn maybe_get_asset_weight_init_discount(
        &self,
        price: I80F48,
    ) -> MarginfiResult<Option<I80F48>> {
        if self.config.usd_init_limit_active() {
            let bank_total_assets_value = calc_value(
                self.get_asset_amount(self.total_asset_shares.into())?,
                price,
                self.get_balance_decimals(),
                None,
            )?;

            let total_asset_value_init_limit =
                I80F48::from_num(self.config.total_asset_value_init_limit);

            #[cfg(target_os = "solana")]
            debug!(
                "Init limit active, limit: {}, total_assets: {}",
                total_asset_value_init_limit, bank_total_assets_value
            );

            if bank_total_assets_value > total_asset_value_init_limit {
                let discount = total_asset_value_init_limit
                    .checked_div(bank_total_assets_value)
                    .ok_or_else(math_error!())?;

                #[cfg(target_os = "solana")]
                debug!(
                    "Discounting assets by {:.2} because of total deposits {} over {} usd cap",
                    discount, bank_total_assets_value, total_asset_value_init_limit
                );

                Ok(Some(discount))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn change_liability_shares(
        &mut self,
        shares: I80F48,
        bypass_borrow_limit: bool,
    ) -> MarginfiResult {
        let total_liability_shares: I80F48 = self.total_liability_shares.into();
        self.total_liability_shares = total_liability_shares
            .checked_add(shares)
            .ok_or_else(math_error!())?
            .into();

        if !bypass_borrow_limit && shares.is_positive() && self.config.is_borrow_limit_active() {
            let total_liability_amount =
                self.get_liability_amount(self.total_liability_shares.into())?;
            let borrow_limit = I80F48::from_num(self.config.borrow_limit);

            if total_liability_amount >= borrow_limit {
                let liab_num: f64 = total_liability_amount.to_num();
                let borrow_num: f64 = borrow_limit.to_num();
                msg!("amt: {:?} borrow lim: {:?}", liab_num, borrow_num);
                return err!(MarginfiError::BankLiabilityCapacityExceeded);
            }
        }

        Ok(())
    }

    fn check_utilization_ratio(&self) -> MarginfiResult {
        let total_assets = self.get_asset_amount(self.total_asset_shares.into())?;
        let total_liabilities = self.get_liability_amount(self.total_liability_shares.into())?;

        if total_assets < total_liabilities {
            let assets_num: f64 = total_assets.to_num();
            let liabs_num: f64 = total_liabilities.to_num();
            msg!("assets: {:?} liabs: {:?}", assets_num, liabs_num);
            return err!(MarginfiError::IllegalUtilizationRatio);
        }

        Ok(())
    }

    fn configure(&mut self, config: &BankConfigOpt) -> MarginfiResult {
        set_if_some!(self.config.asset_weight_init, config.asset_weight_init);
        set_if_some!(self.config.asset_weight_maint, config.asset_weight_maint);
        set_if_some!(
            self.config.liability_weight_init,
            config.liability_weight_init
        );
        set_if_some!(
            self.config.liability_weight_maint,
            config.liability_weight_maint
        );
        set_if_some!(self.config.deposit_limit, config.deposit_limit);

        set_if_some!(self.config.borrow_limit, config.borrow_limit);

        if let Some(new_state) = config.operational_state {
            check!(
                new_state != BankOperationalState::KilledByBankruptcy,
                MarginfiError::Unauthorized
            );
            self.config.operational_state = new_state;
        }

        if let Some(ir_config) = &config.interest_rate_config {
            self.config.interest_rate_config.update(ir_config);
        }

        set_if_some!(self.config.risk_tier, config.risk_tier);

        set_if_some!(self.config.asset_tag, config.asset_tag);

        set_if_some!(
            self.config.total_asset_value_init_limit,
            config.total_asset_value_init_limit
        );

        set_if_some!(
            self.config.oracle_max_confidence,
            config.oracle_max_confidence
        );

        set_if_some!(self.config.oracle_max_age, config.oracle_max_age);

        if let Some(flag) = config.permissionless_bad_debt_settlement {
            msg!(
                "setting bad debt settlement: {:?}",
                config.permissionless_bad_debt_settlement.unwrap()
            );
            self.update_flag(flag, PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG);
        }

        if let Some(flag) = config.freeze_settings {
            msg!(
                "setting freeze settings: {:?}",
                config.freeze_settings.unwrap()
            );
            self.update_flag(flag, FREEZE_SETTINGS);
        }

        if let Some(flag) = config.tokenless_repayments_allowed {
            msg!(
                "setting tokenless repayments allowed: {:?}",
                config.tokenless_repayments_allowed.unwrap()
            );
            self.update_flag(flag, TOKENLESS_REPAYMENTS_ALLOWED);
        }

        self.config.validate()?;

        Ok(())
    }

    /// Configures just the borrow and deposit limits, ignoring all other values
    fn configure_unfrozen_fields_only(&mut self, config: &BankConfigOpt) -> MarginfiResult {
        set_if_some!(self.config.deposit_limit, config.deposit_limit);
        set_if_some!(self.config.borrow_limit, config.borrow_limit);
        // weights didn't change so no validation is needed
        Ok(())
    }

    /// Calculate the interest rate accrual state changes for a given time period
    ///
    /// Collected protocol and insurance fees are stored in state.
    /// A separate instruction is required to withdraw these fees.
    fn accrue_interest(
        &mut self,
        current_timestamp: i64,
        group: &MarginfiGroup,
        #[cfg(not(feature = "client"))] bank: Pubkey,
    ) -> MarginfiResult<()> {
        #[cfg(all(not(feature = "client"), feature = "debug"))]
        anchor_lang::solana_program::log::sol_log_compute_units();

        let time_delta: u64 = (current_timestamp - self.last_update).try_into().unwrap();
        if time_delta == 0 {
            return Ok(());
        }

        let total_assets = self.get_asset_amount(self.total_asset_shares.into())?;
        let total_liabilities = self.get_liability_amount(self.total_liability_shares.into())?;

        self.last_update = current_timestamp;

        if (total_assets == I80F48::ZERO) || (total_liabilities == I80F48::ZERO) {
            #[cfg(not(feature = "client"))]
            emit!(LendingPoolBankAccrueInterestEvent {
                header: GroupEventHeader {
                    marginfi_group: self.group,
                    signer: None
                },
                bank,
                mint: self.mint,
                delta: time_delta,
                fees_collected: 0.,
                insurance_collected: 0.,
            });

            return Ok(());
        }
        let ir_calc = self
            .config
            .interest_rate_config
            .create_interest_rate_calculator(group);

        let InterestRateStateChanges {
            new_asset_share_value: asset_share_value,
            new_liability_share_value: liability_share_value,
            insurance_fees_collected,
            group_fees_collected,
            protocol_fees_collected,
        } = calc_interest_rate_accrual_state_changes(
            time_delta,
            total_assets,
            total_liabilities,
            &ir_calc,
            self.asset_share_value.into(),
            self.liability_share_value.into(),
        )
        .ok_or_else(math_error!())?;

        debug!("deposit share value: {}\nliability share value: {}\nfees collected: {}\ninsurance collected: {}",
            asset_share_value, liability_share_value, group_fees_collected, insurance_fees_collected);

        self.cache.accumulated_since_last_update = asset_share_value
            .checked_sub(I80F48::from(self.asset_share_value))
            .and_then(|v| v.checked_mul(I80F48::from(self.total_asset_shares)))
            .ok_or_else(math_error!())?
            .into();
        self.cache.interest_accumulated_for = time_delta.min(u32::MAX as u64) as u32;
        self.asset_share_value = asset_share_value.into();
        self.liability_share_value = liability_share_value.into();

        if group_fees_collected > I80F48::ZERO {
            self.collected_group_fees_outstanding = {
                group_fees_collected
                    .checked_add(self.collected_group_fees_outstanding.into())
                    .ok_or_else(math_error!())?
                    .into()
            };
        }

        if insurance_fees_collected > I80F48::ZERO {
            self.collected_insurance_fees_outstanding = {
                insurance_fees_collected
                    .checked_add(self.collected_insurance_fees_outstanding.into())
                    .ok_or_else(math_error!())?
                    .into()
            };
        }
        if protocol_fees_collected > I80F48::ZERO {
            self.collected_program_fees_outstanding = {
                protocol_fees_collected
                    .checked_add(self.collected_program_fees_outstanding.into())
                    .ok_or_else(math_error!())?
                    .into()
            };
        }

        #[cfg(not(feature = "client"))]
        {
            #[cfg(feature = "debug")]
            anchor_lang::solana_program::log::sol_log_compute_units();

            emit!(LendingPoolBankAccrueInterestEvent {
                header: GroupEventHeader {
                    marginfi_group: self.group,
                    signer: None
                },
                bank,
                mint: self.mint,
                delta: time_delta,
                fees_collected: group_fees_collected.to_num::<f64>(),
                insurance_collected: insurance_fees_collected.to_num::<f64>(),
            });
        }

        Ok(())
    }

    /// Updates bank cache with the actual values for interest/fee rates.
    ///
    /// Should be called in the end of each instruction calling `accrue_interest` to ensure the cache is up to date.
    ///
    /// # Arguments
    /// * `group` - The marginfi group
    fn update_bank_cache(&mut self, group: &MarginfiGroup) -> MarginfiResult<()> {
        let total_assets_amount: I80F48 = self.get_asset_amount(self.total_asset_shares.into())?;
        let total_liabilities_amount: I80F48 =
            self.get_liability_amount(self.total_liability_shares.into())?;

        if (total_assets_amount == I80F48::ZERO) || (total_liabilities_amount == I80F48::ZERO) {
            self.cache.reset_preserving_oracle_price();
            return Ok(());
        }

        let ir_calc = self
            .config
            .interest_rate_config
            .create_interest_rate_calculator(group);

        let utilization_rate: I80F48 = total_liabilities_amount
            .checked_div(total_assets_amount)
            .ok_or_else(math_error!())?;
        let interest_rates = ir_calc
            .calc_interest_rate(utilization_rate)
            .ok_or_else(math_error!())?;

        update_interest_rates(&mut self.cache, &interest_rates);

        // Update banks last update timestamp
        self.last_update = Clock::get()?.unix_timestamp;
        Ok(())
    }

    /// Updates bank cache with the last used oracle price.
    ///
    /// Should be called in instructions that consume a bank's price to record the last-used value.
    ///
    /// # Arguments
    /// * `group` - The marginfi group
    /// * `oracle_price` - Optional oracle price (with confidence) used in this instruction (if any)
    fn update_cache_price(
        &mut self,
        oracle_price: Option<OraclePriceWithConfidence>,
    ) -> MarginfiResult<()> {
        if let Some(price_with_confidence) = oracle_price {
            self.cache.last_oracle_price = price_with_confidence.price.into();
            self.cache.last_oracle_price_confidence = price_with_confidence.confidence.into();
            self.cache.last_oracle_price_timestamp = Clock::get()?.unix_timestamp;
        } else {
            // no cache update, nothing...
        }

        Ok(())
    }

    fn deposit_spl_transfer<'info>(
        &self,
        amount: u64,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        maybe_mint: Option<&InterfaceAccount<'info, Mint>>,
        program: AccountInfo<'info>,
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult {
        check!(
            to.key.eq(&self.liquidity_vault),
            MarginfiError::InvalidTransfer
        );

        debug!(
            "deposit_spl_transfer: amount: {} from {} to {}, auth {}",
            amount, from.key, to.key, authority.key
        );

        if let Some(mint) = maybe_mint {
            spl_token_2022::onchain::invoke_transfer_checked(
                program.key,
                from,
                mint.to_account_info(),
                to,
                authority,
                remaining_accounts,
                amount,
                mint.decimals,
                &[],
            )?;
        } else {
            #[allow(deprecated)]
            transfer(
                CpiContext::new_with_signer(
                    program,
                    Transfer {
                        from,
                        to,
                        authority,
                    },
                    &[],
                ),
                amount,
            )?;
        }

        Ok(())
    }

    fn withdraw_spl_transfer<'info>(
        &self,
        amount: u64,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        maybe_mint: Option<&InterfaceAccount<'info, Mint>>,
        program: AccountInfo<'info>,
        signer_seeds: &[&[&[u8]]],
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult {
        debug!(
            "withdraw_spl_transfer: amount: {} from {} to {}, auth {}",
            amount, from.key, to.key, authority.key
        );

        if let Some(mint) = maybe_mint {
            spl_token_2022::onchain::invoke_transfer_checked(
                program.key,
                from,
                mint.to_account_info(),
                to,
                authority,
                remaining_accounts,
                amount,
                mint.decimals,
                signer_seeds,
            )?;
        } else {
            // `transfer_checked` and `transfer` does the same thing, the additional `_checked` logic
            // is only to assert the expected attributes by the user (mint, decimal scaling),
            //
            // Security of `transfer` is equal to `transfer_checked`.
            #[allow(deprecated)]
            transfer(
                CpiContext::new_with_signer(
                    program,
                    Transfer {
                        from,
                        to,
                        authority,
                    },
                    signer_seeds,
                ),
                amount,
            )?;
        }

        Ok(())
    }

    /// Socialize a loss of `loss_amount` among depositors, the `total_deposit_shares` stays the
    /// same, but total value of deposits is reduced by `loss_amount`;
    ///
    /// In cases where assets < liabilities, the asset share value will be set to zero, but cannot
    /// go negative. Effectively, depositors forfeit their entire deposit AND all earned interest in
    /// this case.
    fn socialize_loss(&mut self, loss_amount: I80F48) -> MarginfiResult<bool> {
        let mut kill_bank = false;
        let total_asset_shares: I80F48 = self.total_asset_shares.into();
        let old_asset_share_value: I80F48 = self.asset_share_value.into();

        // Compute total "old" value of shares
        let total_value: I80F48 = total_asset_shares
            .checked_mul(old_asset_share_value)
            .ok_or_else(math_error!())?;

        // Subtract loss, clamping at zero (i.e. assets < liabilities, the bank is wiped out)
        if total_value <= loss_amount {
            self.asset_share_value = I80F48::ZERO.into();
            // This state is irrecoverable, the bank is dead.
            kill_bank = true;
        } else {
            // otherwise subtract then redistribute
            let new_share_value: I80F48 = (total_value - loss_amount)
                .checked_div(total_asset_shares)
                .ok_or_else(math_error!())?;
            self.asset_share_value = new_share_value.into();
            // Sanity check: should be unreachable.
            if new_share_value == I80F48::ZERO {
                kill_bank = true;
            }
        }

        Ok(kill_bank)
    }

    fn get_flag(&self, flag: u64) -> bool {
        (self.flags & flag) == flag
    }

    fn override_emissions_flag(&mut self, flag: u64) {
        assert!(Self::verify_emissions_flags(flag));
        self.flags = flag;
    }

    fn update_flag(&mut self, value: bool, flag: u64) {
        assert!(Self::verify_group_flags(flag));

        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    fn verify_emissions_flags(flags: u64) -> bool {
        flags & EMISSION_FLAGS == flags
    }

    fn verify_group_flags(flags: u64) -> bool {
        flags & GROUP_FLAGS == flags
    }

    fn increment_lending_position_count(&mut self) {
        self.lending_position_count = self.lending_position_count.saturating_add(1);
    }

    fn decrement_lending_position_count(&mut self) {
        self.lending_position_count = self.lending_position_count.saturating_sub(1);
    }

    fn increment_borrowing_position_count(&mut self) {
        self.borrowing_position_count = self.borrowing_position_count.saturating_add(1);
    }

    fn decrement_borrowing_position_count(&mut self) {
        self.borrowing_position_count = self.borrowing_position_count.saturating_sub(1);
    }
}

#[derive(Debug, Clone)]
pub enum BankVaultType {
    Liquidity,
    Insurance,
    Fee,
}

impl BankVaultType {
    pub fn get_seed(self) -> &'static [u8] {
        match self {
            BankVaultType::Liquidity => LIQUIDITY_VAULT_SEED.as_bytes(),
            BankVaultType::Insurance => INSURANCE_VAULT_SEED.as_bytes(),
            BankVaultType::Fee => FEE_VAULT_SEED.as_bytes(),
        }
    }

    pub fn get_authority_seed(self) -> &'static [u8] {
        match self {
            BankVaultType::Liquidity => LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            BankVaultType::Insurance => INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(),
            BankVaultType::Fee => FEE_VAULT_AUTHORITY_SEED.as_bytes(),
        }
    }
}
