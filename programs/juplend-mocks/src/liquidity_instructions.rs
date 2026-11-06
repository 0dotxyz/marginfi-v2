//! Discriminators for Liquidity instructions used by integration tests.

pub const CHANGE_STATUS_DISCRIMINATOR: [u8; 8] = [236, 145, 131, 228, 227, 17, 192, 255];
pub const PAUSE_TOKEN_DISCRIMINATOR: [u8; 8] = [226, 150, 72, 211, 159, 51, 226, 39];
pub const UNPAUSE_TOKEN_DISCRIMINATOR: [u8; 8] = [108, 117, 62, 30, 200, 92, 255, 202];
pub const UPDATE_GUARDIANS_DISCRIMINATOR: [u8; 8] = [43, 62, 250, 138, 141, 117, 132, 97];
pub const UPDATE_USER_CLASS_DISCRIMINATOR: [u8; 8] = [12, 206, 68, 135, 63, 212, 48, 119];
pub const PAUSE_USER_DISCRIMINATOR: [u8; 8] = [18, 63, 43, 94, 239, 53, 101, 14];
pub const UNPAUSE_USER_DISCRIMINATOR: [u8; 8] = [71, 115, 128, 252, 182, 126, 234, 62];
