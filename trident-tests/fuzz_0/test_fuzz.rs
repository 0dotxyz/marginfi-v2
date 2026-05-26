use fuzz_accounts::*;
use trident_fuzz::fuzzing::*;

use crate::bank::Currency;
use crate::bank::FuzzTestBank;
use crate::types::marginfi::OracleSetup;
use crate::user::User;
mod constants;
mod fuzz_accounts;
mod invariants;
mod methods;
mod oracle_patch;
mod types;
mod user;
mod utils;

mod bank;

#[derive(FuzzTestMethods)]
struct FuzzTest {
    // ================================================================================================
    trident: Trident,
    fuzz_accounts: AccountAddresses,
    payer: Keypair,
    // ================================================================================================
    // Banks
    usdc_bank: FuzzTestBank,
    eth_bank: FuzzTestBank,
    btc_bank: FuzzTestBank,
    /// Token-2022 mint with `TransferFeeConfig` extension. Used to exercise
    /// marginfi's T22-with-fee deposit math at init time. Only the seeder
    /// holds a token account for this asset; no per-flow ops yet.
    t22_bank: FuzzTestBank,
    t22_seeder_token_account: Pubkey,
    t22_mint_authority: Pubkey,
    // ================================================================================================
    // Marginfi Group
    marginfi_group: Pubkey,
    fee_state: Pubkey,
    // ================================================================================================
    // Liquidator accounts
    liquidator: User,
    // ================================================================================================
    // Initial seeder
    seeder: User,
    // ================================================================================================
    // Users
    users: Vec<User>,
    // ================================================================================================
    // Kamino integration accounts
    kamino_main_lending_market: Pubkey,
    kamino_usdc_reserve_liquidity_supply: Pubkey,
    kamino_usdc_reserve_collateral_mint: Pubkey,
    kamino_usdc_reserve_collateral_supply_vault: Pubkey,
    kamino_usdc_reserve_farm_state: Pubkey,
    kamino_usdc_reserve: Pubkey,
    kamino_scope_prices: Pubkey,
    kamino_oracle: Pubkey,
    // ================================================================================================
    // Juplend integration accounts
    juplend_usdc_lending_state: Pubkey,
    juplend_lending_state_admin: Pubkey,
    juplend_usdc_f_token_mint: Pubkey,
    juplend_usdc_supply_token_reserves_liquidity: Pubkey,
    juplend_usdc_lending_supply_position_on_liquidity: Pubkey,
    juplend_usdc_rate_model: Pubkey,
    juplend_usdc_vault: Pubkey,
    juplend_usdc_liquidity: Pubkey,
    juplend_usdc_rewards_rate_model: Pubkey,
    juplend_claim_account: Pubkey,
    juplend_oracle: Pubkey,
}

#[flow_executor]
impl FuzzTest {
    fn new() -> Self {
        let mut trident = Trident::default();
        let payer = trident.random_keypair();

        // ================================================================================================
        // Marginfi Group
        let marginfi_group = trident.random_keypair().pubkey();
        let fee_state = trident
            .find_program_address(
                &[crate::constants::FEE_STATE_SEED.as_bytes()],
                &crate::types::marginfi::program_id(),
            )
            .0;

        // ================================================================================================
        // Banks
        let usdc_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            currency: Currency::new(constants::USDC, constants::USDC_MINT_AUTHORITY),
            oracle_setup: (OracleSetup::PythPushOracle, constants::USDC_PYTH_PUSH),
            has_transfer_fee: false,
        };
        let eth_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            currency: Currency::new(constants::WETH, constants::WETH_MINT_AUTHORITY),
            oracle_setup: (OracleSetup::PythPushOracle, constants::WETH_PYTH_PUSH),
            has_transfer_fee: false,
        };
        let btc_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            currency: Currency::new(constants::WBTC, constants::WBTC_MINT_AUTHORITY),
            oracle_setup: (OracleSetup::PythPushOracle, constants::BTC_PYTH_PUSH),
            has_transfer_fee: false,
        };
        // T22-with-fee bank — fresh runtime mint, reuses USDC's Pyth-push
        // for oracle so we don't need a 4th forked Pyth feed.
        let t22_mint = trident.random_keypair().pubkey();
        let t22_mint_authority = trident.random_keypair().pubkey();
        let t22_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            currency: Currency::new(t22_mint, t22_mint_authority),
            oracle_setup: (OracleSetup::PythPushOracle, constants::USDC_PYTH_PUSH),
            has_transfer_fee: true,
        };
        let t22_seeder_token_account = trident.random_keypair().pubkey();

        // ================================================================================================
        // Seeder accounts
        let seeder = User::new(
            "Seeder".to_string(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            usdc_amount!(100_000_000_000),
            trident.random_keypair().pubkey(),
            weth_amount!(10_000_000),
            trident.random_keypair().pubkey(),
            btc_amount!(500_000),
        );

        // ================================================================================================
        // User A accounts
        let user_a = User::new(
            "User A".to_string(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            usdc_amount!(100_000_000_000),
            trident.random_keypair().pubkey(),
            weth_amount!(10_000_000),
            trident.random_keypair().pubkey(),
            btc_amount!(500_000),
        );

        // ================================================================================================
        // User B accounts
        let user_b = User::new(
            "User B".to_string(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            usdc_amount!(100_000_000_000),
            trident.random_keypair().pubkey(),
            weth_amount!(10_000_000),
            trident.random_keypair().pubkey(),
            btc_amount!(500_000),
        );

        // ================================================================================================
        // User C accounts
        let user_c = User::new(
            "User C".to_string(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            usdc_amount!(100_000_000_000),
            trident.random_keypair().pubkey(),
            weth_amount!(10_000_000),
            trident.random_keypair().pubkey(),
            btc_amount!(500_000),
        );

        // User D accounts
        let user_d = User::new(
            "User D".to_string(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            usdc_amount!(100_000_000_000),
            trident.random_keypair().pubkey(),
            weth_amount!(10_000_000),
            trident.random_keypair().pubkey(),
            btc_amount!(500_000),
        );
        // ================================================================================================
        // Liquidator accounts
        let liquidator = User::new(
            "Liquidator".to_string(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            trident.random_keypair().pubkey(),
            usdc_amount!(10_000_000_000),
            trident.random_keypair().pubkey(),
            weth_amount!(1_000_000),
            trident.random_keypair().pubkey(),
            btc_amount!(500_000),
        );

        // ================================================================================================
        // Kamino integration accounts
        let kamino_main_lending_market = constants::KAMINO_MAIN_LENDING_MARKET;
        let kamino_usdc_reserve = constants::KAMINO_MAIN_MARKET_USDC_RESERVE;
        let kamino_usdc_reserve_liquidity_supply = constants::USDC_RESERVE_LIQUIDITY_VAULT;
        let kamino_usdc_reserve_collateral_mint = constants::USDC_RESERVE_COLLATERAL_MINT;
        let kamino_usdc_reserve_collateral_supply_vault = constants::USDC_RESERVE_COLLATERAL_VAULT;
        let kamino_usdc_reserve_farm_state = constants::FARMS_RESERVE_FARM_STATE_KEY;
        let kamino_scope_prices = constants::SCOPE_PRICES;
        let kamino_oracle = constants::USDC_PYTH_PUSH;
        // ================================================================================================
        // Juplend integration accounts
        let juplend_usdc_lending_state = constants::JUPITER_USDC_LENDING_STATE;
        let juplend_lending_state_admin = constants::JUPITER_USDC_LENDING_STATE_ADMIN;
        let juplend_usdc_f_token_mint = constants::JUPITER_USDC;
        let juplend_usdc_supply_token_reserves_liquidity =
            constants::JUPITER_USDC_SUPPLY_TOKEN_RESERVES_LIQUIDITY;
        let juplend_usdc_lending_supply_position_on_liquidity =
            constants::JUPITER_USDC_LENDING_SUPPLY_POSITION_ON_LIQUIDITY;
        let juplend_oracle = constants::USDC_PYTH_PUSH;
        let juplend_usdc_rate_model = constants::JUPITER_USDC_RATE_MODEL;
        let juplend_usdc_vault = constants::JUPITER_USDC_VAULT;
        let juplend_usdc_liquidity = constants::JUPITER_USDC_LIQUIDITY;
        let juplend_usdc_rewards_rate_model = constants::JUPITER_USDC_REWARDS_RATE_MODEL;
        let juplend_claim_account = constants::JUPITER_CLAIM_ACCOUNT;
        // ================================================================================================
        // Mainnet Slot
        let slot = utils::get_slot();
        trident.warp_to_slot(slot);

        Self {
            trident,
            payer,
            liquidator,
            seeder,
            marginfi_group,
            fee_state,
            usdc_bank,
            eth_bank,
            btc_bank,
            t22_bank,
            t22_seeder_token_account,
            t22_mint_authority,
            fuzz_accounts: AccountAddresses,
            kamino_usdc_reserve,
            kamino_main_lending_market,
            kamino_usdc_reserve_liquidity_supply,
            kamino_usdc_reserve_collateral_mint,
            kamino_usdc_reserve_collateral_supply_vault,
            kamino_usdc_reserve_farm_state,
            kamino_scope_prices,
            kamino_oracle,
            juplend_usdc_lending_state,
            juplend_lending_state_admin,
            juplend_usdc_f_token_mint,
            juplend_usdc_supply_token_reserves_liquidity,
            juplend_usdc_lending_supply_position_on_liquidity,
            juplend_usdc_rate_model,
            juplend_usdc_vault,
            juplend_usdc_liquidity,
            juplend_usdc_rewards_rate_model,
            juplend_claim_account,
            juplend_oracle,
            users: vec![user_a, user_b, user_c, user_d],
        }
    }

    #[init]
    fn start(&mut self) {
        // ================================================================================================
        // Initialization
        self.init_foundation();

        // ================================================================================================
        // Seeder deposits USDC
        let amount: u64 = usdc_amount!(10_000_000);
        self.lending_account_deposit(
            amount,
            self.usdc_bank,
            self.seeder.usdc_token_account,
            self.seeder.marginfi_account,
            self.seeder.address,
            None,
        );
        // ================================================================================================
        // Seeder deposits WETH
        let amount: u64 = weth_amount!(1_000_000);
        self.lending_account_deposit(
            amount,
            self.eth_bank,
            self.seeder.eth_token_account,
            self.seeder.marginfi_account,
            self.seeder.address,
            None,
        );

        // ================================================================================================
        // Seeder deposits cbBTC
        let amount: u64 = btc_amount!(500_000);
        self.lending_account_deposit(
            amount,
            self.btc_bank,
            self.seeder.btc_token_account,
            self.seeder.marginfi_account,
            self.seeder.address,
            None,
        );
    }
    // ================================================================================================
    // Deposit - USDC
    #[flow(weight = 15)]
    fn flow1(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_deposit(
            amount,
            self.usdc_bank,
            user.usdc_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("USDC deposit for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Withdraw - USDC
    #[flow(weight = 13)]
    fn flow2(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_withdraw(
            amount,
            self.usdc_bank,
            user.usdc_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("USDC withdraw for {}", user.name).as_str()),
        );
    }
    // ================================================================================================
    // Borrow - ETH
    #[flow(weight = 14)]
    fn flow3(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_borrow(
            amount,
            self.eth_bank,
            user.eth_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("ETH borrow for {}", user.name).as_str()),
        );
    }
    // ================================================================================================
    // Repay - ETH
    #[flow(weight = 14)]
    fn flow4(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_repay(
            amount,
            self.eth_bank,
            user.eth_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("ETH repay for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Flashloan - BTC
    #[flow(weight = 9)]
    fn flow6(&mut self) {
        let borrow: u64 = self.trident.random_log_uniform();
        let repay: u64 = if coin_toss!(self) {
            borrow
        } else {
            self.trident.random_log_uniform()
        };
        let user = self.get_random_user();
        self.lending_flashloan_borrow_repay(
            borrow,
            repay,
            self.btc_bank,
            &user,
            Some(format!("BTC flashloan for {}", user.name).as_str()),
        );
    }
    // ================================================================================================
    // Liquidate - USDC vs ETH
    #[flow(weight = 4)]
    fn flow7(&mut self) {
        let mut asset_amount: u64 = self.trident.random_log_uniform();
        if asset_amount == 0 {
            asset_amount = 1;
        }
        // Worsen liab (ETH) vs collateral; restore with the inverse rational so oracle returns to baseline.
        let eth_oracle = crate::constants::WETH_PYTH_PUSH;
        let numerator: i64 = self.trident.random_from_range(1000..=1_000_000);
        let denominator: i64 = 1;
        let user = self.get_random_user();
        self.scale_pyth_push_oracle_prices(&eth_oracle, numerator, denominator);
        self.lending_account_liquidate(
            asset_amount,
            self.usdc_bank,
            self.eth_bank,
            self.liquidator.marginfi_account,
            self.liquidator.address,
            user.marginfi_account,
            Some(format!("Liquidation — USDC vs ETH, liquidatee: {}", user.name).as_str()),
        );
        self.scale_pyth_push_oracle_prices(&eth_oracle, denominator, numerator);
    }

    // ================================================================================================
    // Standalone oracle move (does NOT revert)
    //
    // Flows 7/8 already scale the ETH oracle, but they revert in the same
    // call so subsequent ops see the pre-scaled price. This flow is the
    // analog of the legacy libfuzzer harness's `UpdateOracle` action: pick
    // a random Pyth-push oracle, apply a random multiplier in [0.5×, 5×],
    // and **leave it that way** so the next deposit/borrow/withdraw/repay
    // exercises health checks against the new price. Catches bugs that
    // only surface when prices drift between operations.
    #[flow(weight = 3)]
    fn flow_oracle_move(&mut self) {
        let oracle = match self.trident.random_from_range(0u8..=2) {
            0 => constants::USDC_PYTH_PUSH,
            1 => constants::WETH_PYTH_PUSH,
            _ => constants::BTC_PYTH_PUSH,
        };
        // Numerator in [50, 500], denominator = 100  → scale ∈ [0.5×, 5×].
        // Weighted toward small moves because more integer values fall in
        // the low half of the range.
        let numerator: i64 = self.trident.random_from_range(50i64..=500);
        let denominator: i64 = 100;
        self.scale_pyth_push_oracle_prices(&oracle, numerator, denominator);
    }

    // ================================================================================================
    // Lender Receivership Liquidation - ETH
    #[flow(weight = 2)]
    fn flow8(&mut self) {
        let eth_oracle = crate::constants::WETH_PYTH_PUSH;
        let numerator: i64 = self.trident.random_from_range(1000..=1_000_000);
        let denominator: i64 = 1;
        self.scale_pyth_push_oracle_prices(&eth_oracle, numerator, denominator);
        let user = self.get_random_user();
        self.lending_account_receivership_liquidation(
            user.marginfi_account,
            self.payer.pubkey(),
            self.payer.pubkey(),
            &[],
            Some(
                format!(
                    "Receivership liquidation — start/end, liquidatee: {}",
                    user.name
                )
                .as_str(),
            ),
        );
        self.scale_pyth_push_oracle_prices(&eth_oracle, denominator, numerator);
    }

    // ================================================================================================
    // Deposit to Kamino Obligation - USDC
    #[flow(weight = 6)]
    fn flow10(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.deposit_to_kamino_obligation(
            user.marginfi_account,
            user.address,
            user.usdc_token_account,
            self.usdc_bank.currency.mint,
            self.kamino_main_lending_market,
            self.kamino_usdc_reserve,
            self.kamino_usdc_reserve_liquidity_supply,
            self.kamino_usdc_reserve_collateral_mint,
            self.kamino_usdc_reserve_collateral_supply_vault,
            self.kamino_usdc_reserve_farm_state,
            Some(self.kamino_scope_prices),
            amount,
            Some(format!("Deposit to Kamino Obligation for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Deposit to Jupiter Obligation - USDC
    #[flow(weight = 8)]
    fn flow12(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.deposit_to_juplend(
            user.marginfi_account,
            user.address,
            user.usdc_token_account,
            self.usdc_bank.currency.mint,
            self.juplend_usdc_lending_state,
            self.juplend_usdc_f_token_mint,
            self.juplend_lending_state_admin,
            self.juplend_usdc_supply_token_reserves_liquidity,
            self.juplend_usdc_lending_supply_position_on_liquidity,
            self.juplend_usdc_rate_model,
            self.juplend_usdc_vault,
            self.juplend_usdc_liquidity,
            self.juplend_usdc_rewards_rate_model,
            amount,
            Some(format!("Deposit to Jupiter position for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Withdraw from Kamino Obligation - USDC
    #[flow(weight = 4)]
    fn flow11(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.withdraw_from_kamino_obligation(
            user.marginfi_account,
            user.address,
            user.usdc_token_account,
            self.usdc_bank.currency.mint,
            self.kamino_main_lending_market,
            self.kamino_usdc_reserve,
            self.kamino_usdc_reserve_liquidity_supply,
            self.kamino_usdc_reserve_collateral_mint,
            self.kamino_usdc_reserve_collateral_supply_vault,
            self.kamino_usdc_reserve_farm_state,
            Some(self.kamino_scope_prices),
            amount,
            None,
            Some(format!("Withdraw from Kamino Obligation for {}", user.name).as_str()),
        );
    }
    // ================================================================================================
    // Withdraw from Jupiter Obligation - USDC
    #[flow(weight = 3)]
    fn flow13(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.withdraw_from_juplend(
            user.marginfi_account,
            user.address,
            user.usdc_token_account,
            self.usdc_bank.currency.mint,
            self.juplend_usdc_lending_state,
            self.juplend_usdc_f_token_mint,
            self.juplend_lending_state_admin,
            self.juplend_usdc_supply_token_reserves_liquidity,
            self.juplend_usdc_lending_supply_position_on_liquidity,
            self.juplend_usdc_rate_model,
            self.juplend_usdc_vault,
            self.juplend_usdc_liquidity,
            self.juplend_usdc_rewards_rate_model,
            self.juplend_claim_account,
            amount,
            None,
            Some(format!("Withdraw from Jupiter Position for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Forward In Time + accrue (clock / interest)
    #[flow(weight = 5)]
    fn flow9(&mut self) {
        let time_forward: i64 = self.trident.random_from_range(100..100000);
        self.trident.forward_in_time(time_forward);
        self.lending_pool_accrue_all_banks(Some("Accrue bank interest after time warp"));

        self.update_pyth_timestamp(
            &constants::USDC_PYTH_PUSH,
            self.trident.get_current_timestamp(),
        );
        self.update_pyth_timestamp(
            &constants::WETH_PYTH_PUSH,
            self.trident.get_current_timestamp(),
        );
        self.update_pyth_timestamp(
            &constants::BTC_PYTH_PUSH,
            self.trident.get_current_timestamp(),
        );
    }

    #[end]
    fn end(&mut self) {
        // Advance time by an hour and accrue so any pending interest is
        // materialized into the bank's totals before we run the end-of-
        // sequence reconciliations.
        self.trident.forward_in_time(3600);
        self.lending_pool_accrue_all_banks(Some("End-of-sequence accrue for solvency check"));

        let banks = [
            self.usdc_bank.address,
            self.eth_bank.address,
            self.btc_bank.address,
        ];

        // Bank-level solvency: `vault − fees ≈ deposits − liabs` per bank.
        for bank in banks {
            invariants::assert_bank_solvency(&mut self.trident, bank);
        }

        // Bank position-counter consistency: `bank.lending_position_count`
        // and `bank.borrowing_position_count` must equal the actual count
        // of marginfi accounts with non-zero positions in each bank.
        let marginfi_accounts: Vec<Pubkey> = self
            .users
            .iter()
            .map(|u| u.marginfi_account)
            .chain([self.seeder.marginfi_account, self.liquidator.marginfi_account])
            .collect();
        invariants::assert_bank_position_counts(&mut self.trident, &banks, &marginfi_accounts);
    }
}

fn main() {
    // CI scales this per trigger: short on PR (e.g. 500), full on push to
    // `main` (10000, the default), long on the nightly schedule (e.g. 50000).
    // Defaults match the historical hard-coded values so local
    // `trident fuzz run fuzz_0` behaves the same as before.
    let iterations: u64 = std::env::var("FUZZ_ITERATIONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);
    let flows_per_iter: u64 = std::env::var("FUZZ_FLOWS_PER_ITER")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    FuzzTest::fuzz(iterations, flows_per_iter);
}
