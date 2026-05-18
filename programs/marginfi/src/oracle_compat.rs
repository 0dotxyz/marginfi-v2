pub mod pyth {
    use anchor_lang::prelude::*;

    pub type FeedId = [u8; 32];

    pub const PYTH_PUSH_ORACLE_ID: Pubkey = pubkey!("pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT");

    pub fn id() -> Pubkey {
        PYTH_PUSH_ORACLE_ID
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, PartialEq, Debug)]
    pub enum VerificationLevel {
        Partial { num_signatures: u8 },
        Full,
    }

    impl VerificationLevel {
        pub fn gte(&self, other: VerificationLevel) -> bool {
            match self {
                VerificationLevel::Full => true,
                VerificationLevel::Partial { num_signatures } => match other {
                    VerificationLevel::Full => false,
                    VerificationLevel::Partial {
                        num_signatures: other_num_signatures,
                    } => *num_signatures >= other_num_signatures,
                },
            }
        }
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, PartialEq, Debug)]
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

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
    pub struct PriceUpdateV2 {
        pub write_authority: Pubkey,
        pub verification_level: VerificationLevel,
        pub price_message: PriceFeedMessage,
        pub posted_slot: u64,
    }

    const PRICE_UPDATE_V2_DISCRIMINATOR: [u8; 8] = [34, 241, 35, 99, 157, 126, 244, 205];

    impl anchor_lang::Discriminator for PriceUpdateV2 {
        const DISCRIMINATOR: &'static [u8] = &PRICE_UPDATE_V2_DISCRIMINATOR;
    }

    #[derive(Copy, Clone, Debug, PartialEq)]
    pub struct Price {
        pub price: i64,
        pub conf: u64,
        pub exponent: i32,
        pub publish_time: i64,
    }

    #[derive(Copy, Clone, Debug, PartialEq)]
    pub enum GetPriceError {
        PriceTooOld,
        InvalidWindowSize,
        MismatchedFeedId,
        InsufficientVerificationLevel,
        FeedIdMustBe32Bytes,
        FeedIdNonHexCharacter,
    }

    impl PriceUpdateV2 {
        pub fn get_price_unchecked(
            &self,
            feed_id: &FeedId,
        ) -> core::result::Result<Price, GetPriceError> {
            if self.price_message.feed_id != *feed_id {
                return Err(GetPriceError::MismatchedFeedId);
            }

            Ok(Price {
                price: self.price_message.price,
                conf: self.price_message.conf,
                exponent: self.price_message.exponent,
                publish_time: self.price_message.publish_time,
            })
        }

        pub fn get_price_no_older_than_with_custom_verification_level(
            &self,
            clock: &Clock,
            maximum_age: u64,
            feed_id: &FeedId,
            verification_level: VerificationLevel,
        ) -> core::result::Result<Price, GetPriceError> {
            if !self.verification_level.gte(verification_level) {
                return Err(GetPriceError::InsufficientVerificationLevel);
            }

            let price = self.get_price_unchecked(feed_id)?;
            if price
                .publish_time
                .saturating_add(maximum_age.try_into().unwrap())
                < clock.unix_timestamp
            {
                return Err(GetPriceError::PriceTooOld);
            }

            Ok(price)
        }
    }
}

pub mod switchboard {
    use anchor_lang::prelude::Pubkey;

    pub const PRECISION: u32 = 18;
    pub const PULL_FEED_DISCRIMINATOR: [u8; 8] = [196, 27, 108, 196, 10, 215, 219, 40];

    #[repr(C)]
    #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct CurrentResult {
        pub value: i128,
        pub std_dev: i128,
        pub mean: i128,
        pub range: i128,
        pub min_value: i128,
        pub max_value: i128,
        pub num_samples: u8,
        pub submission_idx: u8,
        pub padding1: [u8; 6],
        pub slot: u64,
        pub min_slot: u64,
        pub max_slot: u64,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct OracleSubmission {
        pub oracle: Pubkey,
        pub slot: u64,
        pub landed_at: u64,
        pub value: i128,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct CompactResult {
        pub std_dev: f32,
        pub mean: f32,
        pub slot: u64,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PullFeedAccountData {
        pub submissions: [OracleSubmission; 32],
        pub authority: Pubkey,
        pub queue: Pubkey,
        pub feed_hash: [u8; 32],
        pub initialized_at: i64,
        pub permissions: u64,
        pub max_variance: u64,
        pub min_responses: u32,
        pub name: [u8; 32],
        pub padding1: [u8; 2],
        pub historical_result_idx: u8,
        pub min_sample_size: u8,
        pub last_update_timestamp: i64,
        pub lut_slot: u64,
        pub reserved1: [u8; 32],
        pub result: CurrentResult,
        pub max_staleness: u32,
        pub padding2: [u8; 12],
        pub historical_results: [CompactResult; 32],
        pub ebuf4: [u8; 8],
        pub ebuf3: [u8; 24],
        pub submission_timestamps: [i64; 32],
    }
}
