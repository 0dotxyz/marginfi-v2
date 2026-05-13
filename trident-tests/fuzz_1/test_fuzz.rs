use fuzz_accounts::*;
use trident_fuzz::fuzzing::*;

use crate::types::marginfi::OracleSetup;
mod constants;
mod core_methods;
mod fuzz_accounts;
mod init_methods;
mod invariants;
mod kamino_methods;
mod kamino_utils;
mod oracle_patch;
mod types;
mod utils;

#[derive(Clone, Copy)]
pub struct FuzzTestBank {
    pub address: Pubkey,
    pub mint: Pubkey,
    pub mint_authority: Pubkey,
    pub oracle_setup: (OracleSetup, Pubkey),
}

#[derive(Clone, Copy)]
pub struct User {
    pub address: Pubkey,
    pub marginfi_account: Pubkey,
    pub usdc_token_account: Pubkey,
    pub eth_token_account: Pubkey,
    pub btc_token_account: Pubkey,
}

impl User {}

#[derive(FuzzTestMethods)]
struct FuzzTest {
    // ---------------------------------------------
    trident: Trident,
    fuzz_accounts: AccountAddresses,
    payer: Keypair,
    // ---------------------------------------------
    // Banks
    usdc_bank: FuzzTestBank,
    eth_bank: FuzzTestBank,
    btc_bank: FuzzTestBank,
    // ---------------------------------------------
    // Marginfi Group
    marginfi_group: Pubkey,
    fee_state: Pubkey,
    // ---------------------------------------------
    // User A accounts
    user_a: User,
    // ---------------------------------------------
    // Flashloan-only user (BTC bank).
    user_b: User,
    // ---------------------------------------------
    // Liquidator accounts
    liquidator: User,
    // ---------------------------------------------
    // Initial seeder
    seeder: User,
    // ---------------------------------------------
    // Kamino integration accounts
    // kamino_main_lending_market: Pubkey,
    // kamino_lending_market_authority: Pubkey,
    // kamino_usdc_reserve: Pubkey,

    // ---------------------------------------------
    // Mainnet Slot
    // slot: u64,
}

#[flow_executor]
impl FuzzTest {
    fn new() -> Self {
        let mut trident = Trident::default();
        let payer = trident.random_keypair();

        // Marginfi Group
        let marginfi_group = trident.random_keypair().pubkey();
        let fee_state = trident
            .find_program_address(
                &[crate::constants::FEE_STATE_SEED.as_bytes()],
                &crate::types::marginfi::program_id(),
            )
            .0;

        // Mints
        let usdc_mint = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        let usdc_mint_authority = pubkey!("BJE5MMbqXjVwjAF7oxwPYXnTXDyspzZyt4vwenNw5ruG");

        let eth_mint = pubkey!("7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs");
        let eth_mint_authority = pubkey!("BCD75RNBHrJJpW4dXVagL5mPjzRLnVZq4YirJdjEYMV7");

        let btc_mint = pubkey!("5XZw2LKTyrfvfiskJ78AMpackRjPcyCif1WhUsPDuVqQ");
        let btc_mint_authority = pubkey!("8qAJSTfLJH7MWDMDGTNEFCijHXHmd5gxu22erUnQ9zt8");

        // Banks
        let usdc_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            mint: usdc_mint,
            mint_authority: usdc_mint_authority,
            oracle_setup: (OracleSetup::PythPushOracle, constants::USDC_PYTH_PUSH),
        };
        let eth_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            mint: eth_mint,
            mint_authority: eth_mint_authority,
            oracle_setup: (OracleSetup::PythPushOracle, constants::WETH_PYTH_PUSH),
        };
        let btc_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            mint: btc_mint,
            mint_authority: btc_mint_authority,
            oracle_setup: (OracleSetup::PythPushOracle, constants::BTC_PYTH_PUSH),
        };

        // Seeder accounts
        let seeder = User {
            address: trident.random_keypair().pubkey(),
            marginfi_account: trident.random_keypair().pubkey(),
            usdc_token_account: trident.random_keypair().pubkey(),
            eth_token_account: trident.random_keypair().pubkey(),
            btc_token_account: trident.random_keypair().pubkey(),
        };

        // User A accounts
        let user_a = User {
            address: trident.random_keypair().pubkey(),
            marginfi_account: trident.random_keypair().pubkey(),
            usdc_token_account: trident.random_keypair().pubkey(),
            eth_token_account: trident.random_keypair().pubkey(),
            btc_token_account: trident.random_keypair().pubkey(),
        };

        // User B accounts
        let user_b = User {
            address: trident.random_keypair().pubkey(),
            marginfi_account: trident.random_keypair().pubkey(),
            usdc_token_account: trident.random_keypair().pubkey(),
            eth_token_account: trident.random_keypair().pubkey(),
            btc_token_account: trident.random_keypair().pubkey(),
        };

        // Liquidator accounts
        let liquidator = User {
            address: trident.random_keypair().pubkey(),
            marginfi_account: trident.random_keypair().pubkey(),
            usdc_token_account: trident.random_keypair().pubkey(),
            eth_token_account: trident.random_keypair().pubkey(),
            btc_token_account: trident.random_keypair().pubkey(),
        };

        // Mainnet Slot
        // let slot = utils::get_slot();
        // trident.warp_to_slot(slot);

        // let kamino_usdc_reserve = constants::KAMINO_USDC_RESERVE;
        // let kamino_main_lending_market = constants::KAMINO_LENDING_MARKET;
        // let kamino_lending_market_authority = constants::KAMINO_LENDING_MARKET_AUTHORITY;

        Self {
            trident,
            fuzz_accounts: AccountAddresses::default(),
            payer,
            user_a,
            user_b,
            liquidator,
            seeder,
            marginfi_group,
            fee_state,
            usdc_bank,
            eth_bank,
            btc_bank,
            // kamino_usdc_reserve,
            // kamino_main_lending_market,
            // kamino_lending_market_authority,
            // slot,
        }
    }

    #[init]
    fn start(&mut self) {
        // Initialization
        self.init_foundation();

        // Seeder deposits
        let amount: u64 = u64::MAX / 100;
        self.lending_account_deposit(
            amount,
            self.usdc_bank,
            self.seeder.usdc_token_account,
            self.seeder.marginfi_account,
            self.seeder.address,
            None,
        );

        // Seeder deposits
        let amount: u64 = u64::MAX / 100;
        self.lending_account_deposit(
            amount,
            self.eth_bank,
            self.seeder.eth_token_account,
            self.seeder.marginfi_account,
            self.seeder.address,
            None,
        );

        self.lending_account_deposit(
            amount,
            self.btc_bank,
            self.seeder.btc_token_account,
            self.seeder.marginfi_account,
            self.seeder.address,
            None,
        );

        let usdc_bank_layout = self.bank_layout(self.usdc_bank.address);
        let eth_bank_layout = self.bank_layout(self.eth_bank.address);
        let btc_bank_layout = self.bank_layout(self.btc_bank.address);

        let usdc_bank_vault = self
            .trident
            .get_token_account(usdc_bank_layout.liquidity_vault)
            .expect("has to exist");

        let eth_bank_vault = self
            .trident
            .get_token_account(eth_bank_layout.liquidity_vault)
            .expect("has to exist");

        let btc_bank_vault = self
            .trident
            .get_token_account(btc_bank_layout.liquidity_vault)
            .expect("has to exist");

        invariant!(usdc_bank_vault.account.amount == amount);
        invariant!(eth_bank_vault.account.amount == amount);
        invariant!(btc_bank_vault.account.amount == amount);
    }
    // ---------------------------------------------------------------------------------------------------------------------------------------
    // Deposit - USDC
    #[flow(weight = 19)]
    fn flow1(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        self.lending_account_deposit(
            amount,
            self.usdc_bank,
            self.user_a.usdc_token_account,
            self.user_a.marginfi_account,
            self.user_a.address,
            Some("Lender Deposit - USDC"),
        );
    }

    // ---------------------------------------------------------------------------------------------------------------------------------------
    // Withdraw - USDC
    #[flow(weight = 17)]
    fn flow2(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        self.lending_account_withdraw(
            amount,
            self.usdc_bank,
            self.user_a.usdc_token_account,
            self.user_a.marginfi_account,
            self.user_a.address,
            Some("Lender Withdraw - USDC"),
        );
    }
    // ---------------------------------------------------------------------------------------------------------------------------------------
    // Borrow - ETH
    #[flow(weight = 17)]
    fn flow3(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        self.lending_account_borrow(
            amount,
            self.eth_bank,
            self.user_a.eth_token_account,
            self.user_a.marginfi_account,
            self.user_a.address,
            Some("Lender Borrow - ETH"),
        );
    }
    // ---------------------------------------------------------------------------------------------------------------------------------------
    // Repay - ETH
    #[flow(weight = 17)]
    fn flow4(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        self.lending_account_repay(
            amount,
            self.eth_bank,
            self.user_a.eth_token_account,
            self.user_a.marginfi_account,
            self.user_a.address,
            Some("Lender Repay - ETH"),
        );
    }

    // ---------------------------------------------------------------------------------------------------------------------------------------
    // Flashloan - BTC
    #[flow(weight = 9)]
    fn flow6(&mut self) {
        let borrow: u64 = self.trident.random_log_uniform();
        let coin: u64 = self.trident.random_log_uniform();
        let matched = (coin & 1) == 0;
        let repay: u64 = if matched {
            borrow
        } else {
            self.trident.random_log_uniform()
        };
        self.lending_flashloan_borrow_repay(
            borrow,
            repay,
            self.btc_bank,
            self.user_b.marginfi_account,
            self.user_b.address,
            self.user_b.btc_token_account,
            Some("User B flashloan - BTC borrow vs repay"),
        );
    }
    // ---------------------------------------------------------------------------------------------------------------------------------------
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
        self.scale_pyth_push_oracle_prices(&eth_oracle, numerator, denominator);
        self.lending_account_liquidate(
            asset_amount,
            self.usdc_bank,
            self.eth_bank,
            self.liquidator.marginfi_account,
            self.liquidator.address,
            self.user_a.marginfi_account,
            Some("Liquidation — USDC vs ETH"),
        );
        self.scale_pyth_push_oracle_prices(&eth_oracle, denominator, numerator);
    }

    // ---------------------------------------------------------------------------------------------------------------------------------------
    // Lender Receivership Liquidation - ETH
    #[flow(weight = 2)]
    fn flow8(&mut self) {
        let eth_oracle = crate::constants::WETH_PYTH_PUSH;
        let numerator: i64 = self.trident.random_from_range(1000..=1_000_000);
        let denominator: i64 = 1;
        self.scale_pyth_push_oracle_prices(&eth_oracle, numerator, denominator);
        self.lending_account_receivership_liquidation(
            self.user_a.marginfi_account,
            self.payer.pubkey(),
            self.payer.pubkey(),
            &[],
            Some("Receivership liquidation — start/end"),
        );
        self.scale_pyth_push_oracle_prices(&eth_oracle, denominator, numerator);
    }

    // ---------------------------------------------------------------------------------------------------------------------------------------
    // Forward In Time + accrue (clock / interest)
    #[flow(weight = 15)]
    fn flow9(&mut self) {
        let time_forward: i64 = self.trident.random_from_range(100..100000);
        self.trident.forward_in_time(time_forward);
        self.lending_pool_accrue_all_banks(Some("Accrue bank interest after time warp"));
    }

    #[end]
    fn end(&mut self) {}
}

fn main() {
    FuzzTest::fuzz(10000, 50);
}
