//! Creates a marginfi account owned by a PDA of this program, entirely through
//! `marginfi_type_crate::ix_builders`. No anchor anywhere in the dependency graph.

use marginfi_type_crate::ix_builders::account::{
    marginfi_account_initialize, MarginfiAccountInitialize,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_program_error::ProgramError;
use solana_pubkey::{declare_id, Pubkey};

declare_id!("NatCP1EXampLe111111111111111111111111111111");

/// Seed for the PDA that owns the created marginfi account.
pub const AUTHORITY_SEED: &[u8] = b"authority";

pub fn authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[AUTHORITY_SEED], &ID)
}

/// Accounts: `[group, marginfi_account (signer), authority_pda, fee_payer (signer),
/// system_program, marginfi_program]`.
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> Result<(), ProgramError> {
    let [group, marginfi_account, authority, fee_payer, system_program, _marginfi_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let (expected_authority, bump) = authority_pda();
    if *authority.key != expected_authority {
        return Err(ProgramError::InvalidSeeds);
    }

    let ix = marginfi_account_initialize(&MarginfiAccountInitialize {
        marginfi_group: *group.key,
        marginfi_account: *marginfi_account.key,
        authority: expected_authority,
        fee_payer: *fee_payer.key,
        system_program: *system_program.key,
    });

    invoke_signed(
        &ix,
        &[
            group.clone(),
            marginfi_account.clone(),
            authority.clone(),
            fee_payer.clone(),
            system_program.clone(),
        ],
        &[&[AUTHORITY_SEED, &[bump]]],
    )
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program_entrypoint::entrypoint!(entry);

#[cfg(not(feature = "no-entrypoint"))]
fn entry(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> Result<(), ProgramError> {
    process_instruction(program_id, accounts, data)
}
