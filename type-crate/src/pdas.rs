use crate::constants::{DRIFT_USER_SEED, DRIFT_USER_STATS_SEED, JUPLEND_F_TOKEN_VAULT_SEED};
use crate::types::{Bank, BankVaultType, OracleSetup};
use solana_pubkey::{pubkey, Pubkey};

pub const KAMINO_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
pub const DRIFT_PROGRAM_ID: Pubkey = pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");
pub const JUPLEND_LENDING_PROGRAM_ID: Pubkey =
    pubkey!("jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9");
pub const JUPLEND_LIQUIDITY_PROGRAM_ID: Pubkey =
    pubkey!("jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC");
pub const JUPLEND_REWARDS_PROGRAM_ID: Pubkey =
    pubkey!("jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar");
pub const SPL_SINGLE_POOL_PROGRAM_ID: Pubkey =
    pubkey!("SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE");
const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

pub fn get_associated_token_address_with_program_id(
    wallet: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Pubkey {
    Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

/// Derive the SPL single-pool PDA chain from a validator vote account:
/// `vote_account -> stake_pool -> (lst_mint, sol_pool, pool_onramp)`.
pub fn derive_single_pool_keys_from_vote(
    validator_vote_account: Pubkey,
) -> (Pubkey, Pubkey, Pubkey, Pubkey) {
    let vote_account_bytes = validator_vote_account.to_bytes();
    let (stake_pool, _) =
        Pubkey::find_program_address(&[b"pool", &vote_account_bytes], &SPL_SINGLE_POOL_PROGRAM_ID);

    let stake_pool_bytes = stake_pool.to_bytes();
    let (lst_mint, _) =
        Pubkey::find_program_address(&[b"mint", &stake_pool_bytes], &SPL_SINGLE_POOL_PROGRAM_ID);
    let (sol_pool, _) =
        Pubkey::find_program_address(&[b"stake", &stake_pool_bytes], &SPL_SINGLE_POOL_PROGRAM_ID);
    let (pool_onramp, _) =
        Pubkey::find_program_address(&[b"onramp", &stake_pool_bytes], &SPL_SINGLE_POOL_PROGRAM_ID);

    (stake_pool, lst_mint, sol_pool, pool_onramp)
}

pub fn derive_staked_onramp_from_vote(validator_vote_account: Pubkey) -> Pubkey {
    let (_stake_pool, _lst_mint, _sol_pool, pool_onramp) =
        derive_single_pool_keys_from_vote(validator_vote_account);
    pool_onramp
}

pub fn derive_juplend_lending_admin() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"lending_admin"], &JUPLEND_LENDING_PROGRAM_ID)
}

pub fn derive_juplend_liquidity() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"liquidity"], &JUPLEND_LIQUIDITY_PROGRAM_ID)
}

pub fn derive_juplend_auth_list() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"auth_list"], &JUPLEND_LIQUIDITY_PROGRAM_ID)
}

pub fn derive_juplend_token_reserve(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"reserve", mint.as_ref()], &JUPLEND_LIQUIDITY_PROGRAM_ID)
}

pub fn derive_juplend_rate_model(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"rate_model", mint.as_ref()],
        &JUPLEND_LIQUIDITY_PROGRAM_ID,
    )
}

pub fn derive_juplend_liquidity_vault(mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    let (liquidity, _) = derive_juplend_liquidity();
    get_associated_token_address_with_program_id(&liquidity, mint, token_program)
}

pub fn derive_juplend_f_token_mint(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"f_token_mint", mint.as_ref()],
        &JUPLEND_LENDING_PROGRAM_ID,
    )
}

pub fn derive_juplend_lending(mint: &Pubkey, f_token_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"lending", mint.as_ref(), f_token_mint.as_ref()],
        &JUPLEND_LENDING_PROGRAM_ID,
    )
}

pub fn derive_juplend_lending_from_mint(mint: &Pubkey) -> (Pubkey, Pubkey) {
    let (f_token_mint, _) = derive_juplend_f_token_mint(mint);
    let (lending, _) = derive_juplend_lending(mint, &f_token_mint);
    (lending, f_token_mint)
}

pub fn derive_juplend_supply_position(mint: &Pubkey, protocol: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"user_supply_position", mint.as_ref(), protocol.as_ref()],
        &JUPLEND_LIQUIDITY_PROGRAM_ID,
    )
}

pub fn derive_juplend_borrow_position(mint: &Pubkey, protocol: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"user_borrow_position", mint.as_ref(), protocol.as_ref()],
        &JUPLEND_LIQUIDITY_PROGRAM_ID,
    )
}

pub fn derive_juplend_claim_account(user: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"user_claim", user.as_ref(), mint.as_ref()],
        &JUPLEND_LIQUIDITY_PROGRAM_ID,
    )
}

pub fn derive_juplend_rewards_admin() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"lending_rewards_admin"], &JUPLEND_REWARDS_PROGRAM_ID)
}

pub fn derive_juplend_rewards_rate_model(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"lending_rewards_rate_model", mint.as_ref()],
        &JUPLEND_REWARDS_PROGRAM_ID,
    )
}

pub fn derive_juplend_f_token_vault(program_id: &Pubkey, bank: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[JUPLEND_F_TOKEN_VAULT_SEED.as_bytes(), bank.as_ref()],
        program_id,
    )
}

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
        &SYSTEM_PROGRAM_ID,
        &SYSTEM_PROGRAM_ID,
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
        &[
            DRIFT_USER_SEED.as_bytes(),
            authority.as_ref(),
            &user_index.to_le_bytes(),
        ],
        &DRIFT_PROGRAM_ID,
    )
}

pub fn derive_drift_user_stats(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[DRIFT_USER_STATS_SEED.as_bytes(), authority.as_ref()],
        &DRIFT_PROGRAM_ID,
    )
}

pub fn derive_bank_vault(
    bank_pk: &Pubkey,
    vault_type: BankVaultType,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(crate::bank_seed!(vault_type, bank_pk), program_id)
}

pub fn derive_bank_vault_authority(
    bank_pk: &Pubkey,
    vault_type: BankVaultType,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(crate::bank_authority_seed!(vault_type, bank_pk), program_id)
}

/// Oracle accounts a bank contributes to a health check, in the order the risk engine expects.
/// Append these after the fixed accounts for any instruction that prices this bank.
pub fn bank_observation_keys(bank: &Bank) -> Vec<Pubkey> {
    let keys = &bank.config.oracle_keys;

    let mut out = match bank.config.oracle_setup {
        OracleSetup::None | OracleSetup::Fixed => vec![],
        OracleSetup::FixedKamino | OracleSetup::FixedDrift | OracleSetup::FixedJuplend => {
            vec![keys[1]]
        }
        OracleSetup::StakedWithPythPush => {
            let onramp = if keys[3] != Pubkey::default() {
                keys[3]
            } else if bank.integration_acc_1 != Pubkey::default() {
                derive_staked_onramp_from_vote(bank.integration_acc_1)
            } else {
                Pubkey::default()
            };
            vec![keys[0], keys[1], keys[2], onramp]
        }
        OracleSetup::PythLegacy
        | OracleSetup::SwitchboardV2
        | OracleSetup::PythPushOracle
        | OracleSetup::SwitchboardPull => vec![keys[0]],
        OracleSetup::KaminoPythPush
        | OracleSetup::KaminoSwitchboardPull
        | OracleSetup::DriftPythPull
        | OracleSetup::DriftSwitchboardPull
        | OracleSetup::SolendPythPull
        | OracleSetup::SolendSwitchboardPull
        | OracleSetup::JuplendPythPull
        | OracleSetup::JuplendSwitchboardPull => vec![keys[0], keys[1]],
    };

    out.retain(|pk| *pk != Pubkey::default());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ata_derivation_matches_spl() {
        let wallet = pubkey!("MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA");
        let mint = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        for token_program in [
            pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
        ] {
            assert_eq!(
                get_associated_token_address_with_program_id(&wallet, &mint, &token_program),
                spl_associated_token_account::get_associated_token_address_with_program_id(
                    &wallet,
                    &mint,
                    &token_program,
                ),
            );
        }
    }
}
