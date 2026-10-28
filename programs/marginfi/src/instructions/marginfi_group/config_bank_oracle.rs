use crate::{
    check,
    events::{
        GroupEventHeader, LendingPoolBankConfigureOracleEvent, LendingPoolBankSetOraclePriceEvent,
    },
    ix_utils,
    state::bank::BankImpl,
    state::bank_config::BankConfigImpl,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::constants::{BANK_SAME_ASSET_EMODE_ELIGIBLE, FREEZE_SETTINGS};
use marginfi_type_crate::types::{Bank, MarginfiGroup, OracleSetup, WrappedI80F48};

pub fn lending_pool_configure_bank_oracle(
    ctx: Context<LendingPoolConfigureBankOracle>,
    setup: u8,
    oracle: Pubkey,
) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let mut bank = ctx.accounts.bank.load_mut()?;

    // If settings are frozen, you can only update the deposit and borrow limits, so this ix will fail
    if bank.get_flag(FREEZE_SETTINGS) {
        panic!("cannot change oracle settings on frozen banks");
    } else {
        let setup_type =
            OracleSetup::from_u8(setup).unwrap_or_else(|| panic!("unsupported oracle type"));
        if matches!(
            setup_type,
            OracleSetup::Fixed
                | OracleSetup::FixedKamino
                | OracleSetup::FixedDrift
                | OracleSetup::FixedJuplend
                | OracleSetup::PTPyth
                | OracleSetup::PTFixed
        ) {
            return err!(MarginfiError::UseSetOraclePrice);
        }
        // Scope-priced banks must go through `lending_pool_configure_bank_oracle_scope`, which
        // takes the entry index; this instruction has no way to provide it.
        if setup_type.is_scope_priced() {
            return err!(MarginfiError::UseConfigureBankOracleScope);
        }
        // Scope entries are price identity too, but every Scope-priced setup has a family of its
        // own, so the family comparison covers a move off Scope.
        check!(
            !bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE)
                || (bank.config.oracle_keys[0] == oracle
                    && bank.config.oracle_setup.feed_family() == setup_type.feed_family()),
            MarginfiError::BadEmodeConfig,
            "disable same-asset e-mode eligibility before changing the feed family or \
             oracle_keys[0]"
        );

        bank.config.oracle_setup = setup_type;
        bank.config.oracle_keys[0] = oracle;
        bank.config.fixed_price = I80F48::ZERO.into();
        // A Pyth / Switchboard feed: drop the Scope entries left from a prior setup.
        bank.config.scope_entry_index = 0;
        bank.config.set_scope_smoothed_entry(None);

        // mSOL / LST setups carry multiplier oracle key (Marinade State / SPL StakePool) beyond the primary feed, populated here from
        // remaining_accounts (validated immediately below):
        match setup_type {
            OracleSetup::PythMSOL | OracleSetup::PythLST => {
                require!(
                    ctx.remaining_accounts.len() == 2,
                    MarginfiError::WrongNumberOfOracleAccounts
                );
                bank.config.oracle_keys[1] = ctx.remaining_accounts[1].key();
            }
            OracleSetup::KaminoMSOL
            | OracleSetup::JuplendMSOL
            | OracleSetup::KaminoLST
            | OracleSetup::JuplendLST => {
                require!(
                    ctx.remaining_accounts.len() == 3,
                    MarginfiError::WrongNumberOfOracleAccounts
                );
                // Note: integration oracle is not set here to ensure it's only assigned on the bank creation.
                bank.config.oracle_keys[2] = ctx.remaining_accounts[2].key();
            }
            _ => {}
        }

        msg!(
            "setting oracle to type: {:?} key: {:?}",
            bank.config.oracle_setup,
            bank.config.oracle_keys[0]
        );

        bank.config
            .validate_oracle_setup(bank.mint, ctx.remaining_accounts, None, None, None)?;

        emit!(LendingPoolBankConfigureOracleEvent {
            header: GroupEventHeader {
                marginfi_group: ctx.accounts.group.key(),
                signer: Some(*ctx.accounts.governance_admin.key)
            },
            bank: ctx.accounts.bank.key(),
            oracle_setup: setup,
            oracle
        });
    }

    Ok(())
}

/// Configure a bank to price from a Scope feed entry: the feed's `OraclePrices` account goes in
/// `oracle_keys[0]` and the entry in `scope_entry_index`. `setup` names the resulting setup:
///
/// | `setup`                                         | remaining accounts        |
/// |-------------------------------------------------|---------------------------|
/// | `Scope`                                         | `[feed]`                  |
/// | `ScopeKamino` / `ScopeJuplend`                  | `[feed, venue]`           |
/// | `ScopeMSOL` / `ScopeLST`                        | `[feed, rate]`            |
/// | `ScopePT` (with `pt_start_price`)               | `[feed, exponent_vault]`  |
/// | `ScopeKaminoMSOL` / `ScopeKaminoLST` / `ScopeJuplend*` | `[feed, venue, rate]` |
///
/// `feed` is the `OraclePrices` account; `rate` is the Marinade State or SPL stake pool. The
/// Scope-fed multiplier setups are the twins of `PythMSOL` / `PythLST` / `PTPyth` and of the
/// `Kamino*` / `Juplend*` mSOL / LST wrappers, and read the Scope entry as their base price. `pt_start_price` is the PT linear-pricing start
/// price in (0, 1], required for `ScopePT` and rejected otherwise. The venue account must match
/// the one stored in `oracle_keys[1]` at bank creation; it is never rewritten here. Staked banks
/// take their oracle from `StakedSettings` instead and are rejected.
///
/// `smoothed_entry_index` sets the entry used for the time-weighted price (initial margin): the
/// `PythLazerEMA` (or `ScopeTwap*`) of `entry_index` in the same feed. Which entry that is rests
/// with the admin, like `entry_index` itself; the program only checks it is a readable, nonzero
/// entry other than `entry_index`. `None` uses the spot price for both price types.
///
/// Separate from `lending_pool_configure_bank_oracle` because Scope configuration needs entry
/// indices alongside the feed account key.
pub fn lending_pool_configure_bank_oracle_scope(
    ctx: Context<LendingPoolConfigureBankOracle>,
    setup: u8,
    oracle: Pubkey,
    entry_index: u16,
    pt_start_price: Option<WrappedI80F48>,
    smoothed_entry_index: Option<u16>,
) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let mut bank = ctx.accounts.bank.load_mut()?;

    if bank.get_flag(FREEZE_SETTINGS) {
        panic!("cannot change oracle settings on frozen banks");
    }

    // The entry index is pricing state that a feed-family comparison does not cover, so
    // same-asset e-mode must be disabled first.
    check!(
        !bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE),
        MarginfiError::BadEmodeConfig,
        "disable same-asset e-mode eligibility before moving a bank to a Scope oracle"
    );

    let setup_type =
        OracleSetup::from_u8(setup).unwrap_or_else(|| panic!("unsupported oracle type"));
    validate_scope_setup(setup_type, pt_start_price.is_some())?;
    // Entry 0 is the "no smoothed entry" value, so it can never be used as one.
    check!(
        smoothed_entry_index != Some(0),
        MarginfiError::ScopeInvalidSmoothedEntry,
        "entry 0 cannot be a smoothed entry"
    );
    let remaining = ctx.remaining_accounts;

    bank.config.oracle_setup = setup_type;
    bank.config.oracle_keys[0] = oracle;
    bank.config.fixed_price = I80F48::ZERO.into();
    bank.config.scope_entry_index = entry_index;

    match setup_type {
        // [0] Scope feed, [1] Marinade State / SPL StakePool / Exponent vault
        OracleSetup::ScopeMSOL | OracleSetup::ScopeLST | OracleSetup::ScopePT => {
            check!(
                remaining.len() == 2,
                MarginfiError::WrongNumberOfOracleAccounts
            );
            bank.config.oracle_keys[1] = remaining[1].key();
        }
        // [0] Scope feed, [1] venue (pinned at creation), [2] Marinade State / SPL StakePool
        OracleSetup::ScopeKaminoMSOL
        | OracleSetup::ScopeJuplendMSOL
        | OracleSetup::ScopeKaminoLST
        | OracleSetup::ScopeJuplendLST => {
            check!(
                remaining.len() == 3,
                MarginfiError::WrongNumberOfOracleAccounts
            );
            bank.config.oracle_keys[2] = remaining[2].key();
        }
        _ => {}
    }
    if let Some(start_price) = pt_start_price {
        let start_price_i80: I80F48 = start_price.into();
        check!(
            start_price_i80 > I80F48::ZERO && start_price_i80 <= I80F48::ONE,
            MarginfiError::InvalidPtStartPrice
        );
        bank.config.fixed_price = start_price;
    }

    bank.config.set_scope_smoothed_entry(smoothed_entry_index);

    msg!(
        "setting scope oracle type: {:?} key: {:?} entry: {:?} smoothed entry: {:?}",
        setup_type,
        oracle,
        entry_index,
        smoothed_entry_index
    );

    bank.config
        .validate_oracle_setup(bank.mint, remaining, None, None, None)?;

    let header = || GroupEventHeader {
        marginfi_group: ctx.accounts.group.key(),
        signer: Some(*ctx.accounts.governance_admin.key),
    };
    if let Some(price) = pt_start_price {
        emit!(LendingPoolBankSetOraclePriceEvent {
            header: header(),
            bank: ctx.accounts.bank.key(),
            price,
        });
    }
    emit!(LendingPoolBankConfigureOracleEvent {
        header: header(),
        bank: ctx.accounts.bank.key(),
        oracle_setup: setup_type as u8,
        oracle
    });

    Ok(())
}

/// Checks `requested` is priced from a Scope entry and that `pt_start_price` is present exactly
/// for `ScopePT`. Staked banks take their oracle from `StakedSettings` and are rejected.
fn validate_scope_setup(requested: OracleSetup, has_pt_start_price: bool) -> MarginfiResult {
    check!(
        requested.is_scope_priced() && requested != OracleSetup::StakedWithScope,
        MarginfiError::InvalidOracleSetup,
        "setup is not priced from a Scope entry (staked banks use StakedSettings)"
    );
    check!(
        has_pt_start_price == (requested == OracleSetup::ScopePT),
        if requested == OracleSetup::ScopePT {
            MarginfiError::InvalidPtStartPrice
        } else {
            MarginfiError::InvalidOracleSetup
        },
        "pt_start_price is required for ScopePT and not accepted for any other setup"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_setup_accepts_scope_priced_setups_only() {
        for setup in [
            OracleSetup::Scope,
            OracleSetup::ScopeKamino,
            OracleSetup::ScopeJuplend,
            OracleSetup::ScopeMSOL,
            OracleSetup::ScopeLST,
            OracleSetup::ScopeKaminoLST,
            OracleSetup::ScopeKaminoMSOL,
            OracleSetup::ScopeJuplendMSOL,
            OracleSetup::ScopeJuplendLST,
        ] {
            validate_scope_setup(setup, false).unwrap();
            assert_eq!(
                validate_scope_setup(setup, true).unwrap_err(),
                MarginfiError::InvalidOracleSetup.into(),
                "{setup:?} must reject a PT start price"
            );
        }
        for setup in [
            OracleSetup::None,
            OracleSetup::PythPushOracle,
            OracleSetup::KaminoPythPush,
            OracleSetup::SwitchboardPull,
            OracleSetup::Fixed,
            OracleSetup::PTFixed,
            OracleSetup::PTPyth,
            OracleSetup::PythMSOL,
            OracleSetup::PythLST,
            OracleSetup::KaminoLST,
            OracleSetup::KaminoMSOL,
            OracleSetup::JuplendMSOL,
            OracleSetup::JuplendLST,
            OracleSetup::StakedWithPythPush,
            OracleSetup::StakedWithScope,
        ] {
            assert_eq!(
                validate_scope_setup(setup, false).unwrap_err(),
                MarginfiError::InvalidOracleSetup.into(),
                "{setup:?}"
            );
        }
    }

    #[test]
    fn scope_setup_requires_a_start_price_for_pt() {
        validate_scope_setup(OracleSetup::ScopePT, true).unwrap();
        assert_eq!(
            validate_scope_setup(OracleSetup::ScopePT, false).unwrap_err(),
            MarginfiError::InvalidPtStartPrice.into()
        );
    }
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBankOracle<'info> {
    #[account(has_one = governance_admin @ MarginfiError::Unauthorized)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub governance_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}
