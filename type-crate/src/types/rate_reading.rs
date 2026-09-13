use crate::{assert_struct_align, assert_struct_size};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};
use fixed::types::I80F48;

/// Low I80F48 bits an encoded index drops, leaving 32 fractional bits and a 2^32 integer range.
const INDEX_DROPPED_BITS: u32 = 16;

assert_struct_size!(RateReading, 24);
assert_struct_align!(RateReading, 8);
/// A bank's share indices at one instant, which an interest-trigger order measures realized rates
/// from. See `Bank::rate_readings`.
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct RateReading {
    /// `asset_share_value` times the venue exchange multiplier, as I80F48 bits with the low
    /// `INDEX_DROPPED_BITS` removed.
    pub asset_index: u64,
    /// `liability_share_value` times the venue exchange multiplier, encoded like `asset_index`.
    pub debt_index: u64,
    /// Unix seconds the reading was taken. Zero in a slot never written.
    pub timestamp: i64,
}

impl RateReading {
    /// `None` when an index does not fit the encoding: negative, or 2^32 and above.
    pub fn new(asset_index: I80F48, debt_index: I80F48, timestamp: i64) -> Option<Self> {
        Some(Self {
            asset_index: encode_index(asset_index)?,
            debt_index: encode_index(debt_index)?,
            timestamp,
        })
    }

    pub fn asset_index(&self) -> I80F48 {
        decode_index(self.asset_index)
    }

    pub fn debt_index(&self) -> I80F48 {
        decode_index(self.debt_index)
    }
}

fn encode_index(index: I80F48) -> Option<u64> {
    u64::try_from(index.to_bits() >> INDEX_DROPPED_BITS).ok()
}

fn decode_index(bits: u64) -> I80F48 {
    I80F48::from_bits(i128::from(bits) << INDEX_DROPPED_BITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_round_trip_less_the_dropped_bits() {
        let exact = I80F48::from_num(1.0625);
        let inexact = I80F48::from_num(7.89);
        let reading = RateReading::new(exact, inexact, 1_700_000_000).unwrap();
        assert_eq!(reading.asset_index(), exact);
        assert_eq!(
            reading.debt_index(),
            I80F48::from_bits(inexact.to_bits() >> INDEX_DROPPED_BITS << INDEX_DROPPED_BITS)
        );
    }

    #[test]
    fn an_index_outside_the_encoding_is_rejected() {
        let ok = I80F48::from_num(u32::MAX);
        assert!(RateReading::new(ok, ok, 1).is_some());
        let too_big = ok + I80F48::ONE;
        assert!(RateReading::new(too_big, ok, 1).is_none());
        assert!(RateReading::new(ok, too_big, 1).is_none());
        assert!(RateReading::new(-I80F48::DELTA, ok, 1).is_none());
    }
}
