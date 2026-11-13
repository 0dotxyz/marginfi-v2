#![no_main]

use anchor_lang::prelude::{AccountLoader, Clock};
use anchor_lang::solana_program::program_pack::Pack;
use anyhow::Result;
use arbitrary::Arbitrary;
use fixed::types::I80F48;
use lazy_static::lazy_static;
use libfuzzer_sys::fuzz_target;
use marginfi::{assert_eq_with_tolerance, state::bank::BankImpl};
use marginfi_fuzz::{
    account_state::AccountsState, arbitrary_helpers::*, metrics::Metrics, MarginfiFuzzContext,
};
use marginfi_type_crate::types::{Bank, MarginfiGroup};
use std::sync::{Arc, RwLock};

#[derive(Debug, Arbitrary)]
enum Action {
    Deposit {
        account: AccountIdx,
        bank: BankIdx,
        asset_amount: AssetAmount,
        deposit_up_to_limit: bool,
    },
    Borrow {
        account: AccountIdx,
        bank: BankIdx,
        asset_amount: AssetAmount,
    },
    UpdateOracle {
        bank: BankIdx,
        price: PriceChange,
    },
    Repay {
        account: AccountIdx,
        bank: BankIdx,
        asset_amount: AssetAmount,
        repay_all: bool,
    },
    Withdraw {
        account: AccountIdx,
        bank: BankIdx,
        asset_amount: AssetAmount,
        withdraw_all: bool,
    },
    Liquidate {
        liquidator: AccountIdx,
        liquidatee: AccountIdx,
        asset_amount: AssetAmount,
    },
}

#[derive(Debug)]
pub struct ActionSequence(Vec<Action>);

impl ActionSequence {
    pub const N_ACTIONS: usize = 400;
}

impl<'a> Arbitrary<'a> for ActionSequence {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let mut actions = Vec::with_capacity(Self::N_ACTIONS);

        for _ in 0..Self::N_ACTIONS {
            if u.is_empty() {
                panic!("Byte exhaustion detected, stopping early");
            }
            let action = Action::arbitrary(u)?;
            actions.push(action);
        }

        Ok(ActionSequence(actions))
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (Self::N_ACTIONS * 10, Some(Self::N_ACTIONS * 10))
    }
}

#[derive(Debug, Arbitrary)]
pub struct FuzzerContext {
    pub action_sequence: ActionSequence,
    pub initial_bank_configs: [BankAndOracleConfig; N_BANKS],
}

fuzz_target!(|data: FuzzerContext| process_actions(data).unwrap());

fn process_actions(ctx: FuzzerContext) -> Result<()> {
    let mut accounst_state = AccountsState::new();

    if !*GET_LOGGER {
        println!("Setting up logger");
    }

    let mut context =
        MarginfiFuzzContext::setup(&accounst_state, &ctx.initial_bank_configs, N_USERS as u8);

    context.metrics = METRICS.clone();

    for action in ctx.action_sequence.0.iter() {
        process_action(action, &context)?;
    }

    context.metrics.read().unwrap().print();
    context.metrics.read().unwrap().log();

    verify_end_state(&context).unwrap();

    accounst_state.reset();

    Ok(())
}

static GET_LOGGER: once_cell::sync::Lazy<bool> = once_cell::sync::Lazy::new(|| {
    #[cfg(feature = "capture_log")]
    setup_logging().unwrap();
    true
});

lazy_static! {
    static ref METRICS: Arc<RwLock<Metrics>> = Arc::new(RwLock::new(Metrics::default()));
}

#[cfg(feature = "capture_log")]
fn setup_logging() -> anyhow::Result<()> {
    let logfile = log4rs::append::file::FileAppender::builder()
        .encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new(
            "{l} - {m}\n",
        )))
        .build("log/history.log")?;

    let config = log4rs::Config::builder()
        .appender(log4rs::config::Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            log4rs::config::Root::builder()
                .appender("logfile")
                .build(log::LevelFilter::Info),
        )?;

    log4rs::init_config(config)?;

    println!("Starging logger");

    log::info!("===========START========");

    Ok(())
}

fn verify_end_state<'a>(mga: &'a MarginfiFuzzContext<'a>) -> anyhow::Result<()> {
    let group = AccountLoader::<MarginfiGroup>::try_from(&mga.marginfi_group).unwrap();
    mga.banks.iter().try_for_each(|bank| {
        let bank_loader = AccountLoader::<Bank>::try_from(&bank.bank).unwrap();
        let mut bank_data = bank_loader.load_mut().unwrap();

        let latest_timestamp = *mga.last_sysvar_current_timestamp.read().unwrap();

        let mut clock = Clock::default();

        clock.unix_timestamp = latest_timestamp as i64 + 3600;

        bank_data.accrue_interest(
            clock.unix_timestamp ,
            &group.load().unwrap()
        )?;

        let outstanding_fees = I80F48::from(bank_data.collected_group_fees_outstanding)
            + I80F48::from(bank_data.collected_insurance_fees_outstanding) + I80F48::from(bank_data.collected_program_fees_outstanding);

        let total_deposits = bank_data.get_asset_amount(bank_data.total_asset_shares.into())?;

        let total_liabilities =
            bank_data.get_liability_amount(bank_data.total_liability_shares.into())?;

        let net_accounted_balance = total_deposits - total_liabilities;

        let liquidity_vault_token_account =
            spl_token::state::Account::unpack(&bank.liquidity_vault.data.borrow()[..spl_token::state::Account::LEN]).unwrap();

        marginfi_fuzz::log!("Accounted Deposits: {}, Liabs: {}, Net {}, Outstanding Fees: {}, Net with Fees {}, Value Token Balance {}, Net Without Fees {}",
            total_deposits,
            total_liabilities,
            net_accounted_balance,
            outstanding_fees,
            net_accounted_balance + outstanding_fees,
            liquidity_vault_token_account.amount,
            liquidity_vault_token_account.amount as i64 - outstanding_fees.to_num::<i64>(),
        );

        assert_eq_with_tolerance!(I80F48::from(liquidity_vault_token_account.amount) - outstanding_fees, net_accounted_balance, I80F48::ONE);

        Ok::<_, anyhow::Error>(())
    })?;

    Ok(())
}

fn process_action<'bump>(action: &Action, mga: &'bump MarginfiFuzzContext<'bump>) -> Result<()> {
    marginfi_fuzz::log!("==================>Action {:?}", action);
    match action {
        Action::Deposit {
            account,
            bank,
            asset_amount,
            deposit_up_to_limit,
        } => mga.process_action_deposit(account, bank, asset_amount, Some(*deposit_up_to_limit))?,
        Action::Withdraw {
            account,
            bank,
            asset_amount,
            withdraw_all,
        } => mga.process_action_withdraw(account, bank, asset_amount, Some(*withdraw_all))?,
        Action::Borrow {
            account,
            bank,
            asset_amount,
        } => mga.process_action_borrow(account, bank, asset_amount)?,
        Action::Repay {
            account,
            bank,
            asset_amount,
            repay_all,
        } => mga.process_action_repay(account, bank, asset_amount, *repay_all)?,
        Action::UpdateOracle { bank, price } => mga.process_update_oracle(bank, price)?,
        Action::Liquidate {
            liquidator,
            liquidatee,
            asset_amount,
        } => mga.process_liquidate_account(liquidator, liquidatee, asset_amount)?,
    };

    mga.advance_time(3600);

    Ok(())
}
