use anchor_lang::prelude::*;

use crate::errors::ErrorCodes;

pub const RATE_OUTPUT_DECIMALS: u32 = oracle::constants::RATE_OUTPUT_DECIMALS;

pub struct OracleCpiAccounts<'info> {
    pub oracle_program: AccountInfo<'info>,
    pub oracle: AccountInfo<'info>,
    pub remaining_accounts: Vec<AccountInfo<'info>>,
}

/// Specifies which oracle CPI call to use.
pub enum OracleCpiTarget {
    Both,
    Operate,
    Liquidate,
}

impl<'info> OracleCpiAccounts<'info> {
    fn get_exchange_rate(&self, nonce: u16, call: OracleCpiTarget) -> Result<(u128, u128)> {
        let cpi_program: AccountInfo<'info> = self.oracle_program.to_account_info();
        let cpi_accounts = oracle::cpi::accounts::GetExchangeRate {
            oracle: self.oracle.clone(),
        };
        let remaining_accounts = self.remaining_accounts.clone();

        // Create the CPI context with signer seeds
        let cpi_ctx: CpiContext<'_, '_, '_, '_, oracle::cpi::accounts::GetExchangeRate<'_>> =
            CpiContext::new(cpi_program, cpi_accounts).with_remaining_accounts(remaining_accounts);

        // https://github.com/solana-foundation/anchor/blob/master/tests/cpi-returns/programs/caller/src/lib.rs#L25
        match call {
            OracleCpiTarget::Liquidate => {
                match oracle::cpi::get_exchange_rate_liquidate(cpi_ctx, nonce) {
                    Ok(result) => Ok((result.get(), 0)),
                    Err(_) => err!(ErrorCodes::VaultCpiToOracleFailed),
                }
            }
            OracleCpiTarget::Operate => {
                match oracle::cpi::get_exchange_rate_operate(cpi_ctx, nonce) {
                    Ok(result) =>  Ok((0, result.get())),
                    Err(_) => err!(ErrorCodes::VaultCpiToOracleFailed),
                }
            }
            OracleCpiTarget::Both => {
                match oracle::cpi::get_both_exchange_rate(cpi_ctx, nonce) {
                    Ok(result) => Ok(result.get()),
                    Err(_) => err!(ErrorCodes::VaultCpiToOracleFailed),
                }
            }
        }
    }

    pub fn get_exchange_rate_operate(&self, nonce: u16) -> Result<u128> {
        Ok(self.get_exchange_rate(nonce, OracleCpiTarget::Operate)?.1)
    }

    pub fn get_exchange_rate_liquidate(&self, nonce: u16) -> Result<u128> {
        Ok(self.get_exchange_rate(nonce, OracleCpiTarget::Liquidate)?.0)
    }

    pub fn get_both_exchange_rate(&self, nonce: u16) -> Result<(u128, u128)> {
        self.get_exchange_rate(nonce, OracleCpiTarget::Both)
    }
}
