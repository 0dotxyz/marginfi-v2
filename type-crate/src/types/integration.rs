use crate::constants::{ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOLEND};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

/// Identifies which integration protocol is being used.
/// Passed into the unified instruction so the operation type is readable from instruction args.
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum IntegrationOpMode {
    Kamino = 0,
    Drift = 1,
    Solend = 2,
    JupLend = 3,
}

impl IntegrationOpMode {
    pub fn to_asset_tag(self) -> u8 {
        match self {
            Self::Kamino => ASSET_TAG_KAMINO,
            Self::Drift => ASSET_TAG_DRIFT,
            Self::Solend => ASSET_TAG_SOLEND,
            Self::JupLend => ASSET_TAG_JUPLEND,
        }
    }
}
