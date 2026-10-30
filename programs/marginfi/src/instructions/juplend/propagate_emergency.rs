use anchor_lang::prelude::*;
use juplend_mocks::state::{Lending, TOKEN_RESERVE_DISCRIMINATOR};
use marginfi_type_crate::{
    constants::{ASSET_TAG_JUPLEND, JUPLEND_EMERGENCY},
    pdas::{derive_juplend_liquidity, JUPLEND_LIQUIDITY_PROGRAM_ID},
    types::Bank,
};

use crate::{MarginfiError, MarginfiResult};

const LIQUIDITY_DISCRIMINATOR: [u8; 8] = [54, 252, 249, 226, 137, 172, 121, 58];
const SUPPLY_POSITION_DISCRIMINATOR: [u8; 8] = [202, 219, 136, 118, 61, 177, 21, 146];
const LIQUIDITY_STATUS_OFFSET: usize = 8 + 32 + 32;
const RESERVE_INTERACTING_PROTOCOL_OFFSET: usize = 144;
const RESERVE_LEN: usize = 192;
const SUPPLY_POSITION_STATUS_OFFSET: usize = 123;

/// Refresh all emergency sources together. A flag remains set until all three are healthy;
/// a partial refresh must never clear an unrelated emergency. Anyone can call this instruction.
/// Fluid state changes do not affect marginfi health until the next successful propagation.
pub fn propagate_juplend_emergency(ctx: Context<PropagateJuplendEmergency>) -> MarginfiResult {
    let lending = ctx.accounts.lending.load()?;
    let mint = lending.mint;
    let lending_key = ctx.accounts.lending.key();
    let bank = ctx.accounts.bank.load()?;
    require!(
        bank.config.asset_tag == ASSET_TAG_JUPLEND
            && bank.config.oracle_keys[1] == lending_key
            && bank.integration_acc_1 == lending_key
            && bank.mint == mint,
        MarginfiError::JuplendEmergencyValidationFailed
    );
    require_keys_eq!(
        ctx.accounts.liquidity.key(),
        derive_juplend_liquidity().0,
        MarginfiError::JuplendEmergencyValidationFailed
    );
    require_keys_eq!(
        ctx.accounts.token_reserve.key(),
        lending.token_reserves_liquidity,
        MarginfiError::JuplendEmergencyValidationFailed
    );
    require_keys_eq!(
        ctx.accounts.supply_position.key(),
        lending.supply_position_on_liquidity,
        MarginfiError::JuplendEmergencyValidationFailed
    );
    drop(bank);
    drop(lending);

    let liquidity = ctx.accounts.liquidity.to_account_info();
    let reserve = ctx.accounts.token_reserve.to_account_info();
    let position = ctx.accounts.supply_position.to_account_info();
    // A canonical account with an unexpected owner, discriminator or layout is not evidence that
    // the venue is safe. Keep pricing available, but fail closed for Initial borrowing power.
    let healthy = [&liquidity, &reserve, &position]
        .iter()
        .all(|account| *account.owner == JUPLEND_LIQUIDITY_PROGRAM_ID)
        && match (
            liquidity.try_borrow_data(),
            reserve.try_borrow_data(),
            position.try_borrow_data(),
        ) {
            (Ok(liquidity), Ok(reserve), Ok(position)) => {
                states_are_healthy(&liquidity, &reserve, &position, &mint, &lending_key)
            }
            _ => false,
        };

    let mut bank = ctx.accounts.bank.load_mut()?;
    if healthy {
        bank.flags &= !JUPLEND_EMERGENCY;
    } else {
        bank.flags |= JUPLEND_EMERGENCY;
    }
    Ok(())
}

fn states_are_healthy(
    liquidity: &[u8],
    reserve: &[u8],
    position: &[u8],
    mint: &Pubkey,
    lending: &Pubkey,
) -> bool {
    if liquidity.len() < LIQUIDITY_STATUS_OFFSET + 2
        || liquidity[..8] != LIQUIDITY_DISCRIMINATOR
        || liquidity[LIQUIDITY_STATUS_OFFSET] != 0
        || reserve.len() < RESERVE_LEN
        || reserve[..8] != TOKEN_RESERVE_DISCRIMINATOR
        || reserve[8..40] != mint.to_bytes()
        || position.len() <= SUPPLY_POSITION_STATUS_OFFSET
        || position[..8] != SUPPLY_POSITION_DISCRIMINATOR
        || position[8..40] != lending.to_bytes()
        || position[40..72] != mint.to_bytes()
        || position[SUPPLY_POSITION_STATUS_OFFSET] != 0
    {
        return false;
    }

    // Fluid's pause_token writes its own Liquidity program ID to interacting_protocol. Ordinary
    // pre_operate calls also write this field, so nonzero alone does not mean a token is locked.
    // Verified against deployed liquidity program SHA256 f7be6791...cd4bfdf705ba632863cffafe.
    reserve[RESERVE_INTERACTING_PROTOCOL_OFFSET..RESERVE_INTERACTING_PROTOCOL_OFFSET + 32]
        != JUPLEND_LIQUIDITY_PROGRAM_ID.to_bytes()
}

#[derive(Accounts)]
pub struct PropagateJuplendEmergency<'info> {
    /// The JupLend Earn Lending state tied to the bank.
    pub lending: AccountLoader<'info, Lending>,
    /// CHECK: canonical key, owner and data validated by the handler.
    pub liquidity: UncheckedAccount<'info>,
    /// CHECK: key tied to Lending; owner and data validated by the handler.
    pub token_reserve: UncheckedAccount<'info>,
    /// CHECK: key tied to Lending; owner and data validated by the handler.
    pub supply_position: UncheckedAccount<'info>,
    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emergency_sources_are_combined_and_invalid_data_fails_closed() {
        let mint = Pubkey::new_unique();
        let lending = Pubkey::new_unique();
        let mut liquidity = vec![0; 74];
        liquidity[..8].copy_from_slice(&LIQUIDITY_DISCRIMINATOR);
        let mut reserve = vec![0; RESERVE_LEN];
        reserve[..8].copy_from_slice(&TOKEN_RESERVE_DISCRIMINATOR);
        reserve[8..40].copy_from_slice(mint.as_ref());
        let mut position = vec![0; SUPPLY_POSITION_STATUS_OFFSET + 1];
        position[..8].copy_from_slice(&SUPPLY_POSITION_DISCRIMINATOR);
        position[8..40].copy_from_slice(lending.as_ref());
        position[40..72].copy_from_slice(mint.as_ref());
        let healthy = |l: &[u8], r: &[u8], p: &[u8]| states_are_healthy(l, r, p, &mint, &lending);
        assert!(healthy(&liquidity, &reserve, &position));
        liquidity[LIQUIDITY_STATUS_OFFSET] = 1;
        assert!(!healthy(&liquidity, &reserve, &position));
        liquidity[LIQUIDITY_STATUS_OFFSET] = 0;
        reserve[RESERVE_INTERACTING_PROTOCOL_OFFSET..RESERVE_INTERACTING_PROTOCOL_OFFSET + 32]
            .copy_from_slice(JUPLEND_LIQUIDITY_PROGRAM_ID.as_ref());
        assert!(!healthy(&liquidity, &reserve, &position));
        position[SUPPLY_POSITION_STATUS_OFFSET] = 1;
        reserve[RESERVE_INTERACTING_PROTOCOL_OFFSET..RESERVE_INTERACTING_PROTOCOL_OFFSET + 32]
            .copy_from_slice(Pubkey::new_unique().as_ref());
        assert!(!healthy(&liquidity, &reserve, &position));
        position[SUPPLY_POSITION_STATUS_OFFSET] = 0;
        assert!(healthy(&liquidity, &reserve, &position));
        assert!(!healthy(&liquidity[..73], &reserve, &position));
        assert!(!healthy(&liquidity, &reserve[..191], &position));
        assert!(!healthy(&liquidity, &reserve, &position[..123]));
        position[40] ^= 1;
        assert!(!healthy(&liquidity, &reserve, &position));
    }
}
