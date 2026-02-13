use anchor_lang::prelude::*;
#[allow(deprecated)]
use solana_program::borsh0_10::try_from_slice_unchecked;

use crate::constants::MILLISECONDS_PER_SECOND;
use crate::errors::ErrorCodes;
use crate::helper::verify_publish_time;
use crate::state::schema::{RedstoneFeed, U256_BYTE_SIZE, U64_START_INDEX};
use crate::state::Price;
use library::math::casting::Cast;

fn get_price(raw_be_value: &[u8; U256_BYTE_SIZE]) -> Result<u128> {
    if !raw_be_value.iter().take(U64_START_INDEX).all(|&v| v == 0) {
        return Err(error!(ErrorCodes::RedstonePriceOverflow));
    }

    let value = u64::from_be_bytes(raw_be_value[U64_START_INDEX..].try_into().unwrap());

    Ok(value.cast()?)
}

pub fn read_redstone_source(
    redstone_feed: &AccountInfo,
    is_liquidate: Option<bool>,
) -> Result<Price> {
    #[allow(deprecated)]
    // @dev using try_from_slice_unchecked as we changed the discriminator which was PriceData to RedstoneFeed
    let redstone_feed =
        try_from_slice_unchecked::<RedstoneFeed>(&redstone_feed.data.borrow()[8..])?;

    let price = get_price(&redstone_feed.value)?;

    let publish_time: u64 = if redstone_feed.write_timestamp.is_some() {
        // return minimum of timestamp and write_timestamp
        redstone_feed
            .timestamp
            .min(redstone_feed.write_timestamp.unwrap())
            .cast()?
    } else {
        return err!(ErrorCodes::TimestampExpected);
    };

    verify_publish_time(publish_time / MILLISECONDS_PER_SECOND, is_liquidate)?;

    Ok(Price {
        price,
        exponent: Some(redstone_feed.decimals.cast()?),
    })
}
