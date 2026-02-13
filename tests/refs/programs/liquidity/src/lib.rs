use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod module;
pub mod state;
pub mod utils;

use crate::state::*;
use library::structs::AddressBool;

#[cfg(feature = "staging")]
declare_id!("5uDkCoM96pwGYhAUucvCzLfm5UcjVRuxz6gH81RnRBmL");

#[cfg(not(feature = "staging"))]
declare_id!("jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC");

#[program]
pub mod liquidity {
    use super::*;

    /***********************************|
    |         User Module               |
    |__________________________________*/

    pub fn pre_operate(ctx: Context<PreOperate>, mint: Pubkey) -> Result<()> {
        module::user::pre_operate(ctx, mint)
    }

    pub fn operate(
        ctx: Context<Operate>,
        supply_amount: i128,
        borrow_amount: i128,
        withdraw_to: Pubkey,
        borrow_to: Pubkey,
        transfer_type: TransferType,
    ) -> Result<(u64, u64)> {
        module::user::operate(
            ctx,
            supply_amount,
            borrow_amount,
            withdraw_to,
            borrow_to,
            transfer_type,
        )
    }

    pub fn init_claim_account(
        ctx: Context<InitClaimAccount>,
        mint: Pubkey,
        user: Pubkey,
    ) -> Result<()> {
        module::user::init_claim_account(ctx, mint, user)
    }

    pub fn close_claim_account(_ctx: Context<CloseClaimAccount>, _mint: Pubkey) -> Result<()> {
        Ok(())
    }

    pub fn claim(ctx: Context<Claim>, recipient: Pubkey) -> Result<()> {
        module::user::claim(ctx, recipient)
    }

    /***********************************|
    |         Admin Module              |
    |__________________________________*/

    pub fn init_liquidity(
        context: Context<InitLiquidity>,
        authority: Pubkey,
        revenue_collector: Pubkey,
    ) -> Result<()> {
        module::admin::init_liquidity(context, authority, revenue_collector)
    }

    pub fn init_token_reserve(context: Context<InitTokenReserve>) -> Result<()> {
        module::admin::init_token_reserve(context)
    }

    pub fn init_new_protocol(
        context: Context<InitNewProtocol>,
        supply_mint: Pubkey,
        borrow_mint: Pubkey,
        protocol: Pubkey,
    ) -> Result<()> {
        module::admin::init_new_protocol(context, supply_mint, borrow_mint, protocol)
    }

    pub fn update_authority(
        context: Context<UpdateAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        module::admin::update_authority(context, new_authority)
    }

    pub fn update_auths(
        context: Context<UpdateAuths>,
        auth_status: Vec<AddressBool>,
    ) -> Result<()> {
        module::admin::update_auths(context, auth_status)
    }

    pub fn update_guardians(
        context: Context<UpdateAuths>,
        guardian_status: Vec<AddressBool>,
    ) -> Result<()> {
        module::admin::update_guardians(context, guardian_status)
    }

    pub fn update_revenue_collector(
        context: Context<UpdateRevenueCollector>,
        revenue_collector: Pubkey,
    ) -> Result<()> {
        module::admin::update_revenue_collector(context, revenue_collector)
    }

    pub fn collect_revenue(context: Context<CollectRevenue>) -> Result<()> {
        module::admin::collect_revenue(context)
    }

    pub fn change_status(context: Context<ChangeStatus>, status: bool) -> Result<()> {
        module::admin::change_status(context, status)
    }

    pub fn update_rate_data_v1(
        context: Context<UpdateRateData>,
        rate_data: RateDataV1Params,
    ) -> Result<()> {
        module::admin::update_rate_data_v1(context, rate_data)
    }

    pub fn update_rate_data_v2(
        context: Context<UpdateRateData>,
        rate_data: RateDataV2Params,
    ) -> Result<()> {
        module::admin::update_rate_data_v2(context, rate_data)
    }

    pub fn update_token_config(
        context: Context<UpdateTokenConfig>,
        token_config: TokenConfig,
    ) -> Result<()> {
        module::admin::update_token_config(context, token_config)
    }

    pub fn update_user_class(
        context: Context<UpdateUserClass>,
        user_class: Vec<AddressU8>,
    ) -> Result<()> {
        module::admin::update_user_class(context, user_class)
    }

    pub fn update_user_withdrawal_limit(
        context: Context<UpdateUserWithdrawalLimit>,
        new_limit: u128,
        protocol: Pubkey,
        mint: Pubkey,
    ) -> Result<()> {
        module::admin::update_user_withdrawal_limit(context, new_limit, protocol, mint)
    }

    pub fn update_user_supply_config(
        context: Context<UpdateUserSupplyConfig>,
        user_supply_config: UserSupplyConfig,
    ) -> Result<()> {
        module::admin::update_user_supply_config(context, user_supply_config)
    }

    pub fn update_user_borrow_config(
        context: Context<UpdateUserBorrowConfig>,
        user_borrow_config: UserBorrowConfig,
    ) -> Result<()> {
        module::admin::update_user_borrow_config(context, user_borrow_config)
    }

    pub fn pause_user(
        context: Context<PauseUser>,
        protocol: Pubkey,
        supply_mint: Pubkey,
        borrow_mint: Pubkey,
        supply_status: Option<u8>,
        borrow_status: Option<u8>,
    ) -> Result<()> {
        module::admin::pause_user(
            context,
            protocol,
            supply_mint,
            borrow_mint,
            supply_status,
            borrow_status,
        )
    }

    pub fn unpause_user(
        context: Context<PauseUser>,
        protocol: Pubkey,
        supply_mint: Pubkey,
        borrow_mint: Pubkey,
        supply_status: Option<u8>,
        borrow_status: Option<u8>,
    ) -> Result<()> {
        module::admin::unpause_user(
            context,
            protocol,
            supply_mint,
            borrow_mint,
            supply_status,
            borrow_status,
        )
    }

    pub fn update_exchange_price(
        context: Context<UpdateExchangePrice>,
        _mint: Pubkey,
    ) -> Result<(u128, u128)> {
        let mut token_reserve = context.accounts.token_reserve.load_mut()?;
        let rate_model = context.accounts.rate_model.load()?;
        Ok(token_reserve.update_exchange_prices_and_rates(&rate_model)?)
    }
}
