//! Instruction data for the marginfi CPI calls used by the mocks program.
//!
//! The caller supplies account metas and the target program ID. Keeping these
//! encodings independent of the program crate lets mocks be built separately.

use crate::constants::ix_discriminators::native::{
    LENDING_POOL_HANDLE_BANKRUPTCY, MARGINFI_ACCOUNT_INITIALIZE_PDA, START_LIQUIDATION,
};

/// Instruction data for `start_liquidation`, which takes no arguments.
pub fn start_liquidation_data() -> Vec<u8> {
    START_LIQUIDATION.to_vec()
}

/// Instruction data for `lending_pool_handle_bankruptcy`, which takes no arguments.
pub fn lending_pool_handle_bankruptcy_data() -> Vec<u8> {
    LENDING_POOL_HANDLE_BANKRUPTCY.to_vec()
}

/// Instruction data for `marginfi_account_initialize_pda`.
pub fn marginfi_account_initialize_pda_data(
    account_index: u16,
    third_party_id: Option<u16>,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(13);
    data.extend_from_slice(&MARGINFI_ACCOUNT_INITIALIZE_PDA);
    data.extend_from_slice(&account_index.to_le_bytes());
    match third_party_id {
        None => data.push(0),
        Some(id) => {
            data.push(1);
            data.extend_from_slice(&id.to_le_bytes());
        }
    }
    data
}
