use anchor_lang::prelude::*;

pub const KAMINO_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
pub const DRIFT_PROGRAM_ID: Pubkey = pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

pub fn derive_kamino_lending_market_authority(lending_market: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"lma", lending_market.as_ref()], &KAMINO_PROGRAM_ID)
}

pub fn derive_kamino_user_state(farm_state: &Pubkey, obligation: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"user", farm_state.as_ref(), obligation.as_ref()],
        &FARMS_PROGRAM_ID,
    )
}

pub fn derive_drift_state() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"drift_state"], &DRIFT_PROGRAM_ID)
}

pub fn derive_drift_spot_market(market_index: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"spot_market", &market_index.to_le_bytes()],
        &DRIFT_PROGRAM_ID,
    )
}

pub fn derive_drift_spot_market_vault(market_index: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"spot_market_vault", &market_index.to_le_bytes()],
        &DRIFT_PROGRAM_ID,
    )
}

pub fn derive_drift_signer() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"drift_signer"], &DRIFT_PROGRAM_ID)
}

pub fn derive_drift_insurance_fund_vault(market_index: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"insurance_fund_vault", &market_index.to_le_bytes()],
        &DRIFT_PROGRAM_ID,
    )
}

pub fn derive_drift_user(authority: &Pubkey, user_index: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"user", authority.as_ref(), &user_index.to_le_bytes()],
        &DRIFT_PROGRAM_ID,
    )
}

pub fn derive_drift_user_stats(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"user_stats", authority.as_ref()], &DRIFT_PROGRAM_ID)
}
