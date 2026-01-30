use anchor_lang::solana_program::sysvar::instructions::*;
use anchor_lang::{
    prelude::*,
    solana_program::instruction::{get_stack_height, Instruction, TRANSACTION_LEVEL_STACK_HEIGHT},
};

use crate::constants::COMPUTE_PROGRAM_KEY;
use crate::{check, check_eq, MarginfiError, MarginfiResult};

/// Structs that implement this trait have a `get_hash` tool that returns the function discriminator
pub trait Hashable {
    fn get_hash() -> [u8; 8];
}

/// The function of struct discriminator is constructed from these 8 bytes. Typically, the namespace  
/// is "account" or "state". For instructions it's typically "global".
///
/// e.g. for LiquidateStart:
/// ```
///  let discrim = get_function_hash("global", "liquidate_start")
/// ```
pub fn get_discrim_hash(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{}:{}", namespace, name);
    let mut sighash = [0u8; 8];
    sighash.copy_from_slice(
        &anchor_lang::solana_program::hash::hash(preimage.as_bytes()).to_bytes()[..8],
    );
    sighash
}

/// Validate the given ix hash is the first in the list of ixes, other than compute budget ixes
/// and explicitly allowed ixes (e.g. Kamino refreshes), and appears only once.
///
/// If the ix implements `Hashable`, use `get_hash()` to get the expected hash.
pub fn validate_ix_first(
    ixes: &[Instruction],
    program_id: &Pubkey,
    expected_hash: &[u8],
    allowed_ixs: &[(Pubkey, &[u8])],
) -> MarginfiResult<()> {
    let compute_budget_key = COMPUTE_PROGRAM_KEY;
    let mut expected_ix_encountered = false;

    for instruction in ixes.iter() {
        if instruction.program_id == compute_budget_key {
            continue;
        }

        // Sanity check the instruction is valid
        if instruction.data.len() < 8 {
            // Note: non-anchor programs (e.g. ComputeBudget) can end up in this path.
            return err!(MarginfiError::StartNotFirst);
        }
        let discrim = &instruction.data[0..8];

        // If this is the first non-allowed ix, it is either the start ix, or we fail
        if !expected_ix_encountered {
            if instruction.program_id == *program_id && discrim == expected_hash {
                expected_ix_encountered = true;
            } else if !allowed_ixs.contains(&(instruction.program_id, discrim)) {
                msg!(
                    "Expected ix from program: {:?} w/ hash: {:?}",
                    program_id,
                    expected_hash
                );
                msg!(
                    "Got ix from program: {:?} w/ hash: {:?}",
                    instruction.program_id,
                    instruction.data,
                );
                msg!("Start IX was not the first ix (other than allowed).");
                return err!(MarginfiError::StartNotFirst);
            }
        } else {
            // Already validated the first ix, check that the start ix does not appear again
            if instruction.program_id == *program_id && discrim == expected_hash {
                msg!("Setup IX appears more than once.");
                return err!(MarginfiError::StartRepeats);
            }
        }
    }

    if expected_ix_encountered {
        Ok(())
    } else {
        msg!("Start IX was not found in the TX.");
        err!(MarginfiError::StartNotFirst)
    }
}

/// Validate the given ix hash is the last ix in the list of ixes.
pub fn validate_ix_last(
    ixes: &[Instruction],
    program_id: &Pubkey,
    expected_hash: &[u8],
) -> MarginfiResult<()> {
    let last_ix = ixes.last().unwrap(); // Safe unwrap, ixes.size() >= 1

    // Sanity check the instruction is valid
    if last_ix.data.len() < 8 {
        // Note: non-anchor programs (e.g. ComputeBudget) can end up in this path.
        return err!(MarginfiError::EndNotLast);
    }
    let discrim = &last_ix.data[0..8];

    check!(last_ix.program_id.eq(program_id), MarginfiError::EndNotLast);
    check_eq!(discrim, expected_hash, MarginfiError::EndNotLast);
    Ok(())
}

// We could probably compress this into one chained iter().any(), but it would unreadable
/// Validate that no other ixes from the given program appear, other than those specified in the
/// allowlist of `expected_hashes`
pub fn validate_ixes_exclusive(
    ixes: &[Instruction],
    program_id: &Pubkey,
    expected_hashes: &[&[u8]],
) -> MarginfiResult<()> {
    // Loop over instructions just in the given program
    for ix in ixes.iter().filter(|ix| &ix.program_id == program_id) {
        // Sanity check the instruction is valid
        if ix.data.len() < 8 {
            // We expect only Anchor programs to be inputs here, so this should be impossible.
            panic!("malformed instruction");
        }
        let discrim = &ix.data[0..8];

        // If none of the allowed hashes match, reject
        let is_allowed = expected_hashes.iter().any(|&h| h == discrim);
        if !is_allowed {
            msg!("Forbidden ix discrim: {:?}", discrim);
            return err!(MarginfiError::ForbiddenIx);
        }
    }
    Ok(())
}

/*** We might use these later for something like to limit the swap venue to e.g. Jup */

/// Errors if the top-level relative instruction is not the Marginfi program, returns the index of
/// the current ix otherwise
pub fn validate_not_cpi_with_sysvar(sysvar: &AccountInfo) -> MarginfiResult<usize> {
    let mrgn_key = id_crate::ID;
    let current_index: usize = load_current_index_checked(sysvar)?.into();
    let current_ix = load_instruction_at_checked(current_index, sysvar)?;

    if current_ix.program_id != mrgn_key {
        err!(MarginfiError::NotAllowedInCPI)
    } else {
        Ok(current_index)
    }
}

/// Errors if this the instruction calling this is within a CPI as defined by stack height > 1
pub fn validate_not_cpi_by_stack_height() -> MarginfiResult<()> {
    check!(
        get_stack_height() == TRANSACTION_LEVEL_STACK_HEIGHT,
        MarginfiError::NotAllowedInCPI
    );
    Ok(())
}

/// The instruction uses one of the given hard-coded allowed programs.
pub fn validate_program_allowed(
    instruction: &Instruction,
    allowed_keys: &[Pubkey],
) -> MarginfiResult<()> {
    let id = &instruction.program_id;
    if !allowed_keys.iter().any(|key| key.eq(id)) {
        msg!("Forbidden ix program: {:?}", id);
        return err!(MarginfiError::ForbiddenIx);
    }
    Ok(())
}

/// Validate that all instructions in the tx belong to an allowed program key. Returns a Vec of ixes
/// in the same order that they appear in the tx.
///
/// * allowed_keys - Pass None to allow instructions from any program, or pass an array of keys to
///   validate only those programs occur in this tx.
pub fn load_and_validate_instructions(
    sysvar: &AccountInfo,
    allowed_keys: Option<&[Pubkey]>,
) -> Result<Vec<Instruction>> {
    let mut ixes: Vec<Instruction> = Vec::new();
    let mut ix_count = 0;
    loop {
        match load_instruction_at_checked(ix_count, sysvar) {
            Ok(instruction) => {
                if let Some(keys) = allowed_keys {
                    validate_program_allowed(&instruction, keys)?;
                }
                ixes.push(instruction);
                ix_count += 1;
            }
            Err(ProgramError::InvalidArgument) => break, // Passed last ix, stop loop
            Err(e) => {
                msg!("unexpected error {:?}", e);
                panic!("Error reading some ix");
            }
        }
    }
    Ok(ixes)
}

// TODO eventually compare these against the generated discrim in the IDL to prevent sausage fingers
// from changing an ix name and thusly the hash.
#[cfg(test)]
mod tests {
    use marginfi_type_crate::constants::{discriminators, ix_discriminators};
    use pretty_assertions::assert_eq;

    use crate::{
        DriftWithdraw, EndDeleverage, EndLiquidation, InitLiquidationRecord, KaminoWithdraw,
        LendingAccountEndFlashloan, LendingAccountRepay, LendingAccountSettleEmissions,
        LendingAccountStartFlashloan, LendingAccountWithdraw, LendingAccountWithdrawEmissions,
        StartDeleverage, StartLiquidation,
    };

    use super::*;

    #[test]
    fn check_struct_discrims_generated() {
        // ─── Bank ──────────────────────────────────────────────────────────────────
        let got_bank = get_discrim_hash("account", "Bank");
        let want_bank = discriminators::BANK;
        assert_eq!(got_bank, want_bank);

        // ─── MarginfiGroup ─────────────────────────────────────────────────────────
        let got_group = get_discrim_hash("account", "MarginfiGroup");
        let want_group = discriminators::GROUP;
        assert_eq!(got_group, want_group);

        // ─── MarginfiAccount ───────────────────────────────────────────────────────
        let got_account = get_discrim_hash("account", "MarginfiAccount");
        let want_account = discriminators::ACCOUNT;
        assert_eq!(got_account, want_account);

        // ─── FeeState ──────────────────────────────────────────────────────────────
        let got_fee_state = get_discrim_hash("account", "FeeState");
        let want_fee_state = discriminators::FEE_STATE;
        assert_eq!(got_fee_state, want_fee_state);

        // ─── StakedSettings ─────────────────────────────────────────────────────────
        let got_staked = get_discrim_hash("account", "StakedSettings");
        let want_staked = discriminators::STAKED_SETTINGS;
        assert_eq!(got_staked, want_staked);

        // ─── LiquidationRecord ─────────────────────────────────────────────────────
        let got_liquidation = get_discrim_hash("account", "LiquidationRecord");
        let want_liquidation = discriminators::LIQUIDATION_RECORD;
        assert_eq!(got_liquidation, want_liquidation);
    }

    #[test]
    fn check_instruction_hash_generated() {
        // ─── InitLiquidationRecord ───────────────────────────────────────────────
        let got_init = InitLiquidationRecord::get_hash();
        let want_init = ix_discriminators::INIT_LIQUIDATION_RECORD;
        assert_eq!(got_init, want_init);

        // ─── StartLiquidation ────────────────────────────────────────────────────
        let got_start = StartLiquidation::get_hash();
        let want_start = ix_discriminators::START_LIQUIDATION;
        assert_eq!(got_start, want_start);

        // ─── EndLiquidation ──────────────────────────────────────────────────────
        let got_end = EndLiquidation::get_hash();
        let want_end = ix_discriminators::END_LIQUIDATION;
        assert_eq!(got_end, want_end);

        // ─── LendingAccountWithdraw ──────────────────────────────────────────────
        let got_withdraw = LendingAccountWithdraw::get_hash();
        let want_withdraw = ix_discriminators::LENDING_ACCOUNT_WITHDRAW;
        assert_eq!(got_withdraw, want_withdraw);

        // ─── LendingAccountRepay ─────────────────────────────────────────────────
        let got_repay = LendingAccountRepay::get_hash();
        let want_repay = ix_discriminators::LENDING_ACCOUNT_REPAY;
        assert_eq!(got_repay, want_repay);

        // ─── LendingAccountWithdrawEmissions ─────────────────────────────────────────────────
        let got_repay = LendingAccountWithdrawEmissions::get_hash();
        let want_repay = ix_discriminators::LENDING_SETTLE_EMISSIONS;
        assert_eq!(got_repay, want_repay);

        // ─── LendingAccountSettleEmissions ─────────────────────────────────────────────────
        let got_repay = LendingAccountSettleEmissions::get_hash();
        let want_repay = ix_discriminators::LENDING_WITHDRAW_EMISSIONS;
        assert_eq!(got_repay, want_repay);

        // ─── LendingAccountStartFlashloan ─────────────────────────────────────────────────
        let got_flash = LendingAccountStartFlashloan::get_hash();
        let want_flash = ix_discriminators::START_FLASHLOAN;
        assert_eq!(got_flash, want_flash);

        // ─── LendingAccountEndFlashloan ─────────────────────────────────────────────────
        let got_flash = LendingAccountEndFlashloan::get_hash();
        let want_flash = ix_discriminators::END_FLASHLOAN;
        assert_eq!(got_flash, want_flash);

        // ─── StartDeleverage ────────────────────────────────────────────────────
        let got_start = StartDeleverage::get_hash();
        let want_start = ix_discriminators::START_DELEVERAGE;
        assert_eq!(got_start, want_start);

        // ─── EndDeleverage ──────────────────────────────────────────────────────
        let got_end = EndDeleverage::get_hash();
        let want_end = ix_discriminators::END_DELEVERAGE;
        assert_eq!(got_end, want_end);

        // ─── DriftWithdraw ─────────────────────────────────────────────────────
        let got_drift = DriftWithdraw::get_hash();
        let want_drift = ix_discriminators::DRIFT_WITHDRAW;
        assert_eq!(got_drift, want_drift);

        // ─── KaminoWithdraw ────────────────────────────────────────────────────
        let got_kamino = KaminoWithdraw::get_hash();
        let want_kamino = ix_discriminators::KAMINO_WITHDRAW;
        assert_eq!(got_kamino, want_kamino);
    }
}
