use borsh::BorshDeserialize;
use borsh::BorshSerialize;
use trident_fuzz::fuzzing::*;

use crate::FuzzTest;

fn scale_i64(v: i64, num: i64, den: i64) -> i64 {
    let out = (v as i128).saturating_mul(num as i128) / den as i128;
    out.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn scale_u64(v: u64, num: i64, den: i64) -> u64 {
    let out = (v as u128).saturating_mul(num as u128) / den as u128;
    u64::try_from(out.min(u128::from(u64::MAX))).unwrap_or(u64::MAX)
}

impl FuzzTest {
    pub fn patch_pyth_push_price_message(
        &mut self,
        oracle: &Pubkey,
        mut f: impl FnMut(&mut PriceFeedMessage),
    ) {
        let mut account = self.trident.get_account(oracle);
        let mut account_data = account.data().to_vec();

        let body = &account_data[8..];
        let mut rest: &[u8] = body;
        let mut update = PriceUpdateV2::deserialize(&mut rest).expect("deserialize PriceUpdateV2");
        let trailing = rest.to_vec();

        f(&mut update.price_message);

        let mut updated_data = Vec::new();
        update
            .serialize(&mut updated_data)
            .expect("serialize PriceUpdateV2");
        updated_data.extend_from_slice(&trailing);
        account_data[8..].copy_from_slice(&updated_data);

        account.set_data_from_slice(&account_data);
        self.trident.set_account_custom(oracle, &account);
    }

    pub fn scale_pyth_push_oracle_prices(&mut self, oracle: &Pubkey, num: i64, den: i64) {
        let timestamp = self.trident.get_current_timestamp();
        assert!(den > 0, "scale denominator must be positive");
        self.patch_pyth_push_price_message(oracle, |pm| {
            pm.price = scale_i64(pm.price, num, den);
            pm.conf = scale_u64(pm.conf, num, den);
            pm.ema_price = scale_i64(pm.ema_price, num, den);
            pm.ema_conf = scale_u64(pm.ema_conf, num, den);
            pm.publish_time = timestamp;
            pm.prev_publish_time = timestamp;
        });
    }

    pub fn update_pyth_timestamp(&mut self, oracle: &Pubkey, timestamp: i64) {
        self.patch_pyth_push_price_message(oracle, |pm| {
            pm.publish_time = timestamp;
            pm.prev_publish_time = timestamp;
        });
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct PriceUpdateV2 {
    pub write_authority: Pubkey,
    pub verification_level: VerificationLevel,
    pub price_message: PriceFeedMessage,
    pub posted_slot: u64,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub enum VerificationLevel {
    Partial {
        #[allow(unused)]
        num_signatures: u8,
    },
    Full,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct PriceFeedMessage {
    pub feed_id: [u8; 32],
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    pub publish_time: i64,
    pub prev_publish_time: i64,
    pub ema_price: i64,
    pub ema_conf: u64,
}
