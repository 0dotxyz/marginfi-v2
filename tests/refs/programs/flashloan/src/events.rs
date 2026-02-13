use anchor_lang::prelude::*;

#[event]
pub struct PauseProtocol {}

#[event]
pub struct ActivateProtocol {}

#[event]
pub struct SetFlashloanFee {
    pub flashloan_fee: u16,
}

#[event]
pub struct LogUpdateAuthority {
    pub new_authority: Pubkey,
}
