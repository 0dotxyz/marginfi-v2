use anyhow::Result;
use clap::{Parser, ValueEnum};
use fixed::types::I80F48;
use solana_sdk::pubkey::Pubkey;
use std::path::PathBuf;

use marginfi_type_crate::types::{centi_to_u32, OrderTrigger};

use crate::config::GlobalOptions;
use crate::processor;

#[derive(Clone, Copy, Debug, Parser, ValueEnum)]
pub enum OrderTriggerTypeArg {
    StopLoss,
    TakeProfit,
    Both,
}

impl OrderTriggerTypeArg {
    pub fn into_order_trigger(
        self,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
        max_slippage_bps: u32,
    ) -> Result<OrderTrigger> {
        let max_slippage = centi_to_u32(I80F48::from_num(max_slippage_bps as f64 / 10_000.0));
        match self {
            OrderTriggerTypeArg::StopLoss => {
                let threshold = stop_loss.ok_or_else(|| {
                    anyhow::anyhow!("stop_loss threshold required for StopLoss trigger")
                })?;
                Ok(OrderTrigger::StopLoss {
                    threshold: I80F48::from_num(threshold).into(),
                    max_slippage,
                })
            }
            OrderTriggerTypeArg::TakeProfit => {
                let threshold = take_profit.ok_or_else(|| {
                    anyhow::anyhow!("take_profit threshold required for TakeProfit trigger")
                })?;
                Ok(OrderTrigger::TakeProfit {
                    threshold: I80F48::from_num(threshold).into(),
                    max_slippage,
                })
            }
            OrderTriggerTypeArg::Both => {
                let sl = stop_loss.ok_or_else(|| {
                    anyhow::anyhow!("stop_loss threshold required for Both trigger")
                })?;
                let tp = take_profit.ok_or_else(|| {
                    anyhow::anyhow!("take_profit threshold required for Both trigger")
                })?;
                Ok(OrderTrigger::Both {
                    stop_loss: I80F48::from_num(sl).into(),
                    take_profit: I80F48::from_num(tp).into(),
                    max_slippage,
                })
            }
        }
    }
}

/// Marginfi account operations.
#[derive(Debug, Parser)]
pub enum AccountCommand {
    /// List all marginfi accounts for the current authority
    List,
    /// Set the default marginfi account for this profile
    Use { account: Pubkey },
    /// Display account details and balances
    Get { account: Option<Pubkey> },
    /// Deposit tokens into a bank (accepts address or symbol like SOL, USDC)
    Deposit {
        bank: String,
        ui_amount: f64,
        deposit_up_to_limit: Option<bool>,
    },
    /// Withdraw tokens from a bank
    Withdraw {
        bank: String,
        ui_amount: f64,
        #[clap(short = 'a', long = "all")]
        withdraw_all: bool,
    },
    /// Borrow tokens from a bank
    Borrow { bank: String, ui_amount: f64 },
    /// Liquidate an undercollateralized account
    Liquidate {
        #[clap(long)]
        liquidatee_marginfi_account: Pubkey,
        #[clap(long)]
        asset_bank: String,
        #[clap(long)]
        liability_bank: String,
        #[clap(long)]
        ui_asset_amount: f64,
    },
    /// Create a new marginfi account
    Create,
    /// Close the default marginfi account
    Close,
    /// Place a stop-loss or take-profit order
    PlaceOrder {
        /// First bank public key (one must be an asset balance)
        #[clap(long)]
        bank_1: String,
        /// Second bank public key (one must be a liability balance)
        #[clap(long)]
        bank_2: String,
        /// Order trigger type
        #[clap(long, value_enum)]
        trigger_type: OrderTriggerTypeArg,
        /// Stop loss threshold value (required for stop-loss and both)
        #[clap(long)]
        stop_loss: Option<f64>,
        /// Take profit threshold value (required for take-profit and both)
        #[clap(long)]
        take_profit: Option<f64>,
        /// Max slippage in basis points (bps). Required by program; defaults to 0 if omitted.
        #[clap(long)]
        max_slippage_bps: u32,
    },
    /// Close an existing order and reclaim lamports
    CloseOrder {
        order: Pubkey,
        /// Recipient of lamports from closed order account (defaults to signer)
        #[clap(long)]
        fee_recipient: Option<Pubkey>,
    },
    /// Keeper closes an order (permissionless)
    KeeperCloseOrder {
        /// Marginfi account that owns (or previously owned) the order
        #[clap(long)]
        marginfi_account: Pubkey,
        /// Order PDA to close
        #[clap(long)]
        order: Pubkey,
        /// Recipient of rent from closed order account (defaults to signer)
        #[clap(long)]
        fee_recipient: Option<Pubkey>,
    },
    /// Keeper executes an order in one tx: start_execute_order + extra ixs + end_execute_order
    ExecuteOrderKeeper {
        /// Order PDA to execute
        #[clap(long)]
        order: Pubkey,
        /// Recipient of rent from closed order/execute-record accounts (defaults to signer)
        #[clap(long)]
        fee_recipient: Option<Pubkey>,
        /// Optional path to JSON file with extra instructions placed between start/end
        #[clap(long)]
        extra_ixs_file: Option<PathBuf>,
    },
    /// Initialize liquidation record PDA for an account
    InitLiqRecord {
        /// Account to initialize the record for (defaults to profile default account)
        #[clap(long)]
        account: Option<Pubkey>,
    },
    /// Receivership liquidation bundle: (optional init) + start_liquidation + extra ixs + end_liquidation
    LiquidateReceivership {
        #[clap(long)]
        liquidatee_marginfi_account: Pubkey,
        /// If set, auto-add init_liq_record if missing
        #[clap(long, default_value_t = false)]
        init_liq_record_if_missing: bool,
        /// Optional path to JSON file with extra instructions placed between start/end
        #[clap(long)]
        extra_ixs_file: Option<PathBuf>,
    },
    /// Set keeper close flags on balance tags
    SetKeeperCloseFlags {
        /// Optional list of bank keys to clear tags for. If not provided, clears all tags.
        #[clap(long)]
        banks: Vec<Pubkey>,
    },
    /// Repay borrowed tokens
    Repay {
        bank: String,
        ui_amount: f64,
        #[clap(short = 'a', long = "all")]
        repay_all: bool,
    },
    /// Close a zero balance position in a bank
    CloseBalance { bank: String },
    /// Transfer account authority to a new owner
    Transfer { new_authority: Pubkey },
    /// Create a PDA-based marginfi account
    CreatePda {
        account_index: u16,
        #[clap(long)]
        third_party_id: Option<u16>,
    },
    /// Freeze or unfreeze a marginfi account (admin only)
    SetFreeze {
        account: Pubkey,
        #[clap(long)]
        frozen: bool,
    },
    /// Pulse health check for an account
    PulseHealth { account: Option<Pubkey> },
}

pub fn dispatch(subcmd: AccountCommand, global_options: &GlobalOptions) -> Result<()> {
    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        match subcmd {
            AccountCommand::Get { .. } | AccountCommand::List => (),
            _ => super::get_consent(&subcmd, &profile)?,
        }
    }

    match subcmd {
        AccountCommand::List => processor::marginfi_account_list(profile, &config),
        AccountCommand::Use { account } => {
            processor::marginfi_account_use(profile, &config, account)
        }
        AccountCommand::Get { account } => {
            processor::marginfi_account_get(profile, &config, account)
        }
        AccountCommand::Deposit {
            bank,
            ui_amount,
            deposit_up_to_limit,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::marginfi_account_deposit(
                &profile,
                &config,
                bank_pk,
                ui_amount,
                deposit_up_to_limit,
            )
        }
        AccountCommand::Withdraw {
            bank,
            ui_amount,
            withdraw_all,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::marginfi_account_withdraw(
                &profile,
                &config,
                bank_pk,
                ui_amount,
                withdraw_all,
            )
        }
        AccountCommand::Borrow { bank, ui_amount } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::marginfi_account_borrow(&profile, &config, bank_pk, ui_amount)
        }
        AccountCommand::Liquidate {
            asset_bank,
            liability_bank,
            liquidatee_marginfi_account: liquidatee_marginfi_account_pk,
            ui_asset_amount,
        } => {
            let asset_bank_pk = super::resolve_bank_for_group(&asset_bank, profile.marginfi_group)?;
            let liability_bank_pk =
                super::resolve_bank_for_group(&liability_bank, profile.marginfi_group)?;
            processor::marginfi_account_liquidate(
                &profile,
                &config,
                liquidatee_marginfi_account_pk,
                asset_bank_pk,
                liability_bank_pk,
                ui_asset_amount,
            )
        }
        AccountCommand::Create => processor::marginfi_account_create(&profile, &config),
        AccountCommand::Close => processor::marginfi_account_close(&profile, &config),
        AccountCommand::PlaceOrder {
            bank_1,
            bank_2,
            trigger_type,
            stop_loss,
            take_profit,
            max_slippage_bps,
        } => {
            let bank_1_pk = super::resolve_bank_for_group(&bank_1, profile.marginfi_group)?;
            let bank_2_pk = super::resolve_bank_for_group(&bank_2, profile.marginfi_group)?;
            let trigger =
                trigger_type.into_order_trigger(stop_loss, take_profit, max_slippage_bps)?;
            processor::marginfi_account_place_order(
                &profile, &config, bank_1_pk, bank_2_pk, trigger,
            )
        }
        AccountCommand::CloseOrder {
            order,
            fee_recipient,
        } => processor::marginfi_account_close_order(&profile, &config, order, fee_recipient),
        AccountCommand::KeeperCloseOrder {
            marginfi_account,
            order,
            fee_recipient,
        } => processor::marginfi_account_keeper_close_order(
            &config,
            marginfi_account,
            order,
            fee_recipient,
        ),
        AccountCommand::ExecuteOrderKeeper {
            order,
            fee_recipient,
            extra_ixs_file,
        } => processor::marginfi_account_keeper_execute_order(
            &config,
            order,
            fee_recipient,
            extra_ixs_file,
        ),
        AccountCommand::InitLiqRecord { account } => {
            processor::marginfi_account_init_liquidation_record(&profile, &config, account)
        }
        AccountCommand::LiquidateReceivership {
            liquidatee_marginfi_account,
            init_liq_record_if_missing,
            extra_ixs_file,
        } => processor::marginfi_account_liquidate_receivership(
            &config,
            liquidatee_marginfi_account,
            init_liq_record_if_missing,
            extra_ixs_file,
        ),
        AccountCommand::SetKeeperCloseFlags { banks } => {
            let bank_keys_opt = if banks.is_empty() { None } else { Some(banks) };
            processor::marginfi_account_set_keeper_close_flags(&profile, &config, bank_keys_opt)
        }
        AccountCommand::Repay {
            bank,
            ui_amount,
            repay_all,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::marginfi_account_repay(&profile, &config, bank_pk, ui_amount, repay_all)
        }
        AccountCommand::CloseBalance { bank } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::marginfi_account_close_balance(&profile, &config, bank_pk)
        }
        AccountCommand::Transfer { new_authority } => {
            processor::marginfi_account_transfer(&profile, &config, new_authority)
        }
        AccountCommand::CreatePda {
            account_index,
            third_party_id,
        } => {
            processor::marginfi_account_create_pda(&profile, &config, account_index, third_party_id)
        }
        AccountCommand::SetFreeze { account, frozen } => {
            processor::marginfi_account_set_freeze(&profile, &config, account, frozen)
        }
        AccountCommand::PulseHealth { account } => {
            processor::marginfi_account_pulse_health(&profile, &config, account)
        }
    }?;

    Ok(())
}
