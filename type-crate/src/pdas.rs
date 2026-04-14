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

pub fn derive_kamino_reserve_liquidity_supply(reserve: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"reserve_liq_supply", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    )
}

pub fn derive_kamino_reserve_collateral_mint(reserve: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"reserve_coll_mint", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    )
}

pub fn derive_kamino_reserve_collateral_supply(reserve: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"reserve_coll_supply", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    )
}

pub fn derive_kamino_user_metadata(owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"user_meta", owner.as_ref()], &KAMINO_PROGRAM_ID)
}

pub fn derive_kamino_obligation(
    owner: &Pubkey,
    lending_market: &Pubkey,
    seed1_account: &Pubkey,
    seed2_account: &Pubkey,
    tag: u8,
    id: u8,
) -> (Pubkey, u8) {
    let tag_seed = [tag];
    let id_seed = [id];

    Pubkey::find_program_address(
        &[
            tag_seed.as_ref(),
            id_seed.as_ref(),
            owner.as_ref(),
            lending_market.as_ref(),
            seed1_account.as_ref(),
            seed2_account.as_ref(),
        ],
        &KAMINO_PROGRAM_ID,
    )
}

pub fn derive_kamino_base_obligation(owner: &Pubkey, lending_market: &Pubkey) -> (Pubkey, u8) {
    derive_kamino_obligation(
        owner,
        lending_market,
        &anchor_lang::solana_program::system_program::ID,
        &anchor_lang::solana_program::system_program::ID,
        0,
        0,
    )
}

pub fn derive_kamino_farm_vaults_authority(farm_state: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"authority", farm_state.as_ref()], &FARMS_PROGRAM_ID)
}

pub fn derive_kamino_rewards_vault(farm_state: &Pubkey, reward_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"rvault", farm_state.as_ref(), reward_mint.as_ref()],
        &FARMS_PROGRAM_ID,
    )
}

pub fn derive_kamino_rewards_treasury_vault(
    global_config: &Pubkey,
    reward_mint: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"tvault", global_config.as_ref(), reward_mint.as_ref()],
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
