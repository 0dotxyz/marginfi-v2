use anchor_lang::prelude::borsh::BorshSchema;
use anchor_lang::prelude::*;
use solana_program::stake::state::Lockup;

/***********************************|
|          Spl Stake Pool           |
|__________________________________*/

pub mod stake_pool {
    use super::*;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, AnchorSerialize, AnchorDeserialize, BorshSchema)]
    pub(crate) enum FutureEpoch<T> {
        None,
        One(T),
        Two(T),
    }

    impl<T> Default for FutureEpoch<T> {
        fn default() -> Self {
            Self::None
        }
    }

    #[derive(Clone, Debug, Default, PartialEq, AnchorDeserialize, AnchorSerialize, BorshSchema)]
    pub(crate) enum AccountType {
        #[default]
        Uninitialized,
        StakePool,
        ValidatorList,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default, AnchorSerialize, AnchorDeserialize, BorshSchema)]
    pub(crate) struct Fee {
        pub denominator: u64,
        pub numerator: u64,
    }

    impl PartialEq for Fee {
        fn eq(&self, other: &Self) -> bool {
            let self_scaled = u128::from(self.numerator) * u128::from(other.denominator);
            let other_scaled = u128::from(other.numerator) * u128::from(self.denominator);
            self_scaled == other_scaled
        }
    }

    impl Eq for Fee {}

    impl PartialOrd for Fee {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Fee {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            let self_scaled = u128::from(self.numerator) * u128::from(other.denominator);
            let other_scaled = u128::from(other.numerator) * u128::from(self.denominator);
            self_scaled.cmp(&other_scaled)
        }
    }

    #[repr(C)]
    #[derive(Clone, Debug, Default, PartialEq, AnchorDeserialize, BorshSchema)]
    pub(crate) struct StakePool {
        pub account_type: AccountType,
        pub manager: Pubkey,
        pub staker: Pubkey,
        pub stake_deposit_authority: Pubkey,
        pub stake_withdraw_bump_seed: u8,
        pub validator_list: Pubkey,
        pub reserve_stake: Pubkey,
        pub pool_mint: Pubkey,
        pub manager_fee_account: Pubkey,
        pub token_program_id: Pubkey,
        pub total_lamports: u64,
        pub pool_token_supply: u64,
        pub last_update_epoch: u64,
        pub lockup: Lockup,
        pub epoch_fee: Fee,
        pub next_epoch_fee: FutureEpoch<Fee>,
        pub preferred_deposit_validator_vote_address: Option<Pubkey>,
        pub preferred_withdraw_validator_vote_address: Option<Pubkey>,
        pub stake_deposit_fee: Fee,
        pub stake_withdrawal_fee: Fee,
        pub next_stake_withdrawal_fee: FutureEpoch<Fee>,
        pub stake_referral_fee: u8,
        pub sol_deposit_authority: Option<Pubkey>,
        pub sol_deposit_fee: Fee,
        pub sol_referral_fee: u8,
        pub sol_withdraw_authority: Option<Pubkey>,
        pub sol_withdrawal_fee: Fee,
        pub next_sol_withdrawal_fee: FutureEpoch<Fee>,
        pub last_epoch_pool_token_supply: u64,
        pub last_epoch_total_lamports: u64,
    }
}

/***********************************|
|          Msol Stake Pool          |
|__________________________________*/

pub mod msol_pool {
    use anchor_lang::prelude::borsh::BorshSchema;

    use super::*;
    #[derive(
        Clone,
        Copy,
        Debug,
        Default,
        AnchorSerialize,
        AnchorDeserialize,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
    )]
    pub struct Fee {
        pub basis_points: u32,
    }

    #[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, Debug)]
    pub struct LiqPool {
        pub lp_mint: Pubkey,
        pub lp_mint_authority_bump_seed: u8,
        pub sol_leg_bump_seed: u8,
        pub msol_leg_authority_bump_seed: u8,
        pub msol_leg: Pubkey,
        pub lp_liquidity_target: u64,
        pub lp_max_fee: Fee,
        pub lp_min_fee: Fee,
        pub treasury_cut: Fee,
        pub lp_supply: u64,
        pub lent_from_sol_leg: u64,
        pub liquidity_sol_cap: u64,
    }

    #[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, BorshSchema, Debug)]
    pub struct List {
        pub account: Pubkey,
        pub item_size: u32,
        pub count: u32,
        pub new_account: Pubkey,
        pub copied_count: u32,
    }

    #[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, Debug)]
    pub struct StakeSystem {
        pub stake_list: List,
        pub delayed_unstake_cooling_down: u64,
        pub stake_deposit_bump_seed: u8,
        pub stake_withdraw_bump_seed: u8,
        pub slots_for_stake_delta: u64,
        pub last_stake_delta_epoch: u64,
        pub min_stake: u64,
        pub extra_stake_delta_runs: u32,
    }

    #[derive(Default, Clone, AnchorSerialize, AnchorDeserialize, Debug)]
    pub struct ValidatorSystem {
        pub validator_list: List,
        pub manager_authority: Pubkey,
        pub total_validator_score: u32,
        pub total_active_balance: u64,
        pub auto_add_validator_enabled: u8,
    }

    #[derive(Default, Clone, Debug, AnchorDeserialize, AnchorSerialize)]
    pub struct State {
        pub msol_mint: Pubkey,
        pub admin_authority: Pubkey,
        pub operational_sol_account: Pubkey,
        pub treasury_msol_account: Pubkey,
        pub reserve_bump_seed: u8,
        pub msol_mint_authority_bump_seed: u8,
        pub rent_exempt_for_token_acc: u64,
        pub reward_fee: Fee,
        pub stake_system: StakeSystem,
        pub validator_system: ValidatorSystem,
        pub liq_pool: LiqPool,
        pub available_reserve_balance: u64,
        pub msol_supply: u64,
        pub msol_price: u64,
        pub circulating_ticket_count: u64,
        pub circulating_ticket_balance: u64,
        pub lent_from_reserve: u64,
        pub min_deposit: u64,
        pub min_withdraw: u64,
        pub staking_sol_cap: u64,
        pub emergency_cooling_down: u64,
    }
}

/***********************************|
|          Redstone Feed            |
|__________________________________*/

pub const RESERVED_BYTE_SIZE: usize = 64;
pub const U256_BYTE_SIZE: usize = 256 / 8;
pub const U64_START_INDEX: usize = U256_BYTE_SIZE - 8;

#[account]
pub struct RedstoneFeed {
    pub feed_id: [u8; U256_BYTE_SIZE],
    pub value: [u8; U256_BYTE_SIZE],
    // `timestamp` (in milliseconds) is when the price was computed...
    pub timestamp: u64,
    // ... and `write_timestamp` (in milliseconds) is when the price was pushed to the account
    pub write_timestamp: Option<u64>,
    pub write_slot_number: u64,
    pub decimals: u8,
    pub reserved: [u8; RESERVED_BYTE_SIZE],
}
