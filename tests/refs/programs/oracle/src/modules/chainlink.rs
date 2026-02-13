use anchor_lang::prelude::*;

use chainlink_solana::v2::read_feed_v2;

use crate::errors::ErrorCodes;
use crate::helper::verify_publish_time_with_slot;
use crate::state::Price;
use library::math::casting::Cast;

pub fn read_chainlink_source(
    chainlink_feed: &AccountInfo,
    is_liquidate: Option<bool>,
) -> Result<Price> {
    let feed = read_feed_v2(
        chainlink_feed.try_borrow_data()?,
        chainlink_feed.owner.to_bytes(),
    )
    .map_err(|_| error!(ErrorCodes::ChainlinkPriceReadError))?;

    let data = feed
        .latest_round_data()
        .ok_or(error!(ErrorCodes::ChainlinkPriceReadError))?;

    verify_publish_time_with_slot(data.slot, is_liquidate)?;

    Ok(Price {
        price: data.answer.cast()?,
        exponent: Some(feed.decimals()),
    })
}
