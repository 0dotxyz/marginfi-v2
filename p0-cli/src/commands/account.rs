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
    /// Build an `OrderTrigger` from CLI args. `max_slippage_bps` is in basis points (0-10000).
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
    /// List all marginfi accounts owned by the current authority
    List,
    /// Set the default marginfi account for this profile
    Use { account: Pubkey },
    /// Display account details and balances
    Get { account: Option<Pubkey> },
    /// Create a new marginfi account
    Create,
    /// Create a PDA-based marginfi account
    CreatePda {
        account_index: u16,
        #[clap(long)]
        third_party_id: Option<u16>,
    },
    /// Close the default marginfi account
    Close,
    /// Deposit tokens into a bank
    Deposit {
        bank: String,
        ui_amount: f64,
        #[clap(
            long = "up-to-limit",
            action,
            help = "If the requested deposit exceeds the bank's deposit limit, deposit only the remaining allowed amount instead of failing"
        )]
        deposit_up_to_limit: bool,
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
    /// Repay borrowed tokens
    Repay {
        bank: String,
        ui_amount: f64,
        #[clap(short = 'a', long = "all")]
        repay_all: bool,
    },
    /// Close a zero-balance position in a bank
    CloseBalance { bank: String },
    /// Transfer account authority to a new owner
    Transfer { new_authority: Pubkey },
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
    /// Initialize the liquidation-record PDA for an account
    InitLiqRecord {
        #[clap(
            long,
            help = "Account to initialize the record for (defaults to profile default)"
        )]
        account: Option<Pubkey>,
    },
    /// Close the liquidation-record PDA and return rent to the original payer (permissionless)
    CloseLiqRecord {
        #[clap(
            long,
            help = "Account whose record is being closed (defaults to profile default)"
        )]
        account: Option<Pubkey>,
    },
    /// Run the receivership liquidation flow
    LiquidateReceivership {
        #[clap(long)]
        liquidatee_marginfi_account: Pubkey,
        #[clap(
            long,
            default_value_t = false,
            help = "Auto-add init_liq_record if missing"
        )]
        init_liq_record_if_missing: bool,
        #[clap(long, help = "JSON file with extra ixs placed between start/end")]
        extra_ixs_file: Option<PathBuf>,
    },
    /// Place a stop-loss or take-profit order
    PlaceOrder {
        #[clap(long, help = "First bank pubkey (one must be an asset balance)")]
        bank_1: String,
        #[clap(long, help = "Second bank pubkey (one must be a liability balance)")]
        bank_2: String,
        #[clap(long, value_enum)]
        trigger_type: OrderTriggerTypeArg,
        #[clap(long, help = "Stop-loss threshold (required for stop-loss / both)")]
        stop_loss: Option<f64>,
        #[clap(long, help = "Take-profit threshold (required for take-profit / both)")]
        take_profit: Option<f64>,
        #[clap(long, help = "Max slippage in basis points")]
        max_slippage_bps: u32,
    },
    /// Close an existing order and reclaim lamports
    CloseOrder {
        order: Pubkey,
        #[clap(long, help = "Recipient of returned rent (defaults to signer)")]
        fee_recipient: Option<Pubkey>,
    },
    /// Keeper: close an order whose account no longer holds the relevant balances
    KeeperCloseOrder {
        #[clap(long)]
        marginfi_account: Pubkey,
        #[clap(long)]
        order: Pubkey,
        #[clap(long, help = "Recipient of returned rent (defaults to signer)")]
        fee_recipient: Option<Pubkey>,
    },
    /// Keeper: execute an order in one transaction
    ExecuteOrderKeeper {
        #[clap(long)]
        order: Pubkey,
        #[clap(long, help = "Recipient of returned rent (defaults to signer)")]
        fee_recipient: Option<Pubkey>,
        #[clap(long, help = "JSON file with extra ixs placed between start/end")]
        extra_ixs_file: Option<PathBuf>,
    },
    /// Clear keeper-close tags on balances (empty list clears all)
    SetKeeperCloseFlags {
        #[clap(long)]
        banks: Vec<Pubkey>,
    },
    /// Refresh the cached health for an account (permissionless)
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
        AccountCommand::Create => processor::marginfi_account_create(&profile, &config),
        AccountCommand::CreatePda {
            account_index,
            third_party_id,
        } => {
            processor::marginfi_account_create_pda(&profile, &config, account_index, third_party_id)
        }
        AccountCommand::Close => processor::marginfi_account_close(&profile, &config),
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
                deposit_up_to_limit.then_some(true),
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
        AccountCommand::Liquidate {
            asset_bank,
            liability_bank,
            liquidatee_marginfi_account,
            ui_asset_amount,
        } => {
            let asset_bank_pk = super::resolve_bank_for_group(&asset_bank, profile.marginfi_group)?;
            let liability_bank_pk =
                super::resolve_bank_for_group(&liability_bank, profile.marginfi_group)?;
            processor::marginfi_account_liquidate(
                &profile,
                &config,
                liquidatee_marginfi_account,
                asset_bank_pk,
                liability_bank_pk,
                ui_asset_amount,
            )
        }
        AccountCommand::InitLiqRecord { account } => {
            processor::marginfi_account_init_liquidation_record(&profile, &config, account)
        }
        AccountCommand::CloseLiqRecord { account } => {
            processor::marginfi_account_close_liquidation_record(&profile, &config, account)
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
        AccountCommand::SetKeeperCloseFlags { banks } => {
            let bank_keys_opt = if banks.is_empty() { None } else { Some(banks) };
            processor::marginfi_account_set_keeper_close_flags(&profile, &config, bank_keys_opt)
        }
        AccountCommand::PulseHealth { account } => {
            processor::marginfi_account_pulse_health(&profile, &config, account)
        }
    }?;

    Ok(())
}
