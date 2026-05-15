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
        }
    }
}
