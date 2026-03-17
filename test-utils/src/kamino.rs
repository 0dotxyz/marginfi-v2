use anchor_lang::{Discriminator, ZeroCopy};
use bytemuck::{from_bytes, Pod};
use kamino_mocks::state::{MinimalObligation, MinimalReserve};
use solana_program_test::ProgramTestContext;
use solana_sdk::account::ReadableAccount;
use std::{cell::RefCell, rc::Rc};

use crate::utils::load_account_from_file;

#[derive(Clone)]
pub struct KaminoFixture {
    pub reserve: MinimalReserve,
    pub obligation: MinimalObligation,
}

impl KaminoFixture {
    pub fn new_from_files(
        ctx: Rc<RefCell<ProgramTestContext>>,
        reserve_json_path: &str,
        obligation_json_path: &str,
    ) -> Self {
        let (reserve_key, reserve_acc) = load_account_from_file(reserve_json_path);
        let (obligation_key, obligation_acc) = load_account_from_file(obligation_json_path);

        let mut c = ctx.borrow_mut();
        c.set_account(&reserve_key, &reserve_acc);
        c.set_account(&obligation_key, &obligation_acc);

        let reserve = parse_zero_copy_account::<MinimalReserve>(reserve_acc.data());
        let obligation = parse_zero_copy_account::<MinimalObligation>(obligation_acc.data());

        Self {
            reserve,
            obligation,
        }
    }
}

fn parse_zero_copy_account<T>(data: &[u8]) -> T
where
    T: Discriminator + ZeroCopy + Pod,
{
    let disc = T::DISCRIMINATOR;
    assert!(
        data.len() >= disc.len(),
        "account data too short for discriminator"
    );
    assert_eq!(
        &data[..disc.len()],
        disc,
        "unexpected account discriminator"
    );
    assert!(
        data.len() >= disc.len() + core::mem::size_of::<T>(),
        "account data too short for zero-copy payload"
    );

    // For zero-copy account types, account payload starts right after 8-byte discriminator.
    *from_bytes::<T>(&data[disc.len()..disc.len() + core::mem::size_of::<T>()])
}
