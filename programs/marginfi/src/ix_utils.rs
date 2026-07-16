use anchor_lang::{
    prelude::*,
    solana_program::instruction::{get_stack_height, Instruction, TRANSACTION_LEVEL_STACK_HEIGHT},
};
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_sha256_hasher::{hash, hashv};

use crate::constants::{ASSOCIATED_TOKEN_KEY, COMPUTE_PROGRAM_KEY};
use crate::{check, check_eq, MarginfiError, MarginfiResult};
use marginfi_type_crate::constants::ix_discriminators as ixd;
use marginfi_type_crate::pdas::{
    DRIFT_PROGRAM_ID, JUPLEND_LENDING_PROGRAM_ID, JUPLEND_LIQUIDITY_PROGRAM_ID, KAMINO_PROGRAM_ID,
};

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
    sighash.copy_from_slice(&hash(preimage.as_bytes()).to_bytes()[..8]);
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
        let is_allowed = expected_hashes.contains(&discrim);
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

/// Tx-structure sandwich for rebalance: the end instruction must be last, start must be top-level
/// (not CPI), and only an allowlisted set of instructions may appear — the marginfi
/// rebalance/withdraw/deposit legs, plus each venue program's (non-mutating) refresh/crank ixs ONLY.
/// Forbidding the venues' deposit/borrow/withdraw ops here is what stops an attacker-keeper from
/// spiking a venue's utilization-derived supply rate inside the sandwich to pass the improvement gate
/// and farm fees.
pub fn validate_rebalance_instructions(
    sysvar: &AccountInfo,
    marginfi_account: &Pubkey,
) -> MarginfiResult {
    let allowed_programs = [
        id_crate::ID,
        COMPUTE_PROGRAM_KEY,
        KAMINO_PROGRAM_ID,
        DRIFT_PROGRAM_ID,
        JUPLEND_LENDING_PROGRAM_ID,
        JUPLEND_LIQUIDITY_PROGRAM_ID,
        ASSOCIATED_TOKEN_KEY,
        anchor_spl::token::ID,
        anchor_spl::token_2022::ID,
    ];
    let ixes = load_and_validate_instructions(sysvar, Some(&allowed_programs))?;
    validate_ix_last(&ixes, &id_crate::ID, &ixd::END_REBALANCE)?;

    // Bind every deposit/withdraw leg to the account being rebalanced. The supply-rate gate reads
    // utilization, so a leg on a FOREIGN account (the attacker's own deposit) could spike a bank's rate
    // to pass the gate and restore it before end, all with allowed discriminators. `marginfi_account`
    // is meta index 1 (group, marginfi_account, authority, ...) on every native and venue leg.
    const MOVE_LEG_DISCRIMS: [[u8; 8]; 8] = [
        ixd::LENDING_ACCOUNT_WITHDRAW,
        ixd::LENDING_ACCOUNT_DEPOSIT,
        ixd::KAMINO_WITHDRAW,
        ixd::KAMINO_DEPOSIT,
        ixd::DRIFT_WITHDRAW,
        ixd::DRIFT_DEPOSIT,
        ixd::JUPLEND_WITHDRAW,
        ixd::JUPLEND_DEPOSIT,
    ];
    for ix in ixes.iter() {
        if ix.program_id != id_crate::ID || ix.data.len() < 8 {
            continue;
        }
        let discrim = &ix.data[0..8];
        if MOVE_LEG_DISCRIMS.iter().any(|d| &d[..] == discrim) {
            check!(
                ix.accounts.len() > 1 && ix.accounts[1].pubkey == *marginfi_account,
                MarginfiError::RebalanceForeignAccountLeg
            );
        }
    }
    // Exactly one start/end pair per sandwich: a start sets ACCOUNT_IN_REBALANCE on its account and
    // only its paired end clears it, so a second start would leave another account's flag set past
    // the tx, where any signer could act on it. `validate_ix_last` pins the final ix to an end but
    // not the counts, so pin them here.
    let count_marginfi_ix = |discrim: &[u8]| {
        ixes.iter()
            .filter(|ix| {
                ix.program_id == id_crate::ID && ix.data.len() >= 8 && &ix.data[0..8] == discrim
            })
            .count()
    };
    check!(
        count_marginfi_ix(&ixd::START_REBALANCE) == 1,
        MarginfiError::RebalanceMalformedSandwich
    );
    check!(
        count_marginfi_ix(&ixd::END_REBALANCE) == 1,
        MarginfiError::RebalanceMalformedSandwich
    );
    validate_ixes_exclusive(
        &ixes,
        &id_crate::ID,
        &[
            &ixd::START_REBALANCE,
            &ixd::END_REBALANCE,
            &ixd::LENDING_ACCOUNT_WITHDRAW,
            &ixd::LENDING_ACCOUNT_DEPOSIT,
            &ixd::KAMINO_WITHDRAW,
            &ixd::KAMINO_DEPOSIT,
            &ixd::DRIFT_WITHDRAW,
            &ixd::DRIFT_DEPOSIT,
            &ixd::JUPLEND_WITHDRAW,
            &ixd::JUPLEND_DEPOSIT,
        ],
    )?;
    // Venue programs may appear ONLY as their refresh/crank ixs (which recompute at current
    // utilization without changing it). Any other venue ix — deposit/borrow/withdraw — is rejected,
    // closing the in-sandwich rate-manipulation path.
    validate_ixes_exclusive(
        &ixes,
        &KAMINO_PROGRAM_ID,
        &[
            &ixd::KAMINO_REFRESH_RESERVE,
            &ixd::KAMINO_REFRESH_OBLIGATION,
        ],
    )?;
    validate_ixes_exclusive(
        &ixes,
        &DRIFT_PROGRAM_ID,
        &[&ixd::DRIFT_UPDATE_SPOT_MARKET_CUMULATIVE_INTEREST],
    )?;
    // The supply-rate gate reads the Fluid `TokenReserve`, which is refreshed by the LIQUIDITY
    // program's `update_exchange_price`; the LENDING program's `update_rate` refreshes the separate
    // `Lending` account. Allow each crank only on its own program.
    validate_ixes_exclusive(
        &ixes,
        &JUPLEND_LIQUIDITY_PROGRAM_ID,
        &[&ixd::JUPLEND_UPDATE_EXCHANGE_PRICE],
    )?;
    validate_ixes_exclusive(
        &ixes,
        &JUPLEND_LENDING_PROGRAM_ID,
        &[&ixd::JUPLEND_UPDATE_RATE],
    )?;
    validate_not_cpi_by_stack_height()
}

/// Finds the hash of a slice of keys, sorting them before hashing
pub fn keys_sha256_hash(keys: &[Pubkey]) -> [u8; 32] {
    let mut slices: Vec<&[u8]> = keys.iter().map(|pk| pk.as_ref()).collect();
    slices.sort_unstable();
    hashv(&slices).to_bytes()
}

// TODO eventually compare these against the generated discrim in the IDL to prevent sausage fingers
// from changing an ix name and thusly the hash.
#[cfg(test)]
mod tests {
    use marginfi_type_crate::constants::{discriminators, ix_discriminators};
    use pretty_assertions::assert_eq;

    use crate::{
        DriftWithdraw, EndDeleverage, EndExecuteOrder, EndLiquidation, InitLiquidationRecord,
        KaminoWithdraw, LendingAccountEndFlashloan, LendingAccountRepay,
        LendingAccountStartFlashloan, LendingAccountWithdraw, StartDeleverage, StartExecuteOrder,
        StartLiquidation,
    };

    use super::*;

    #[test]
    fn check_struct_discrims_generated() {
        let got_bank = get_discrim_hash("account", "Bank");
        let want_bank = discriminators::BANK;
        assert_eq!(got_bank, want_bank);

        let got_group = get_discrim_hash("account", "MarginfiGroup");
        let want_group = discriminators::GROUP;
        assert_eq!(got_group, want_group);

        let got_account = get_discrim_hash("account", "MarginfiAccount");
        let want_account = discriminators::ACCOUNT;
        assert_eq!(got_account, want_account);

        let got_fee_state = get_discrim_hash("account", "FeeState");
        let want_fee_state = discriminators::FEE_STATE;
        assert_eq!(got_fee_state, want_fee_state);

        let got_staked = get_discrim_hash("account", "StakedSettings");
        let want_staked = discriminators::STAKED_SETTINGS;
        assert_eq!(got_staked, want_staked);

        let got_liquidation = get_discrim_hash("account", "LiquidationRecord");
        let want_liquidation = discriminators::LIQUIDATION_RECORD;
        assert_eq!(got_liquidation, want_liquidation);

        let got_order = get_discrim_hash("account", "Order");
        let want_order = discriminators::ORDER;
        assert_eq!(got_order, want_order);

        let got_exec_record = get_discrim_hash("account", "ExecuteOrderRecord");
        let want_exec_record = discriminators::EXECUTE_ORDER_RECORD;
        assert_eq!(got_exec_record, want_exec_record);
    }

    #[test]
    fn check_instruction_hash_generated() {
        let got_init = InitLiquidationRecord::get_hash();
        let want_init = ix_discriminators::INIT_LIQUIDATION_RECORD;
        assert_eq!(got_init, want_init);

        let got_start = StartLiquidation::get_hash();
        let want_start = ix_discriminators::START_LIQUIDATION;
        assert_eq!(got_start, want_start);

        let got_end = EndLiquidation::get_hash();
        let want_end = ix_discriminators::END_LIQUIDATION;
        assert_eq!(got_end, want_end);

        let got_withdraw = LendingAccountWithdraw::get_hash();
        let want_withdraw = ix_discriminators::LENDING_ACCOUNT_WITHDRAW;
        assert_eq!(got_withdraw, want_withdraw);

        let got_repay = LendingAccountRepay::get_hash();
        let want_repay = ix_discriminators::LENDING_ACCOUNT_REPAY;
        assert_eq!(got_repay, want_repay);

        let got_flash = LendingAccountStartFlashloan::get_hash();
        let want_flash = ix_discriminators::START_FLASHLOAN;
        assert_eq!(got_flash, want_flash);

        let got_flash = LendingAccountEndFlashloan::get_hash();
        let want_flash = ix_discriminators::END_FLASHLOAN;
        assert_eq!(got_flash, want_flash);

        let got_start = StartDeleverage::get_hash();
        let want_start = ix_discriminators::START_DELEVERAGE;
        assert_eq!(got_start, want_start);

        let got_end = EndDeleverage::get_hash();
        let want_end = ix_discriminators::END_DELEVERAGE;
        assert_eq!(got_end, want_end);

        let got_drift = DriftWithdraw::get_hash();
        let want_drift = ix_discriminators::DRIFT_WITHDRAW;
        assert_eq!(got_drift, want_drift);

        let got_kamino = KaminoWithdraw::get_hash();
        let want_kamino = ix_discriminators::KAMINO_WITHDRAW;
        assert_eq!(got_kamino, want_kamino);

        let got_start_exec = StartExecuteOrder::get_hash();
        let want_start_exec = ix_discriminators::START_EXECUTE_ORDER;
        assert_eq!(got_start_exec, want_start_exec);

        let got_end_exec = EndExecuteOrder::get_hash();
        let want_end_exec = ix_discriminators::END_EXECUTE_ORDER;
        assert_eq!(got_end_exec, want_end_exec);
    }

    #[test]
    fn venue_crank_discrims_match_anchor() {
        // The foreign venue crank discriminators must equal the standard anchor derivation.
        assert_eq!(
            get_discrim_hash("global", "refresh_reserve"),
            ix_discriminators::KAMINO_REFRESH_RESERVE
        );
        assert_eq!(
            get_discrim_hash("global", "refresh_obligation"),
            ix_discriminators::KAMINO_REFRESH_OBLIGATION
        );
        assert_eq!(
            get_discrim_hash("global", "update_spot_market_cumulative_interest"),
            ix_discriminators::DRIFT_UPDATE_SPOT_MARKET_CUMULATIVE_INTEREST
        );
        assert_eq!(
            get_discrim_hash("global", "update_rate"),
            ix_discriminators::JUPLEND_UPDATE_RATE
        );
        assert_eq!(
            get_discrim_hash("global", "update_exchange_price"),
            ix_discriminators::JUPLEND_UPDATE_EXCHANGE_PRICE
        );
    }

    #[test]
    fn rebalance_allowlist_rejects_venue_mutations() {
        let kamino_ix = |discrim: [u8; 8]| Instruction {
            program_id: KAMINO_PROGRAM_ID,
            accounts: vec![],
            data: discrim.to_vec(),
        };
        let cranks: &[&[u8]] = &[
            &ix_discriminators::KAMINO_REFRESH_RESERVE,
            &ix_discriminators::KAMINO_REFRESH_OBLIGATION,
        ];
        // A refresh crank is permitted...
        assert!(validate_ixes_exclusive(
            &[kamino_ix(ix_discriminators::KAMINO_REFRESH_RESERVE)],
            &KAMINO_PROGRAM_ID,
            cranks,
        )
        .is_ok());
        // ...but a rate-manipulating Kamino deposit is rejected.
        assert!(validate_ixes_exclusive(
            &[
                kamino_ix(ix_discriminators::KAMINO_REFRESH_RESERVE),
                kamino_ix(ix_discriminators::KAMINO_DEPOSIT),
            ],
            &KAMINO_PROGRAM_ID,
            cranks,
        )
        .is_err());
    }
}
