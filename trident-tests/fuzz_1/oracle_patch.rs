//! Patch forked Pyth **push** oracle accounts (`PriceUpdateV2`) using the same layout marginfi
//! reads after the 8-byte Anchor discriminator (`load_price_update_v2_checked` in
//! `programs/marginfi/src/state/price.rs`).
//!
//! We avoid `pyth-solana-receiver-sdk` / `anchor-lang` derives here: Trident’s dependency graph
//! uses `borsh` versions that do not match those crates’ derive expectations in this standalone
//! package.
//!
//! Layout verified against fork cache `42amVS4KgzR9rA28tkVYqVXjq9Qa8dcZQMbH5EYFX6XC.json`:
//! `write_authority` (32) + `verification_level` (1 or 2 bytes) + `PriceFeedMessage` (84) +
//! `posted_slot` (8).

use trident_fuzz::fuzzing::*;

use crate::FuzzTest;

const WRITE_AUTHORITY_END: usize = 32;

fn verification_level_size(body: &[u8]) -> usize {
    match body.get(WRITE_AUTHORITY_END).copied() {
        Some(0) => 2, // `Partial { num_signatures: u8 }`
        Some(1) => 1, // `Full`
        _ => panic!("unknown Pyth VerificationLevel discriminant"),
    }
}

/// Byte range of `PriceFeedMessage` inside the `PriceUpdateV2` **body** (after 8-byte disc).
fn price_message_range(body: &[u8]) -> std::ops::Range<usize> {
    let vlen = verification_level_size(body);
    let start = WRITE_AUTHORITY_END + vlen;
    let end = start + 84;
    assert!(
        body.len() >= end,
        "oracle account body too small for PriceFeedMessage"
    );
    start..end
}

// Offsets inside `PriceFeedMessage` (see `pythnet_sdk::messages::PriceFeedMessage`).
const PM_PRICE: std::ops::Range<usize> = 32..40;
const PM_CONF: std::ops::Range<usize> = 40..48;
const PM_PUBLISH: std::ops::Range<usize> = 52..60;
const PM_PREV_PUBLISH: std::ops::Range<usize> = 60..68;
const PM_EMA_PRICE: std::ops::Range<usize> = 68..76;
const PM_EMA_CONF: std::ops::Range<usize> = 76..84;

fn scale_i64_bytes(r: &mut [u8], num: i64, den: i64) {
    let v = i64::from_le_bytes(r.try_into().unwrap());
    let out = (v as i128).saturating_mul(num as i128) / den as i128;
    let out = out.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
    r.copy_from_slice(&out.to_le_bytes());
}

fn scale_u64_bytes(r: &mut [u8], num: i64, den: i64) {
    let v = u64::from_le_bytes(r.try_into().unwrap());
    let out = (v as u128).saturating_mul(num as u128) / den as u128;
    let out = u64::try_from(out.min(u128::from(u64::MAX))).unwrap_or(u64::MAX);
    r.copy_from_slice(&out.to_le_bytes());
}

impl FuzzTest {
    /// Read–modify–write the `PriceFeedMessage` portion of a Pyth push oracle (spot + EMA prices
    /// and confidences). Refreshes publish timestamps from the Trident clock.
    pub fn patch_pyth_push_price_message(&mut self, oracle: &Pubkey, mut f: impl FnMut(&mut [u8])) {
        let mut account = self.trident.get_account(oracle);
        let mut buf = account.data().to_vec();
        assert!(buf.len() >= 8, "oracle account missing discriminator");

        let body_len = buf.len() - 8;
        let body = &mut buf[8..];
        assert!(
            body_len >= WRITE_AUTHORITY_END + 1 + 84 + 8,
            "oracle account too small for PriceUpdateV2"
        );

        let range = price_message_range(body);
        let pm = &mut body[range];
        f(pm);

        let ts = self.trident.get_current_timestamp();
        pm[PM_PUBLISH].copy_from_slice(&ts.to_le_bytes());
        pm[PM_PREV_PUBLISH].copy_from_slice(&ts.to_le_bytes());

        account.set_data_from_slice(&buf);
        self.trident.set_account_custom(oracle, &account);
    }

    /// Scale spot + EMA price and confidences by `num / den` (e.g. `2`, `1` doubles).
    pub fn scale_pyth_push_oracle_prices(&mut self, oracle: &Pubkey, num: i64, den: i64) {
        assert!(den > 0, "scale denominator must be positive");
        self.patch_pyth_push_price_message(oracle, |pm| {
            scale_i64_bytes(&mut pm[PM_PRICE], num, den);
            scale_u64_bytes(&mut pm[PM_CONF], num, den);
            scale_i64_bytes(&mut pm[PM_EMA_PRICE], num, den);
            scale_u64_bytes(&mut pm[PM_EMA_CONF], num, den);
        });
    }
}
