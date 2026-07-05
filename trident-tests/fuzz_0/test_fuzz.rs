use std::collections::{HashMap, HashSet};

use fixed::types::I80F48;
use fuzz_accounts::*;
use trident_fuzz::fuzzing::*;

use crate::invariants::{AccountDataSnapshot, BankBaseline};

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
    /// Token-2022 mint with `TransferFeeConfig` extension. Drives marginfi's
    /// transfer-fee-aware deposit / withdraw paths. Every user holds a
    /// `t22_token_account` (see `User`); the seeder mints + deposits at
    /// init, and the 4 users exercise `flow_t22_deposit` / `flow_t22_withdraw`.
    t22_bank: FuzzTestBank,
    t22_mint_authority: Pubkey,
    /// Isolated-tier bank (`RiskTier::Isolated`, `asset_weight = 0`).
    /// Exists so the natural cross-bank interactions in the random fuzz
    /// hit `IsolatedAccountIllegalState` (6029) — `flow_isolated_deposit`
    /// gives users positions to mix from.
    isolated_bank: FuzzTestBank,
    isolated_mint_authority: Pubkey,
    // ================================================================================================
    // Marginfi Group
    marginfi_group: Pubkey,
    fee_state: Pubkey,
    fee_state_v2: Pubkey,
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
    // ================================================================================================
    // Per-sequence accrue tracking — drives the solvency tolerance in
    // `#[end]`. Every successful bank-touching ix (deposit, withdraw,
    // borrow, repay, liquidate, bankruptcy) implicitly accrues its target
    // bank; every `LendingPoolAccrueBankInterest` is an explicit accrue.
    // Each accrue applies a chain of I80F48 mul/div ops that round at
    // ≈ 2⁻⁴⁸, so per-bank drift grows ~linearly with this count.
    pub(crate) accrue_counts: HashMap<Pubkey, u32>,

    // Post-init snapshot of per-bank share values and outstanding-fee
    // buckets — drives the `bank_state` directional invariants in
    // `#[end]`. Captured once after foundation deposits + the M12
    // regression, then frozen.
    pub(crate) bank_baselines: HashMap<Pubkey, BankBaseline>,

    // Set of banks that saw a successful `LendingPoolHandleBankruptcy`
    // during the sequence. The auto-coupled bankruptcy inside
    // `lending_account_liquidate` registers here; the directional
    // invariant exempts `asset_share_value` (and `collected_insurance_
    // fees_outstanding`) monotonicity for these banks because
    // socialised loss legitimately reduces them.
    pub(crate) banks_with_bankruptcy: HashSet<Pubkey>,

    // End-of-`#[init]` raw-bytes snapshots of `MarginfiGroup` and
    // `FeeState`. Neither account is mutated by any flow the harness
    // exercises (no admin / config / pause / panic ixs in the flow
    // set), so byte-for-byte equality at `#[end]` is the strictest
    // immutability check available.
    pub(crate) marginfi_group_snapshot: Option<AccountDataSnapshot>,
    pub(crate) fee_state_snapshot: Option<AccountDataSnapshot>,
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
        let fee_state_v2 = trident
            .find_program_address(
                &[crate::constants::FEE_STATE_V2_SEED.as_bytes()],
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
        // Isolated-tier bank — fresh runtime mint, classic SPL Token,
        // reuses USDC's Pyth-push for the oracle. `RiskTier::Isolated` +
        // `asset_weight = 0` is set in `isolated_bank_config()`.
        let isolated_mint = trident.random_keypair().pubkey();
        let isolated_mint_authority = trident.random_keypair().pubkey();
        let isolated_bank = FuzzTestBank {
            address: trident.random_keypair().pubkey(),
            currency: Currency::new(isolated_mint, isolated_mint_authority),
            oracle_setup: (OracleSetup::PythPushOracle, constants::USDC_PYTH_PUSH),
            has_transfer_fee: false,
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
            trident.random_keypair().pubkey(),
            10_000_000_000,
            trident.random_keypair().pubkey(),
            10_000_000_000,
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
            trident.random_keypair().pubkey(),
            10_000_000_000,
            trident.random_keypair().pubkey(),
            10_000_000_000,
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
            trident.random_keypair().pubkey(),
            10_000_000_000,
            trident.random_keypair().pubkey(),
            10_000_000_000,
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
            trident.random_keypair().pubkey(),
            10_000_000_000,
            trident.random_keypair().pubkey(),
            10_000_000_000,
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
            trident.random_keypair().pubkey(),
            10_000_000_000,
            trident.random_keypair().pubkey(),
            10_000_000_000,
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
            trident.random_keypair().pubkey(),
            10_000_000_000,
            trident.random_keypair().pubkey(),
            10_000_000_000,
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
            fee_state_v2,
            usdc_bank,
            eth_bank,
            btc_bank,
            t22_bank,
            t22_mint_authority,
            isolated_bank,
            isolated_mint_authority,
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
            accrue_counts: HashMap::new(),
            bank_baselines: HashMap::new(),
            banks_with_bankruptcy: HashSet::new(),
            marginfi_group_snapshot: None,
            fee_state_snapshot: None,
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
            self.usdc_bank.clone(),
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
            self.eth_bank.clone(),
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
            self.btc_bank.clone(),
            self.seeder.btc_token_account,
            self.seeder.marginfi_account,
            self.seeder.address,
            None,
        );

        // ================================================================================================
        // M12 dust-on-repay regression
        //
        // Audit fix `b186b77e` ("M12 issue fix - dust clamping") closed a
        // hole in `increase_balance_internal::RepayOnly` where a repay
        // overshooting the user's liability by sub-`ZERO_AMOUNT_THRESHOLD`
        // dust silently minted asset shares on the balance. The old
        // seed-based regression (`regression-seeds/m12-dust-on-repay.*`)
        // pinned a master seed that reached the bug; subsequent harness
        // changes that consume randomness (`random_bool` calls in
        // deposit/withdraw/repay, the bankruptcy and accrue-tracking
        // additions) decoupled that seed from the dust codepath.
        //
        // This deterministic reproduction runs every sequence: User A
        // opens a small ETH borrow against fresh USDC collateral, then
        // repays 2 native units. The pre-state has `liability_shares > 0,
        // asset_shares = 0`; in the buggy code the dust repay flips
        // `asset_shares` to a tiny positive value, which
        // `assert_repay_success_share_invariants` (called from inside
        // `lending_account_repay`) fires on. On audit-fixed code the
        // assertion passes silently.
        // Forked Pyth-push feeds carry the fork-time `publish_time`, so
        // health-checking ixs (borrow / repay) reject them as stale.
        // `update_pyth_timestamp` is normally driven from `flow9`, but
        // the M12 reproduction runs in `#[init]` before any flow fires
        // — bring the relevant oracles current here.
        let now = self.trident.get_current_timestamp();
        self.update_pyth_timestamp(&constants::USDC_PYTH_PUSH, now);
        self.update_pyth_timestamp(&constants::WETH_PYTH_PUSH, now);

        let user_a = self.users[0].clone();
        self.lending_account_deposit(
            usdc_amount!(1_000),
            self.usdc_bank.clone(),
            user_a.usdc_token_account,
            user_a.marginfi_account,
            user_a.address,
            Some("[M12 regression] User A USDC collateral"),
        );
        self.lending_account_borrow(
            100_000,
            self.eth_bank.clone(),
            user_a.eth_token_account,
            user_a.marginfi_account,
            user_a.address,
            Some("[M12 regression] User A ETH borrow"),
        );
        self.lending_account_repay(
            2,
            self.eth_bank.clone(),
            user_a.eth_token_account,
            user_a.marginfi_account,
            user_a.address,
            Some("[M12 regression] User A dust repay"),
        );

        // ================================================================================================
        // Take the per-bank baseline snapshot once foundation + M12 setup
        // settle. From here on, the `bank_state` directional invariants
        // (share-value monotonicity, fee-bucket monotonicity, cumulative
        // shares ≤ totals, last_update strict advance) measure drift
        // against this frozen reference.
        self.bank_baselines = invariants::snapshot_bank_baselines(
            &mut self.trident,
            &[
                self.usdc_bank.address,
                self.eth_bank.address,
                self.btc_bank.address,
            ],
        );

        // Group + fee-state immutability snapshots — no flow in the
        // harness invokes a group-admin / fee-state-mutating ix, so
        // raw bytes must be identical at `#[end]`.
        self.marginfi_group_snapshot = Some(invariants::snapshot_account_data(
            &mut self.trident,
            self.marginfi_group,
        ));
        self.fee_state_snapshot = Some(invariants::snapshot_account_data(
            &mut self.trident,
            self.fee_state,
        ));
    }
    // ================================================================================================
    // Deposit - USDC
    #[flow(weight = 7)]
    fn flow1(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_deposit(
            amount,
            self.usdc_bank.clone(),
            user.usdc_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("USDC deposit for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Withdraw - USDC
    #[flow(weight = 10)]
    fn flow2(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_withdraw(
            amount,
            self.usdc_bank.clone(),
            user.usdc_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("USDC withdraw for {}", user.name).as_str()),
        );
    }
    // ================================================================================================
    // Borrow - ETH
    #[flow(weight = 11)]
    fn flow3(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_borrow(
            amount,
            self.eth_bank.clone(),
            user.eth_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("ETH borrow for {}", user.name).as_str()),
        );
    }
    // ================================================================================================
    // Repay - ETH
    #[flow(weight = 12)]
    fn flow4(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_repay(
            amount,
            self.eth_bank.clone(),
            user.eth_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("ETH repay for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Multi-bank flashloan — borrow on two banks, repay both in the
    // same tx. Exercises `LendingAccountStartFlashloan` /
    // `LendingAccountEndFlashloan` with non-trivial middle ixs: a
    // crossed pair of borrow/repay touching two different banks. The
    // `flow6` single-bank flashloan covers the close-loop case; this
    // covers the multi-bank-conservation case (end_health check sees
    // both banks).
    //
    // Amounts on each leg match (borrow == repay), so the flashloan
    // nets to zero and `assert_flashloan_closed_loop_user_unchanged`
    // semantics hold for every touched bank. The bigger payoff is on
    // *failure* — partially-balanced flashloans must revert in full,
    // and the helper already asserts `state-unchanged on tx failure`.
    #[flow(weight = 2)]
    fn flow_flashloan_multibank(&mut self) {
        let amount_eth: u64 = self.trident.random_log_uniform();
        let amount_btc: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();

        let borrow_eth = self.lending_account_borrow_ix(
            amount_eth,
            self.eth_bank.address,
            self.eth_bank.currency.mint,
            user.eth_token_account,
            user.marginfi_account,
            user.address,
        );
        let borrow_btc = self.lending_account_borrow_ix(
            amount_btc,
            self.btc_bank.address,
            self.btc_bank.currency.mint,
            user.btc_token_account,
            user.marginfi_account,
            user.address,
        );
        let repay_btc = self.lending_account_repay_ix(
            amount_btc,
            self.btc_bank.address,
            self.btc_bank.currency.mint,
            user.btc_token_account,
            user.marginfi_account,
            user.address,
            true,
        );
        let repay_eth = self.lending_account_repay_ix(
            amount_eth,
            self.eth_bank.address,
            self.eth_bank.currency.mint,
            user.eth_token_account,
            user.marginfi_account,
            user.address,
            true,
        );

        let _ = self.lending_flashloan(
            &user,
            vec![borrow_eth, borrow_btc, repay_btc, repay_eth],
            Some(
                format!("Multi-bank flashloan (ETH+BTC) for {}", user.name).as_str(),
            ),
            Some(vec![self.eth_bank.address, self.btc_bank.address]),
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
            self.btc_bank.clone(),
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
            self.usdc_bank.clone(),
            self.eth_bank.clone(),
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
        // Numerator in [0, 1_000_000], denominator = 100  →
        //   scale ∈ {0×, ~0.01×, …, ~0.5×, 1×, 2×, …, ~10000×}.
        // The 0 endpoint exercises marginfi's zero-price guards
        // (`ZeroAssetPrice` / `ZeroLiabilityPrice`); the high end pushes
        // health checks against extreme valuations without permanently
        // crashing the test (oracle scale is per-flow, never reverted).
        let numerator: i64 = self.trident.random_from_range(0i64..=1_000_000);
        let denominator: i64 = 100;
        self.scale_pyth_push_oracle_prices(&oracle, numerator, denominator);
    }

    // ================================================================================================
    // Deposit on the Token-2022 + TransferFeeConfig bank.
    // Drives marginfi's `transfer_checked_with_fee` codepath through
    // `LendingAccountDeposit` on every flow hit; `bank.has_transfer_fee`
    // (set on the t22_bank) tells the helper to relax exact-amount /
    // conservation invariants while keeping share-direction strict.
    #[flow(weight = 3)]
    fn flow_t22_deposit(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_deposit(
            amount,
            self.t22_bank.clone(),
            user.t22_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("T22-fee deposit for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Deposit on the Isolated-tier bank. `asset_weight = 0` so the
    // deposit contributes no collateral; the value of this flow is in
    // giving users a position on the isolated bank that later
    // cross-bank ops will collide with, surfacing
    // `IsolatedAccountIllegalState` (6029).
    #[flow(weight = 3)]
    fn flow_isolated_deposit(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_deposit(
            amount,
            self.isolated_bank.clone(),
            user.isolated_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("Isolated deposit for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Withdraw from the Token-2022 + TransferFeeConfig bank.
    #[flow(weight = 2)]
    fn flow_t22_withdraw(&mut self) {
        let amount: u64 = self.trident.random_log_uniform();
        let user = self.get_random_user();
        self.lending_account_withdraw(
            amount,
            self.t22_bank.clone(),
            user.t22_token_account,
            user.marginfi_account,
            user.address,
            Some(format!("T22-fee withdraw for {}", user.name).as_str()),
        );
    }

    // ================================================================================================
    // Bank operational-state churn — flips a random bank between
    // `Operational`, `Paused`, and `ReduceOnly` via
    // `LendingPoolConfigureBank`. Paused rejects new deposits/borrows;
    // ReduceOnly rejects new borrows while allowing repays/withdraws.
    // The downstream deposit/withdraw/borrow/repay flows already assert
    // `state-unchanged on tx failure` semantics, so this flow exercises
    // the state-machine rejection paths without needing any new
    // invariant. Bias toward `Operational` (50%) keeps the harness
    // mostly progressing; without that the random walk would lock
    // banks too often.
    #[flow(weight = 2)]
    fn flow_bank_state_change(&mut self) {
        let bank = match self.trident.random_from_range(0u8..=2) {
            0 => self.usdc_bank.address,
            1 => self.eth_bank.address,
            _ => self.btc_bank.address,
        };
        let state = match self.trident.random_from_range(0u8..=3) {
            0 | 1 => types::marginfi::BankOperationalState::Operational,
            2 => types::marginfi::BankOperationalState::Paused,
            _ => types::marginfi::BankOperationalState::ReduceOnly,
        };
        let msg = format!("Bank state churn: {:?}", state);
        self.lending_pool_configure_bank_state(bank, state, Some(msg.as_str()));
    }

    // ================================================================================================
    // Engineered bankruptcy — deposit → borrow → oracle crash → drain
    // liquidation → auto-coupled bankruptcy → oracle restore.
    //
    // Pure-random `flow_handle_bankruptcy` never reaches a `is_bankrupt`
    // precondition (0% success across 500K sequences), so the codepath
    // that drains insurance into the liquidity vault and socialises
    // residual bad debt never runs. This flow engineers the exact
    // prerequisites every time it fires:
    //
    // 1. Liquidator pre-deposits USDC so they're solvent enough to absorb
    //    the ETH liability the liquidation hands them.
    // 2. The victim (User D, by convention to minimise collision with
    //    other random flows) deposits some USDC and opens a small ETH
    //    borrow.
    // 3. ETH oracle is scaled up 1_000_000× — victim is now massively
    //    undercollateralised.
    // 4. Liquidator drains victim's USDC entirely with `asset_amount =
    //    u64::MAX` (marginfi clamps to the victim's available balance);
    //    `lending_account_liquidate` already auto-couples a
    //    `LendingPoolHandleBankruptcy` ix on success — that's where
    //    bankruptcy itself runs against an actually-bankrupt balance.
    // 5. Oracle is restored so downstream flows see normal prices.
    //
    // Side-effects (drained victim, oracle scaling round-trip) are
    // localised: the helpers all reset to prior balances on failure, the
    // bankruptcy ix closes the victim's ETH balance to zero, and the
    // oracle is back to baseline on exit. Subsequent random flows see a
    // clean per-bank state.
    #[flow(weight = 2)]
    fn flow_engineered_bankruptcy(&mut self) {
        let victim = self.users[3].clone();
        let liquidator = self.liquidator.clone();
        let eth_oracle = constants::WETH_PYTH_PUSH;

        self.lending_account_deposit(
            usdc_amount!(1_000_000),
            self.usdc_bank.clone(),
            liquidator.usdc_token_account,
            liquidator.marginfi_account,
            liquidator.address,
            Some("[engineered bankruptcy] liquidator USDC collateral"),
        );

        self.lending_account_deposit(
            usdc_amount!(10_000),
            self.usdc_bank.clone(),
            victim.usdc_token_account,
            victim.marginfi_account,
            victim.address,
            Some("[engineered bankruptcy] victim USDC deposit"),
        );
        // 0.1 ETH — well inside victim's $10k USDC at baseline prices
        // (eth_bank liability weight is 1.85). The oracle crash below
        // multiplies the liability value, so victim ends up severely
        // underwater regardless of the absolute borrow size.
        self.lending_account_borrow(
            10_000_000,
            self.eth_bank.clone(),
            victim.eth_token_account,
            victim.marginfi_account,
            victim.address,
            Some("[engineered bankruptcy] victim ETH borrow"),
        );

        self.scale_pyth_push_oracle_prices(&eth_oracle, 1_000_000, 1);

        // marginfi's `liquidate` rejects `asset_amount > pre_balance`
        // (see `programs/marginfi/src/instructions/marginfi_account/
        // liquidate.rs:318`). Read the victim's actual USDC asset
        // balance and pass slightly less so the bank's own pre-ix
        // accrue can't push pre_balance below our amount. The 1000
        // native residual (~$0.001) is well under BANKRUPT_THRESHOLD
        // ($0.1), so the auto-coupled handle_bankruptcy ix still sees
        // a bankrupt balance.
        let drain = self
            .read_user_bank_asset_amount(victim.marginfi_account, self.usdc_bank.address)
            .saturating_sub(1000);

        self.lending_account_liquidate(
            drain,
            self.usdc_bank.clone(),
            self.eth_bank.clone(),
            liquidator.marginfi_account,
            liquidator.address,
            victim.marginfi_account,
            Some("[engineered bankruptcy] drain liquidation"),
        );

        self.scale_pyth_push_oracle_prices(&eth_oracle, 1, 1_000_000);
    }

    // ================================================================================================
    // Handle Bankruptcy — random user / random bank
    //
    // Most calls fail with `AccountNotBankrupt` (6013) because the random
    // target isn't actually bankrupt; that's the desired fuzz behaviour —
    // the codepath is exercised and the state-unchanged check on failure
    // covers the common case. Rare successes (after a deep liquidation
    // leaves a balance with bad debt) drive the real insurance-vault →
    // liquidity-vault socialisation flow.
    #[flow(weight = 2)]
    fn flow_handle_bankruptcy(&mut self) {
        let user = self.get_random_user();
        let bank = match self.trident.random_from_range(0u8..=2) {
            0 => self.usdc_bank.clone(),
            1 => self.eth_bank.clone(),
            _ => self.btc_bank.clone(),
        };
        self.lending_pool_handle_bankruptcy(
            bank,
            user.marginfi_account,
            Some(format!("Handle bankruptcy attempt: {}", user.name).as_str()),
        );
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
        // Tolerance scales with the bank's per-sequence accrue count —
        // each accrue applies I80F48 rounding at ≈ 2⁻⁴⁸ precision, which
        // empirically maps to ≈ 1 native unit of drift per accrue. The
        // 2× factor leaves headroom for both rounding directions and
        // the `max(2, …)` floor covers the trivial cases (a bank that
        // saw zero or one accrue can still drift by a fraction of a
        // unit from the final accrue itself).
        for bank in banks {
            let count = self.accrue_counts.get(&bank).copied().unwrap_or(0);
            let tolerance = I80F48::from_num((u64::from(count) * 2).max(2));
            invariants::assert_bank_solvency(&mut self.trident, bank, tolerance);
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

        // Per-bank directional invariants: share-value monotonicity
        // (liability always, asset unless bankruptcy fired) and fee-
        // bucket monotonicity. Captures any code path that secretly
        // writes share-values down or drains outstanding-fee buckets.
        invariants::assert_bank_directional_invariants(
            &mut self.trident,
            &banks,
            &self.bank_baselines,
            &self.banks_with_bankruptcy,
        );

        // Global-consistency cross-check: sum of per-user shares must
        // not exceed bank totals. Complements `position_counts`'s
        // balance-count check by also verifying the value side.
        invariants::assert_cumulative_shares_within_totals(
            &mut self.trident,
            &banks,
            &marginfi_accounts,
        );

        // Variable-borrow premium invariants: per-balance receivable sanity (non-negative,
        // zero without a liability, bounded by the configured cap) and per-bank collected
        // counter sanity.
        invariants::assert_premium_invariants(&mut self.trident, &banks, &marginfi_accounts);

        // Transient + admin-set account-flag invariants — none of
        // `ACCOUNT_DISABLED`, `ACCOUNT_IN_FLASHLOAN`,
        // `ACCOUNT_IN_RECEIVERSHIP`, `ACCOUNT_IN_DELEVERAGE`,
        // `ACCOUNT_FROZEN`, or `ACCOUNT_IN_ORDER_EXECUTION` should be
        // set at sequence end (transients must self-clear, admin
        // bits are never toggled by the harness).
        invariants::assert_marginfi_accounts_have_no_transient_flags(
            &mut self.trident,
            &marginfi_accounts,
        );

        // Group-ownership consistency — every harness-tracked
        // account still belongs to our marginfi_group.
        invariants::assert_marginfi_accounts_group_unchanged(
            &mut self.trident,
            &marginfi_accounts,
            self.marginfi_group,
        );

        // No marginfi account should carry two active balances for
        // the same bank — the program's `find_or_create` is the only
        // path that opens a balance and it re-uses the existing slot.
        invariants::assert_no_duplicate_bank_balances(&mut self.trident, &marginfi_accounts);

        // Group + fee-state immutability — no admin / config / pause
        // ix in the harness's flow set, so both accounts must be
        // byte-for-byte identical to their end-of-init snapshot.
        if let Some(snap) = &self.marginfi_group_snapshot {
            let snap = snap.clone();
            invariants::assert_account_data_unchanged(&mut self.trident, &snap);
        }
        if let Some(snap) = &self.fee_state_snapshot {
            let snap = snap.clone();
            invariants::assert_account_data_unchanged(&mut self.trident, &snap);
        }
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
