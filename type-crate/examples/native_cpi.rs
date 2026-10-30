//! Minimal instruction construction and CPI wiring for a program that does not depend on Anchor.

use marginfi_type_crate::ix_builders::{self, lending::LendingAccountDeposit};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::AccountMeta;
use solana_program_error::ProgramError;

/// Invokes `lending_account_deposit` with fixed accounts and an optional Token-2022 mint.
///
/// `fixed_account_infos` must match [`LendingAccountDeposit`] field order. If the mint has a
/// transfer hook, append its resolved metas to the instruction and its matching `AccountInfo`
/// values to `account_infos` after the mint and before the marginfi program account.
pub fn cpi_deposit<'a>(
    accounts: &LendingAccountDeposit,
    amount: u64,
    fixed_account_infos: &[AccountInfo<'a>],
    marginfi_program: AccountInfo<'a>,
    token_2022_mint: Option<(AccountMeta, AccountInfo<'a>)>,
    signer_seeds: &[&[&[u8]]],
) -> Result<(), ProgramError> {
    let mut ix = ix_builders::lending::lending_account_deposit(accounts, amount, None);
    let mut account_infos = fixed_account_infos.to_vec();

    if let Some((mint_meta, mint_info)) = token_2022_mint {
        ix.accounts.push(mint_meta);
        account_infos.push(mint_info);
    }

    account_infos.push(marginfi_program);
    invoke_signed(&ix, &account_infos, signer_seeds)
}

fn main() {}
