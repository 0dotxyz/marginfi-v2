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
        };
        let eth_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            currency: Currency::new(constants::WETH, constants::WETH_MINT_AUTHORITY),
            oracle_setup: (OracleSetup::PythPushOracle, constants::WETH_PYTH_PUSH),
        };
        let btc_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            currency: Currency::new(constants::WBTC, constants::WBTC_MINT_AUTHORITY),
            oracle_setup: (OracleSetup::PythPushOracle, constants::BTC_PYTH_PUSH),
        };

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
    #[flow(weight = 16)]
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
    #[flow(weight = 15)]
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
    #[flow(weight = 15)]
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
    fn end(&mut self) {}
}

fn main() {
    FuzzTest::fuzz(10000, 50);
}
