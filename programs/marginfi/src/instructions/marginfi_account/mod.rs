mod admin_close;
mod borrow;
mod close;
mod close_balance;
mod close_liquid_record;
mod deposit;
mod emissions;
mod flashloan;
mod freeze;
mod init_liquid_record;
mod initialize;
mod liquidate;
mod liquidate_end;
mod liquidate_start;
mod order;
mod pulse_health;
mod purge_delev_balance;
mod rebalance;
mod repay;
mod sync_indexer_flags;
mod tag_liquid_record;
mod transfer_account;
mod withdraw;

use anchor_lang::{prelude::*, system_program};
use marginfi_type_crate::types::{
    MarginfiAccount, ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_REBALANCE,
    ACCOUNT_IN_RECEIVERSHIP, ACCOUNT_IN_RISK_ADMIN_LIQUIDATION,
};
use std::cell::RefMut;

use crate::{check, check_eq, prelude::*};

/// Intermediate liquidation, flashloan, order execution, or rebalance balances must not determine
/// order closure eligibility.
fn check_order_close_state(account: &MarginfiAccount) -> MarginfiResult {
    check!(
        !account.get_flag(ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_RISK_ADMIN_LIQUIDATION),
        MarginfiError::UnexpectedLiquidationState
    );
    check!(
        !account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );
    check!(
        !account.get_flag(ACCOUNT_IN_ORDER_EXECUTION),
        MarginfiError::UnexpectedOrderExecutionState
    );
    check!(
        !account.get_flag(ACCOUNT_IN_REBALANCE),
        MarginfiError::IllegalAction
    );
    Ok(())
}

/// Load a live marginfi account, or return `None` for a closed account.
fn load_marginfi_account_unless_closed<'a>(
    info: &'a AccountInfo<'_>,
) -> MarginfiResult<Option<RefMut<'a, MarginfiAccount>>> {
    if info.owner.eq(&system_program::ID) && info.data_is_empty() {
        return Ok(None);
    }
    require_keys_eq!(*info.owner, crate::ID, MarginfiError::InternalLogicError);
    let data = info.try_borrow_mut_data()?;
    require!(
        data.len() >= 8 + std::mem::size_of::<MarginfiAccount>(),
        MarginfiError::InternalLogicError
    );
    check_eq!(
        &data[..8],
        MarginfiAccount::DISCRIMINATOR,
        MarginfiError::InternalLogicError
    );
    Ok(Some(RefMut::map(data, |data| {
        bytemuck::from_bytes_mut(&mut data[8..8 + std::mem::size_of::<MarginfiAccount>()])
    })))
}

pub use admin_close::*;
pub use borrow::*;
pub use close::*;
pub use close_balance::*;
pub use close_liquid_record::*;
pub use deposit::*;
pub use emissions::*;
pub use flashloan::*;
pub use freeze::*;
pub use init_liquid_record::*;
pub use initialize::*;
pub use liquidate::*;
pub use liquidate_end::*;
pub use liquidate_start::*;
pub use order::*;
pub use pulse_health::*;
pub use purge_delev_balance::*;
pub use rebalance::*;
pub use repay::*;
pub use sync_indexer_flags::*;
pub use tag_liquid_record::*;
pub use transfer_account::*;
pub use withdraw::*;
