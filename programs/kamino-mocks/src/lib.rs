pub mod macros;
pub mod state;

use anchor_lang::{
    prelude::*,
    solana_program::{
        entrypoint::ProgramResult,
        hash::hash,
        instruction::{AccountMeta, Instruction},
        program::invoke,
        program_error::ProgramError,
    },
};

declare_id!(marginfi_type_crate::pdas::KAMINO_PROGRAM_ID);

declare_program!(kamino_lending);
declare_program!(kamino_farms);

#[program]
pub mod kamino_mocks {}

#[error_code]
pub enum KaminoMocksError {
    #[msg("Math error")]
    MathError,
}

/// Custom mock-kamino instruction payload that triggers a CPI into
/// marginfi::lending_account_close_balance.
pub const CPI_CLOSE_BALANCE_IX_DATA: [u8; 8] = *b"CPICLSBL";

fn lending_account_close_balance_discriminator() -> [u8; 8] {
    let mut sighash = [0u8; 8];
    sighash
        .copy_from_slice(&hash("global:lending_account_close_balance".as_bytes()).to_bytes()[..8]);
    sighash
}

fn process_cpi_close_balance(accounts: &[AccountInfo]) -> ProgramResult {
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let group_ai = &accounts[0];
    let marginfi_account_ai = &accounts[1];
    let authority_ai = &accounts[2];
    let bank_ai = &accounts[3];
    let marginfi_program_ai = &accounts[4];

    let ix = Instruction {
        program_id: *marginfi_program_ai.key,
        accounts: vec![
            AccountMeta::new_readonly(*group_ai.key, false),
            AccountMeta::new(*marginfi_account_ai.key, false),
            AccountMeta::new_readonly(*authority_ai.key, true),
            AccountMeta::new(*bank_ai.key, false),
        ],
        data: lending_account_close_balance_discriminator().to_vec(),
    };

    invoke(
        &ix,
        &[
            group_ai.clone(),
            marginfi_account_ai.clone(),
            authority_ai.clone(),
            bank_ai.clone(),
            marginfi_program_ai.clone(),
        ],
    )
}

// A lightweight mock that accepts any ix and returns Ok(()).
// Used in Rust tests.
pub fn mock_kamino_lending_processor(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    ix_data: &[u8],
) -> ProgramResult {
    if ix_data == CPI_CLOSE_BALANCE_IX_DATA {
        return process_cpi_close_balance(accounts);
    }

    Ok(())
}
