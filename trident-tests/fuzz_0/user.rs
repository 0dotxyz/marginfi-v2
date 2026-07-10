use trident_fuzz::fuzzing::*;

#[derive(Clone)]
pub struct User {
    pub name: String,
    pub address: Pubkey,
    pub marginfi_account: Pubkey,
    pub usdc_token_account: Pubkey,
    pub initial_usdc_amount: u64,
    pub eth_token_account: Pubkey,
    pub initial_eth_amount: u64,
    pub btc_token_account: Pubkey,
    pub initial_btc_amount: u64,
    /// Token-2022 with TransferFeeConfig — per-user slot enabling
    /// `flow_t22_deposit` / `flow_t22_withdraw` to exercise marginfi's
    /// transfer-fee-aware deposit and withdraw paths on every user.
    pub t22_token_account: Pubkey,
    pub initial_t22_amount: u64,
    /// Isolated-risk-tier asset (asset_weight = 0). Mixing a position on an
    /// isolated bank with positions on default banks triggers
    /// `IsolatedAccountIllegalState` (6029) — the fuzz harness exercises
    /// this naturally via the random cross-bank interactions.
    pub isolated_token_account: Pubkey,
    pub initial_isolated_amount: u64,
}

impl User {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        address: Pubkey,
        marginfi_account: Pubkey,
        usdc_token_account: Pubkey,
        initial_usdc_amount: u64,
        eth_token_account: Pubkey,
        initial_eth_amount: u64,
        btc_token_account: Pubkey,
        initial_btc_amount: u64,
        t22_token_account: Pubkey,
        initial_t22_amount: u64,
        isolated_token_account: Pubkey,
        initial_isolated_amount: u64,
    ) -> Self {
        Self {
            name,
            address,
            marginfi_account,
            usdc_token_account,
            initial_usdc_amount,
            eth_token_account,
            initial_eth_amount,
            btc_token_account,
            initial_btc_amount,
            t22_token_account,
            initial_t22_amount,
            isolated_token_account,
            initial_isolated_amount,
        }
    }
}
