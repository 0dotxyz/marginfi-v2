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
