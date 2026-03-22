use anchor_lang::prelude::*;
#[allow(deprecated)]
use solana_program::borsh0_10::try_from_slice_unchecked;

use pyth_solana_receiver_sdk::{
    error::GetPriceError,
    price_update::{Price as PythPrice, PriceUpdateV2, VerificationLevel},
};

use crate::constants::*;
use crate::errors::ErrorCodes;
use crate::state::Price;

use library::math::casting::Cast;
use library::math::safe_math::SafeMath;

pub fn read_pyth_source(
    pyth_price_update: &AccountInfo,
    is_liquidate: Option<bool>,
) -> Result<Price> {
    let price_update_data = pyth_price_update.data.borrow();

    #[allow(deprecated)]
    let price_update = try_from_slice_unchecked::<PriceUpdateV2>(&price_update_data.as_ref()[8..])?;

    if !price_update.verification_level.gte(VerificationLevel::Full) {
        return err!(ErrorCodes::PriceNotValid);
    }

    let maximum_age = if is_liquidate.is_some() && is_liquidate.unwrap() {
        MAX_AGE_LIQUIDATE
    } else {
        MAX_AGE_OPERATE
    };

    let sdk_price = price_update
        .get_price_no_older_than(
            &Clock::get()?,
            maximum_age,
            &price_update.price_message.feed_id,
        )
        .map_err(|err| -> ProgramError { map_pyth_get_price_error(err).into() })?;

    let PythPrice {
        price,
        conf,
        publish_time,
        exponent,
    } = sdk_price;

    // if price is 0 or exponent is greater than 0, return error
    if price <= 0 || exponent > 0 {
        return err!(ErrorCodes::PriceNotValid);
    }

    if is_liquidate.is_some() && is_liquidate.unwrap() {
        if conf.safe_mul(CONFIDENCE_SCALE_FACTOR_LIQUIDATE)? > price.abs() as u64 {
            return err!(ErrorCodes::PriceConfidenceNotSufficient);
        }
    } else {
        if conf.safe_mul(CONFIDENCE_SCALE_FACTOR_OPERATE)? > price.abs() as u64 {
            return err!(ErrorCodes::PriceConfidenceNotSufficient);
        }
    }

    Ok(Price {
        price: price.cast()?,
        exponent: Some(exponent.abs().cast()?),
    })
}

fn map_pyth_get_price_error(err: GetPriceError) -> Error {
    match err {
        GetPriceError::PriceTooOld => Error::from(ErrorCodes::PriceTooOld),
        GetPriceError::MismatchedFeedId
        | GetPriceError::FeedIdMustBe32Bytes
        | GetPriceError::FeedIdNonHexCharacter => Error::from(ErrorCodes::InvalidSource),
        GetPriceError::InsufficientVerificationLevel => Error::from(ErrorCodes::PriceNotValid),
        GetPriceError::InvalidWindowSize => Error::from(ErrorCodes::PriceTooOld),
    }
}
