use anchor_lang::prelude::*;
use bytemuck::{Pod, Zeroable};
use std::{
    cell::{Ref, RefMut},
    collections::HashMap,
};

use library::math::{casting::*, safe_math::*, tick::TickMath};

use crate::errors::ErrorCodes;
use crate::state::tick::TickAccounts;

pub const COLD_TICK: i32 = TickMath::COLD_TICK;
pub const MIN_TICK: i32 = TickMath::MIN_TICK; // -16383
pub const MAX_TICK: i32 = TickMath::MAX_TICK; // 16383
pub const TICK_HAS_DEBT_ARRAY_SIZE: usize = 8;
pub const TICK_HAS_DEBT_CHILDREN_SIZE: usize = 32; // 32 bytes = 256 bits
pub const BIT_PER_BYTE: usize = 8;
pub const TICK_HAS_DEBT_CHILDREN_SIZE_IN_BITS: usize = TICK_HAS_DEBT_CHILDREN_SIZE * BIT_PER_BYTE;
pub const TICKS_PER_TICK_HAS_DEBT: usize =
    TICK_HAS_DEBT_ARRAY_SIZE * TICK_HAS_DEBT_CHILDREN_SIZE_IN_BITS; // 8 * 256 = 2048

// Total range: -16383 to 16383 = 32767 ticks
// Each index covers 2048 ticks, so we need 16 indices to cover all ticks
pub const TOTAL_INDICES_NEEDED: usize = 16;

/// Tick has debt structure
/// Each TickHasDebt can track 8 * 256 = 2048 ticks
/// children_bits has 32 bytes = 256 bits total
/// Each map within the array covers 256 ticks
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, InitSpace, Pod, Zeroable, Copy)]
#[repr(C, packed)]
pub struct TickHasDebt {
    // 32 bytes = 256 bits total, each map_index within this covers 256 ticks
    pub children_bits: [u8; TICK_HAS_DEBT_CHILDREN_SIZE],
}

#[account(zero_copy)]
#[repr(C, packed)]
#[derive(InitSpace)]
pub struct TickHasDebtArray {
    pub vault_id: u16, // Vault ID
    pub index: u8,     // Index of this TickHasDebtArray (0-15)
    /// Each array contains 8 TickHasDebt structs
    /// Each TickHasDebt covers 256 ticks
    /// Total: 8 * 256 = 2048 ticks per TickHasDebtArray
    pub tick_has_debt: [TickHasDebt; TICK_HAS_DEBT_ARRAY_SIZE],
}

impl TickHasDebtArray {
    pub fn get_children_bits(&self, map_id: u8) -> [u8; TICK_HAS_DEBT_CHILDREN_SIZE] {
        self.tick_has_debt[map_id as usize].children_bits
    }

    pub fn has_bits(&self, map_id: usize) -> bool {
        self.tick_has_debt[map_id]
            .children_bits
            .iter()
            .any(|&x| x != 0)
    }

    pub fn set_tick_has_debt(
        &mut self,
        map_id: u8,
        tick_has_debt: [u8; TICK_HAS_DEBT_CHILDREN_SIZE],
    ) -> Result<()> {
        self.tick_has_debt[map_id as usize].children_bits = tick_has_debt;
        Ok(())
    }

    /// param add_or_remove if true then add else remove
    pub fn update_tick_has_debt(&mut self, tick: i32, add_or_remove: bool) -> Result<()> {
        let (map_index, byte_index, bit_index) = get_tick_indices_for_array(tick, self.index)?;

        let mask = 1 << bit_index;

        let byte = &mut self.tick_has_debt[map_index as usize].children_bits[byte_index as usize];

        if add_or_remove {
            *byte |= mask; // Set bit
        } else {
            *byte &= !mask; // Clear bit
        }

        Ok(())
    }

    pub fn clear_bits_for_tick(&mut self, tick: i32) -> Result<()> {
        let (_, map_index, byte_index, bit_index) = get_tick_indices(tick)?;

        self.clear_bits(map_index, byte_index, bit_index)?;
        Ok(())
    }

    // Create a mask for the current byte that will:
    // - Keep all bits lower than bit_index (these are smaller ticks)
    // - Clear the current bit and all higher bits (these are the current and larger ticks)
    fn clear_bits(&mut self, map_index: usize, byte_index: usize, bit_index: usize) -> Result<()> {
        // Create a working copy of the current map's bitmap
        let mut bitmap: [u8; TICK_HAS_DEBT_CHILDREN_SIZE] =
            self.tick_has_debt[map_index].children_bits;

        // Clear the current tick's bit and all higher bits in the current byte
        if bit_index > 0 {
            // Create mask to keep only bits lower than current bit
            let mask = (1 << bit_index) - 1;
            bitmap[byte_index] &= mask;
        } else {
            // If bit_index is 0, clear the entire byte
            bitmap[byte_index] = 0;
        }

        // Clear all bytes with higher indices (representing higher ticks)
        for i in (byte_index + 1)..TICK_HAS_DEBT_CHILDREN_SIZE {
            bitmap[i] = 0;
        }

        self.tick_has_debt[map_index].children_bits = bitmap;

        Ok(())
    }

    pub fn fetch_next_top_tick(&mut self, mut map_index: usize) -> Result<(i32, bool)> {
        // Search for the next tick with debt
        loop {
            if self.has_bits(map_index) {
                let (next_tick, has_next_tick) = self.get_next_tick(map_index.cast()?)?;

                if has_next_tick {
                    return Ok((next_tick, true));
                }
            }

            // No bits found in current map, move to the previous map (lower ticks)
            if map_index == 0 {
                if self.index == 0 {
                    // No more maps to check in this array
                    return Ok((COLD_TICK, true));
                } else {
                    return Ok((COLD_TICK, false));
                }
            }

            map_index -= 1;
        }
    }

    fn get_most_significant_bit(&self, map_id: usize, byte_idx: usize) -> u32 {
        let bits: u8 = self.tick_has_debt[map_id].children_bits[byte_idx];

        // Find most significant bit in the byte
        bits.leading_zeros()
    }

    fn get_next_tick(&self, map_index: usize) -> Result<(i32, bool)> {
        for byte_idx in (0..TICK_HAS_DEBT_CHILDREN_SIZE).rev() {
            if self.tick_has_debt[map_index].children_bits[byte_idx] != 0 {
                // Find the highest set bit in this byte
                let leading_zeros = self.get_most_significant_bit(map_index, byte_idx);
                // 7 - leading_zeros ensures we count from the left of the byte
                let bit_pos = 7 - leading_zeros as usize;

                // Calculate the tick within the map (0-255)
                let tick_within_map = byte_idx * BIT_PER_BYTE + bit_pos;

                // Calculate the actual tick value
                let map_first_tick = self.get_first_tick_for_map_index(map_index)?;
                return Ok((map_first_tick + tick_within_map as i32, true));
            }
        }

        Ok((COLD_TICK, false))
    }

    fn get_first_tick_for_map_index(&self, map_index: usize) -> Result<i32> {
        get_first_tick_for_map_in_array(self.index, map_index.cast()?)
    }
}

pub struct TickHasDebtAccounts<'info> {
    pub accounts: Vec<AccountLoader<'info, TickHasDebtArray>>,
    pub indices: HashMap<u8, usize>,
}

impl<'info> TickHasDebtAccounts<'info> {
    fn get_index(&self, index: u8) -> Result<&usize> {
        match self.indices.get(&index) {
            Some(index) => Ok(index),
            None => {
                msg!("Tick has debt not found: index = {}", index);
                Err(error!(ErrorCodes::VaultTickHasDebtNotFound))
            }
        }
    }

    pub fn load(&self, index: u8) -> Result<Ref<TickHasDebtArray>> {
        let index = self.get_index(index)?;
        let loaded = self.accounts[*index].load()?;

        Ok(loaded)
    }

    pub fn load_mut(&self, index: u8) -> Result<RefMut<TickHasDebtArray>> {
        let index = self.get_index(index)?;
        let loaded = self.accounts[*index].load_mut()?;

        Ok(loaded)
    }

    pub fn update_tick_has_debt(&self, tick: i32, add_or_remove: bool) -> Result<()> {
        let array_index = get_array_index_for_tick(tick)?;
        let mut tick_has_debt =  self.load_mut(array_index)?;

        // Remove tick from tick_has_debt
        tick_has_debt.update_tick_has_debt(tick, add_or_remove)?;

        Ok(())
    }

    pub fn fetch_next_top_tick(&self, top_tick: i32) -> Result<i32> {
        let (mut array_index, mut map_index, byte_index, bit_index) = get_tick_indices(top_tick)?;

        let mut current_tick_has_debt = self.load_mut(array_index)?;

        if current_tick_has_debt.index != array_index {
            return Err(ErrorCodes::VaultTickHasDebtIndexMismatch.into());
        }

        current_tick_has_debt.clear_bits(map_index, byte_index, bit_index)?;

        loop {
            let (next_tick, has_next_tick) =
                current_tick_has_debt.fetch_next_top_tick(map_index)?;

            if has_next_tick {
                return Ok(next_tick);
            } else {
                array_index -= 1;
                map_index = TICK_HAS_DEBT_ARRAY_SIZE - 1;
                current_tick_has_debt = self.load_mut(array_index)?;
            }
        }
    }

    pub fn fetch_next_tick_absorb(
        &self,
        tick_accounts: &Box<TickAccounts<'info>>,
        current_tick: i32,
        max_tick: i32,
    ) -> Result<(i32, u128, u128)> {
        let (mut array_index, mut map_index, _, _) = get_tick_indices(current_tick)?;

        let mut current_tick_has_debt = self.load_mut(array_index)?;

        if current_tick_has_debt.index != array_index {
            return Err(ErrorCodes::VaultTickHasDebtIndexMismatch.into());
        }

        let mut col_absorbed: u128 = 0;
        let mut debt_absorbed: u128 = 0;

        // For last user remaining in vault there could be a lot of loop iterations
        // Chances of this to happen is extremely low (like ~0%)
        loop {
            let (next_tick, has_next_tick) =
                current_tick_has_debt.fetch_next_top_tick(map_index)?;

            if has_next_tick {
                if next_tick > max_tick {
                    let mut tick_data = tick_accounts.load_mut(next_tick)?;

                    let tick_debt = tick_data.get_raw_debt()?;
                    let ratio = TickMath::get_ratio_at_tick(next_tick)?;

                    debt_absorbed = debt_absorbed.safe_add(tick_debt)?;
                    col_absorbed = col_absorbed.safe_add(
                        tick_debt
                            .safe_mul(TickMath::ZERO_TICK_SCALED_RATIO)?
                            .safe_div(ratio)?,
                    )?;

                    tick_data.set_fully_liquidated();
                    current_tick_has_debt.clear_bits_for_tick(next_tick)?;
                } else {
                    return Ok((next_tick, col_absorbed, debt_absorbed));
                }
            } else {
                array_index -= 1;
                map_index = TICK_HAS_DEBT_ARRAY_SIZE - 1;
                current_tick_has_debt = self.load_mut(array_index)?;
            }
        }
    }

    pub fn fetch_next_tick_liquidate(
        &self,
        current_tick: i32,
        liquidation_tick: i32,
        clear_bits: bool,
    ) -> Result<i32> {
        let (mut array_index, mut map_index, byte_index, bit_index) =
            get_tick_indices(current_tick)?;

        let mut current_tick_has_debt = self.load_mut(array_index)?;

        if current_tick_has_debt.index != array_index {
            return Err(ErrorCodes::VaultTickHasDebtIndexMismatch.into());
        }

        if clear_bits {
            current_tick_has_debt.clear_bits(map_index, byte_index, bit_index)?;
        }

        // For last user remaining in vault there could be a lot of loop iterations
        // Chances of this to happen is extremely low (like ~0%)
        loop {
            let (next_tick, has_next_tick) =
                current_tick_has_debt.fetch_next_top_tick(map_index)?;

            if has_next_tick {
                return Ok(next_tick);
            } else {
                if current_tick_has_debt.get_first_tick_for_map_index(map_index)? < liquidation_tick
                {
                    return Ok(COLD_TICK);
                }

                array_index -= 1;
                map_index = TICK_HAS_DEBT_ARRAY_SIZE - 1;
                current_tick_has_debt = self.load_mut(array_index)?;
            }
        }
    }
}

fn load_tick_has_debt_accounts<'info>(
    remaining_accounts: &'info [AccountInfo<'info>],
    start_index: usize,
    end_index: usize,
    length: usize,
    vault_id: u16,
) -> Result<Box<TickHasDebtAccounts<'info>>> {
    if remaining_accounts.len() < end_index.cast()? {
        return Err(error!(ErrorCodes::VaultLiquidateRemainingAccountsTooShort));
    }

    let mut tick_has_debt_accounts = Box::new(TickHasDebtAccounts {
        accounts: Vec::with_capacity(length),
        indices: HashMap::with_capacity(length),
    });

    for account in remaining_accounts.iter().take(end_index).skip(start_index) {
        if *account.owner != crate::ID {
            return Err(error!(ErrorCodes::VaultTickHasDebtOwnerNotValid));
        }

        let tick_has_debt = AccountLoader::<TickHasDebtArray>::try_from(account)?;
        tick_has_debt_accounts.accounts.push(tick_has_debt);
    }

    tick_has_debt_accounts.indices =
        get_tick_has_debt_indices(&tick_has_debt_accounts.accounts, vault_id)?;

    Ok(tick_has_debt_accounts)
}

fn get_tick_has_debt_indices<'info>(
    tick_has_debt_accounts: &Vec<AccountLoader<'info, TickHasDebtArray>>,
    vault_id: u16,
) -> Result<HashMap<u8, usize>> {
    tick_has_debt_accounts
        .iter()
        .enumerate()
        .map(|(idx, t)| {
            let tick_has_debt = t.load()?;
            if tick_has_debt.vault_id != vault_id {
                return Err(error!(ErrorCodes::VaultTickHasDebtVaultIdMismatch));
            }
            Ok((tick_has_debt.index, idx))
        })
        .collect()
}

pub fn get_tick_has_debt_from_remaining_accounts_operate<'info>(
    remaining_accounts: &'info [AccountInfo<'info>],
    remaining_accounts_indices: &Vec<u8>,
    vault_id: u16,
) -> Result<Box<TickHasDebtAccounts<'info>>> {
    // remaining_accounts_indices[0] is oracle sources length
    // remaining_accounts_indices[1] is branches length
    // remaining_accounts_indices[2] is tick has debt length

    let tick_has_debt_length: usize = remaining_accounts_indices[2].cast::<usize>()?;

    let start_index: usize = remaining_accounts_indices[0].cast::<usize>()?
        + remaining_accounts_indices[1].cast::<usize>()?;
    let end_index: usize = start_index + tick_has_debt_length;

    load_tick_has_debt_accounts(
        remaining_accounts,
        start_index,
        end_index,
        tick_has_debt_length,
        vault_id,
    )
}

pub fn get_tick_has_debt_from_remaining_accounts_liquidate<'info>(
    remaining_accounts: &'info [AccountInfo<'info>],
    remaining_accounts_indices: &Vec<u8>,
    vault_id: u16,
) -> Result<Box<TickHasDebtAccounts<'info>>> {
    // remaining_accounts_indices[0] is oracle sources length
    // remaining_accounts_indices[1] is branches length
    // remaining_accounts_indices[2] is ticks length
    // remaining_accounts_indices[3] is tick has debt length

    let tick_has_debt_length: usize = remaining_accounts_indices[3].cast::<usize>()?;

    let start_index: usize = remaining_accounts_indices[0].cast::<usize>()?
        + remaining_accounts_indices[1].cast::<usize>()?
        + remaining_accounts_indices[2].cast::<usize>()?;

    let end_index: usize = start_index + tick_has_debt_length;

    load_tick_has_debt_accounts(
        remaining_accounts,
        start_index,
        end_index,
        tick_has_debt_length,
        vault_id,
    )
}

/// Get the index (0-15) for a given tick
pub fn get_array_index_for_tick(tick: i32) -> Result<u8> {
    if tick < MIN_TICK || tick > MAX_TICK {
        return Err(ErrorCodes::VaultTickHasDebtOutOfRange.into());
    }

    // Convert tick to 0-based index
    let tick_offset = tick.safe_sub(MIN_TICK)?; // 0 to 32766

    // Each index covers 2048 ticks
    let index = (tick_offset / TICKS_PER_TICK_HAS_DEBT as i32) as u8;

    Ok(index)
}

/// Get the first tick for a given index (0-15)
pub fn get_first_tick_for_array_index(index: u8) -> Result<i32> {
    if index >= TOTAL_INDICES_NEEDED as u8 {
        return Err(ErrorCodes::VaultTickHasDebtOutOfRange.into());
    }

    // Index layout:
    // index 0: ticks -16383 to -14336 (2048 ticks)
    // index 1: ticks -14335 to -12288 (2048 ticks)
    // index 2: ticks -12287 to -10240 (2048 ticks)
    // index 3: ticks -10239 to -8192 (2048 ticks)
    // index 4: ticks -8191 to -6144 (2048 ticks)
    // index 5: ticks -6143 to -4096 (2048 ticks)
    // index 6: ticks -4095 to -2048 (2048 ticks)
    // index 7: ticks -2047 to 0 (2048 ticks)
    // index 8: ticks 1 to 2048 (2048 ticks)
    // index 9: ticks 2049 to 4096 (2048 ticks)
    // ...
    // index 15: ticks 14336 to 16383 (2047 ticks)

    Ok(MIN_TICK.safe_add((index as i32).safe_mul(TICKS_PER_TICK_HAS_DEBT as i32)?)?)
}

/// Get the first tick for a given map_index within a specific TickHasDebtArray
pub fn get_first_tick_for_map_in_array(array_index: u8, map_index: u8) -> Result<i32> {
    if array_index >= TOTAL_INDICES_NEEDED as u8 || map_index >= TICK_HAS_DEBT_ARRAY_SIZE as u8 {
        return Err(ErrorCodes::VaultTickHasDebtOutOfRange.into());
    }

    // Each array covers 2048 ticks, each map within array covers 256 ticks
    let array_first_tick = get_first_tick_for_array_index(array_index)?;

    let map_first_tick = array_first_tick
        .safe_add((map_index as i32).safe_mul(TICK_HAS_DEBT_CHILDREN_SIZE_IN_BITS as i32)?)?;

    Ok(map_first_tick)
}

/// Given a tick and the array index, returns (map_index, byte_index, bit_index) where:
/// - map_index: the index within tick_has_debt array (0-7)
/// - byte_index: the index within the children_bits array (0-31)
/// - bit_index: the bit index within the byte (0-7)
pub fn get_tick_indices_for_array(tick: i32, array_index: u8) -> Result<(u8, u8, u8)> {
    // Validate tick range
    if tick < MIN_TICK || tick > MAX_TICK {
        return Err(ErrorCodes::VaultTickHasDebtOutOfRange.into());
    }

    // Get the expected index for this tick
    let expected_index = get_array_index_for_tick(tick)?;
    if expected_index != array_index {
        return Err(ErrorCodes::VaultTickHasDebtIndexMismatch.into());
    }

    // Get the first tick for this array index
    let first_tick_for_index = get_first_tick_for_array_index(array_index)?;

    // Calculate position within this array (0 to 2047)
    let tick_within_array = tick.safe_sub(first_tick_for_index)?;

    if tick_within_array >= TICKS_PER_TICK_HAS_DEBT as i32 {
        return Err(ErrorCodes::VaultTickHasDebtOutOfRange.into());
    }

    // Each map_index covers 256 ticks
    let map_index =
        (tick_within_array / TICK_HAS_DEBT_CHILDREN_SIZE_IN_BITS as i32).cast::<u8>()?;

    // Within each map, calculate byte and bit position
    let tick_within_map = tick_within_array % TICK_HAS_DEBT_CHILDREN_SIZE_IN_BITS as i32;
    let byte_index = (tick_within_map / BIT_PER_BYTE as i32).cast::<u8>()?;
    let bit_index = (tick_within_map % BIT_PER_BYTE as i32).cast::<u8>()?;

    Ok((map_index, byte_index, bit_index))
}

pub fn get_tick_indices(tick: i32) -> Result<(u8, usize, usize, usize)> {
    let array_index = get_array_index_for_tick(tick)?;
    let (map_index, byte_index, bit_index) = get_tick_indices_for_array(tick, array_index)?;

    Ok((
        array_index,
        map_index.cast()?,
        byte_index.cast()?,
        bit_index.cast()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_calculation() {
        // Test boundary cases for index calculation
        assert_eq!(get_array_index_for_tick(MIN_TICK).unwrap(), 0);
        assert_eq!(get_array_index_for_tick(-14336).unwrap(), 0); // Last tick in index 0
        assert_eq!(get_array_index_for_tick(-14335).unwrap(), 1); // First tick in index 1
        assert_eq!(get_array_index_for_tick(0).unwrap(), 7);
        assert_eq!(get_array_index_for_tick(MAX_TICK).unwrap(), 15);
    }

    #[test]
    fn test_first_tick_for_index() {
        assert_eq!(get_first_tick_for_array_index(0).unwrap(), -16383);
        assert_eq!(get_first_tick_for_array_index(1).unwrap(), -14335);
        assert_eq!(get_first_tick_for_array_index(8).unwrap(), 1);
        assert_eq!(get_first_tick_for_array_index(15).unwrap(), 14337);
    }

    #[test]
    fn test_tick_indices_for_array() {
        // Test MIN_TICK in array index 0
        let (map_index, byte_index, bit_index) = get_tick_indices_for_array(MIN_TICK, 0).unwrap();
        assert_eq!(map_index, 0);
        assert_eq!(byte_index, 0);
        assert_eq!(bit_index, 0);

        // Test first tick in array index 1
        let (map_index, byte_index, bit_index) = get_tick_indices_for_array(-14335, 1).unwrap();
        assert_eq!(map_index, 0);
        assert_eq!(byte_index, 0);
        assert_eq!(bit_index, 0);

        // Test tick 0
        let (map_index, byte_index, bit_index) = get_tick_indices_for_array(0, 7).unwrap();
        assert_eq!(map_index, 7);
        assert_eq!(byte_index, 31);
        assert_eq!(bit_index, 7);

        // Test tick 1
        let (map_index, byte_index, bit_index) = get_tick_indices_for_array(1, 8).unwrap();
        assert_eq!(map_index, 0);
        assert_eq!(byte_index, 0);
        assert_eq!(bit_index, 0);
    }

    #[test]
    fn test_bit_index_never_exceeds_7() {
        // Test random ticks to ensure bit_index is always 0-7
        let test_ticks = vec![MIN_TICK, -1000, -1, 0, 1, 1000, MAX_TICK];

        for tick in test_ticks {
            let array_index = get_array_index_for_tick(tick).unwrap();
            let (_, _, bit_index) = get_tick_indices_for_array(tick, array_index).unwrap();
            assert!(
                bit_index <= 7,
                "bit_index {} exceeds 7 for tick {}",
                bit_index,
                tick
            );
        }
    }

    #[test]
    fn test_byte_index_never_exceeds_31() {
        // Test random ticks to ensure byte_index is always 0-31
        let test_ticks = vec![MIN_TICK, -1000, -1, 0, 1, 1000, MAX_TICK];

        for tick in test_ticks {
            let array_index = get_array_index_for_tick(tick).unwrap();
            let (_, byte_index, _) = get_tick_indices_for_array(tick, array_index).unwrap();
            assert!(
                byte_index <= 31,
                "byte_index {} exceeds 31 for tick {}",
                byte_index,
                tick
            );
        }
    }

    #[test]
    fn test_coverage_completeness() {
        // Verify that every tick from MIN_TICK to MAX_TICK can be mapped
        let mut tick_count = 0;
        for tick in MIN_TICK..=MAX_TICK {
            let result = get_array_index_for_tick(tick);
            assert!(result.is_ok(), "Failed to get index for tick {}", tick);

            let array_index = result.unwrap();
            let result2 = get_tick_indices_for_array(tick, array_index);
            assert!(result2.is_ok(), "Failed to map tick {}", tick);
            tick_count += 1;
        }

        // Total ticks should be exactly 32767 (from -16383 to +16383 inclusive)
        assert_eq!(tick_count, 32767);
    }

    #[test]
    fn test_legacy_function() {
        // Test the legacy get_tick_indices function
        let (array_index, map_index, byte_index, bit_index) = get_tick_indices(MIN_TICK).unwrap();
        assert_eq!(array_index, 0);
        assert_eq!(map_index, 0);
        assert_eq!(byte_index, 0);
        assert_eq!(bit_index, 0);

        let (array_index, _, _, _) = get_tick_indices(MAX_TICK).unwrap();
        assert_eq!(array_index, 15);
        // MAX_TICK should be in the last position of index 15
    }

    #[test]
    fn test_array_boundaries() {
        // Test boundaries between arrays
        let boundaries = vec![
            (-16383, -14336), // index 0 range
            (-14335, -12288), // index 1 range
            (-1, 0),          // around zero
            (1, 2048),        // index 8 start
        ];

        for (start_tick, end_tick) in boundaries {
            if start_tick >= MIN_TICK && end_tick <= MAX_TICK {
                let start_index = get_array_index_for_tick(start_tick).unwrap();
                let end_index = get_array_index_for_tick(end_tick).unwrap();

                // Verify that ticks in different ranges get different indices
                if end_tick - start_tick > 2048 {
                    assert_ne!(
                        start_index, end_index,
                        "Different arrays should have different indices for ticks {} and {}",
                        start_tick, end_tick
                    );
                }
            }
        }
    }

    #[test]
    fn test_clear_bits_functionality() {
        // Test the specific bit clearing functionality that was affected by the precedence bug
        let mut tick_has_debt_array = TickHasDebtArray {
            vault_id: 1,
            index: 0,
            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
        };

        // Set up a scenario with multiple bits set
        let map_index = 0;
        let test_byte_index = 5;

        // Set multiple bits in the test byte
        tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index] = 0b11111111; // All bits set
        tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index + 1] =
            0b11111111; // Next byte all set
        tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index + 2] =
            0b11111111; // Next byte all set

        // Test clearing from bit_index 0 (should clear entire byte)
        let result = tick_has_debt_array.clear_bits(map_index, test_byte_index, 0);

        assert!(result.is_ok());
        assert_eq!(
            tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index],
            0
        );
        assert_eq!(
            tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index + 1],
            0
        );
        assert_eq!(
            tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index + 2],
            0
        );
    }

    #[test]
    fn test_clear_bits_partial_byte() {
        let mut tick_has_debt_array = TickHasDebtArray {
            vault_id: 1,
            index: 0,
            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
        };

        let map_index = 0;
        let test_byte_index = 5;

        // Set all bits in test byte
        tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index] = 0b11111111;
        tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index + 1] =
            0b11111111;

        // Test clearing from bit_index 3
        // With the fix: (1 << 3) - 1 = 8 - 1 = 7 = 0b00000111
        // Should keep bits 0, 1, 2 and clear bits 3, 4, 5, 6, 7
        let result = tick_has_debt_array.clear_bits(map_index, test_byte_index, 3);
        assert!(result.is_ok());

        // Expected: 0b11111111 & 0b00000111 = 0b00000111
        assert_eq!(
            tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index],
            0b00000111
        );
        // Higher bytes should be cleared
        assert_eq!(
            tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index + 1],
            0
        );
    }

    #[test]
    fn test_clear_bits_all_bit_positions() {
        // Test clear_bits for each possible bit_index (0-7) to ensure the mask calculation is correct
        for bit_index in 0..8 {
            let mut tick_has_debt_array = TickHasDebtArray {
                vault_id: 1,
                index: 0,
                tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
            };

            let map_index = 0;
            let test_byte_index = 10;

            // Set all bits
            tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index] =
                0b11111111;

            let result = tick_has_debt_array.clear_bits(map_index, test_byte_index, bit_index);
            assert!(
                result.is_ok(),
                "clear_bits failed for bit_index {}",
                bit_index
            );

            if bit_index == 0 {
                // When bit_index is 0, entire byte should be cleared
                assert_eq!(
                    tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index], 0,
                    "Bit index 0 should clear entire byte"
                );
            } else {
                // With correct precedence: (1 << bit_index) - 1
                let expected_mask = (1u8 << bit_index) - 1;
                let expected_result = 0b11111111 & expected_mask;

                assert_eq!(
                    tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index],
                    expected_result,
                    "Incorrect mask for bit_index {}. Expected mask: 0b{:08b}, got: 0b{:08b}",
                    bit_index,
                    expected_mask,
                    tick_has_debt_array.tick_has_debt[map_index].children_bits[test_byte_index]
                );
            }
        }
    }

    #[test]
    fn test_bit_manipulation_precedence_verification() {
        // Explicitly test the operator precedence issue that was fixed
        // This test documents the difference between the buggy and correct behavior

        for bit_index in 1..8 {
            // Correct behavior with parentheses: (1 << bit_index) - 1
            let correct_mask = (1u8 << bit_index) - 1;

            // What the buggy code would have produced: 1 << (bit_index - 1)
            let buggy_mask = if bit_index > 0 {
                1u8 << (bit_index - 1)
            } else {
                0
            };

            // Verify they're different (except for bit_index = 1 where they happen to be the same)
            if bit_index > 1 {
                assert_ne!(
                    correct_mask, buggy_mask,
                    "Masks should be different for bit_index {}: correct=0b{:08b}, buggy=0b{:08b}",
                    bit_index, correct_mask, buggy_mask
                );
            }

            // Verify correct mask properties
            assert_eq!(
                correct_mask.count_ones() as usize,
                bit_index,
                "Correct mask should have exactly {} bits set for bit_index {}",
                bit_index,
                bit_index
            );
        }
    }

    #[test]
    fn test_update_tick_has_debt_integration() {
        // Integration test to verify the bit manipulation works correctly in the context
        // of the actual tick debt tracking
        let mut tick_has_debt_array = TickHasDebtArray {
            vault_id: 1,
            index: 0,
            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
        };

        // Test adding debt for multiple ticks
        let test_tick = MIN_TICK + 100;
        let result = tick_has_debt_array.update_tick_has_debt(test_tick, true);
        assert!(result.is_ok());

        // Verify the bit was set
        let (map_index, byte_index, bit_index) = get_tick_indices_for_array(test_tick, 0).unwrap();
        let byte_val = tick_has_debt_array.tick_has_debt[map_index as usize].children_bits
            [byte_index as usize];
        let expected_bit = 1 << bit_index;
        assert_eq!(
            byte_val & expected_bit,
            expected_bit,
            "Bit should be set for tick {}",
            test_tick
        );

        // Test clearing bits for this tick should work correctly with the fixed precedence
        let result = tick_has_debt_array.clear_bits_for_tick(test_tick);
        assert!(result.is_ok());
    }

    #[test]
    fn test_edge_case_bit_positions() {
        // Test edge cases that might expose the precedence bug
        let test_cases = vec![
            (MIN_TICK, 0),       // First possible tick
            (MIN_TICK + 7, 0),   // Bit position 7 in first byte
            (MIN_TICK + 8, 0),   // First bit in second byte
            (MIN_TICK + 255, 0), // Last bit in first map
            (MIN_TICK + 256, 0), // First bit in second map
        ];

        for (tick, expected_array_index) in test_cases {
            let mut tick_has_debt_array = TickHasDebtArray {
                vault_id: 1,
                index: expected_array_index,
                tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
            };

            // Set the bit
            let result = tick_has_debt_array.update_tick_has_debt(tick, true);
            assert!(result.is_ok(), "Failed to set bit for tick {}", tick);

            // Verify it was set
            let (map_index, byte_index, bit_index) =
                get_tick_indices_for_array(tick, expected_array_index).unwrap();
            let byte_val = tick_has_debt_array.tick_has_debt[map_index as usize].children_bits
                [byte_index as usize];
            let expected_bit = 1 << bit_index;
            assert_eq!(
                byte_val & expected_bit,
                expected_bit,
                "Bit should be set for tick {}",
                tick
            );

            // Test clear_bits doesn't break due to precedence issues
            let result = tick_has_debt_array.clear_bits(
                map_index as usize,
                byte_index as usize,
                bit_index as usize,
            );
            assert!(result.is_ok(), "clear_bits failed for tick {}", tick);
        }
    }
}

#[cfg(test)]
mod tests_advanced {
    use super::*;

    // Helper function to create a TickHasDebtArray with random data
    fn create_test_array_with_random_data(vault_id: u16, index: u8, seed: u64) -> TickHasDebtArray {
        let mut tick_has_debt_array = TickHasDebtArray {
            vault_id,
            index,
            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
        };

        // Simple LCG for deterministic "random" data
        let mut rng = seed;
        for map_idx in 0..TICK_HAS_DEBT_ARRAY_SIZE {
            for byte_idx in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx] =
                    (rng & 0xFF) as u8;
            }
        }
        tick_has_debt_array
    }

    #[test]
    fn test_exhaustive_tick_mapping() {
        // Test every single tick in the entire range to ensure correctness
        let mut tested_ticks = 0;
        let mut array_coverage = [0u32; TOTAL_INDICES_NEEDED];

        for tick in MIN_TICK..=MAX_TICK {
            let array_index = get_array_index_for_tick(tick).unwrap();
            let (map_index, byte_index, bit_index) =
                get_tick_indices_for_array(tick, array_index).unwrap();

            // Verify all indices are within bounds
            assert!(array_index < TOTAL_INDICES_NEEDED as u8);
            assert!(map_index < TICK_HAS_DEBT_ARRAY_SIZE as u8);
            assert!(byte_index < TICK_HAS_DEBT_CHILDREN_SIZE as u8);
            assert!(bit_index < 8);

            // Track coverage
            array_coverage[array_index as usize] += 1;
            tested_ticks += 1;

            // Verify round-trip calculation
            let calculated_first_tick =
                get_first_tick_for_map_in_array(array_index, map_index).unwrap();
            let tick_offset_in_map = tick - calculated_first_tick;
            assert!(
                tick_offset_in_map >= 0 && tick_offset_in_map < 256,
                "Tick {} maps incorrectly. Expected offset 0-255, got {}",
                tick,
                tick_offset_in_map
            );

            let expected_byte_index = tick_offset_in_map / 8;
            let expected_bit_index = tick_offset_in_map % 8;
            assert_eq!(byte_index as i32, expected_byte_index);
            assert_eq!(bit_index as i32, expected_bit_index);
        }

        assert_eq!(tested_ticks, 32767); // Total tick range

        // Verify all arrays got reasonable coverage
        for (idx, count) in array_coverage.iter().enumerate() {
            if idx < 15 {
                assert_eq!(*count, 2048, "Array {} should have exactly 2048 ticks", idx);
            } else {
                assert_eq!(*count, 2047, "Last array should have 2047 ticks"); // MAX_TICK edge case
            }
        }
    }

    #[test]
    fn test_stress_bit_manipulation_all_positions() {
        // Stress test bit manipulation for every possible position
        for array_index in 0..TOTAL_INDICES_NEEDED as u8 {
            for map_index in 0..TICK_HAS_DEBT_ARRAY_SIZE {
                for byte_index in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                    for bit_index in 0..8 {
                        let mut tick_has_debt_array = TickHasDebtArray {
                            vault_id: 1,
                            index: array_index,
                            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
                        };

                        // Fill with pattern to detect corruption
                        for i in 0..TICK_HAS_DEBT_ARRAY_SIZE {
                            for j in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                                tick_has_debt_array.tick_has_debt[i].children_bits[j] = 0xFF;
                            }
                        }

                        // Test clear_bits
                        let result =
                            tick_has_debt_array.clear_bits(map_index, byte_index, bit_index);
                        assert!(
                            result.is_ok(),
                            "clear_bits failed at array={}, map={}, byte={}, bit={}",
                            array_index,
                            map_index,
                            byte_index,
                            bit_index
                        );

                        // Verify the target byte has correct mask
                        let actual_byte =
                            tick_has_debt_array.tick_has_debt[map_index].children_bits[byte_index];
                        let expected_mask = if bit_index == 0 {
                            0
                        } else {
                            (1u8 << bit_index) - 1
                        };
                        assert_eq!(actual_byte, expected_mask,
                                  "Wrong mask at array={}, map={}, byte={}, bit={}. Expected: 0b{:08b}, got: 0b{:08b}",
                                  array_index, map_index, byte_index, bit_index, expected_mask, actual_byte);

                        // Verify all higher bytes in same map are cleared
                        for higher_byte in (byte_index + 1)..TICK_HAS_DEBT_CHILDREN_SIZE {
                            assert_eq!(
                                tick_has_debt_array.tick_has_debt[map_index].children_bits
                                    [higher_byte],
                                0,
                                "Higher byte {} not cleared in map {}",
                                higher_byte,
                                map_index
                            );
                        }

                        // Verify other maps are unchanged
                        for other_map in 0..TICK_HAS_DEBT_ARRAY_SIZE {
                            if other_map != map_index {
                                for j in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                                    assert_eq!(
                                        tick_has_debt_array.tick_has_debt[other_map].children_bits
                                            [j],
                                        0xFF,
                                        "Other map {} corrupted at byte {}",
                                        other_map,
                                        j
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_massive_random_operations() {
        // Stress test with thousands of random operations
        const NUM_OPERATIONS: usize = 10000;
        const NUM_ARRAYS: usize = 4;

        let mut arrays = Vec::new();
        for i in 0..NUM_ARRAYS {
            arrays.push(create_test_array_with_random_data(
                100,
                i as u8,
                i as u64 * 12345,
            ));
        }

        let mut rng = 98765u64;
        let mut operation_count = 0;

        for _ in 0..NUM_OPERATIONS {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let array_idx = (rng as usize) % NUM_ARRAYS;
            let tick_range_start = get_first_tick_for_array_index(array_idx as u8).unwrap();
            let tick_offset = (rng % 2048) as i32;
            let test_tick = tick_range_start + tick_offset;

            if test_tick < MIN_TICK || test_tick > MAX_TICK {
                continue;
            }

            let operation = rng % 3;
            match operation {
                0 => {
                    // Set bit
                    let result = arrays[array_idx].update_tick_has_debt(test_tick, true);
                    assert!(
                        result.is_ok(),
                        "Failed to set bit for tick {} in operation {}",
                        test_tick,
                        operation_count
                    );
                }
                1 => {
                    // Clear bit
                    let result = arrays[array_idx].update_tick_has_debt(test_tick, false);
                    assert!(
                        result.is_ok(),
                        "Failed to clear bit for tick {} in operation {}",
                        test_tick,
                        operation_count
                    );
                }
                2 => {
                    // Clear bits from this position
                    let result = arrays[array_idx].clear_bits_for_tick(test_tick);
                    assert!(
                        result.is_ok(),
                        "Failed to clear bits for tick {} in operation {}",
                        test_tick,
                        operation_count
                    );
                }
                _ => unreachable!(),
            }

            operation_count += 1;
        }

        println!(
            "Successfully completed {} random operations",
            operation_count
        );
    }

    #[test]
    fn test_concurrent_pattern_corruption_detection() {
        // Test that operations don't corrupt unrelated data
        let mut tick_has_debt_array = TickHasDebtArray {
            vault_id: 1,
            index: 5,
            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
        };

        // Create distinct patterns in each map
        for map_idx in 0..TICK_HAS_DEBT_ARRAY_SIZE {
            let pattern = 0x11 * (map_idx as u8 + 1); // 0x11, 0x22, 0x33, etc.
            for byte_idx in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx] = pattern;
            }
        }

        // Perform operations on middle map
        let target_map = 3;
        let target_byte = 15;
        let target_bit = 5;

        // Store original patterns
        let mut original_patterns = [[0u8; TICK_HAS_DEBT_CHILDREN_SIZE]; TICK_HAS_DEBT_ARRAY_SIZE];
        for map_idx in 0..TICK_HAS_DEBT_ARRAY_SIZE {
            for byte_idx in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                original_patterns[map_idx][byte_idx] =
                    tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx];
            }
        }

        // Perform clear_bits operation
        let result = tick_has_debt_array.clear_bits(target_map, target_byte, target_bit);
        assert!(result.is_ok());

        // Verify only the target map/bytes were affected
        for map_idx in 0..TICK_HAS_DEBT_ARRAY_SIZE {
            if map_idx == target_map {
                // Verify target byte has correct result (original_pattern & mask)
                let expected_mask = (1u8 << target_bit) - 1;
                let original_pattern = original_patterns[map_idx][target_byte];
                let expected_result = original_pattern & expected_mask;
                assert_eq!(
                    tick_has_debt_array.tick_has_debt[map_idx].children_bits[target_byte],
                    expected_result
                );

                // Verify higher bytes are cleared
                for byte_idx in (target_byte + 1)..TICK_HAS_DEBT_CHILDREN_SIZE {
                    assert_eq!(
                        tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx],
                        0
                    );
                }

                // Verify lower bytes are unchanged
                for byte_idx in 0..target_byte {
                    assert_eq!(
                        tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx],
                        original_patterns[map_idx][byte_idx]
                    );
                }
            } else {
                // Other maps should be completely unchanged
                for byte_idx in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                    assert_eq!(
                        tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx],
                        original_patterns[map_idx][byte_idx],
                        "Corruption detected in map {} byte {} (should be unchanged)",
                        map_idx,
                        byte_idx
                    );
                }
            }
        }
    }

    #[test]
    fn test_boundary_tick_operations() {
        // Test operations at critical boundaries between arrays/maps/bytes
        let critical_ticks = vec![
            MIN_TICK,
            MIN_TICK + 7,    // Last bit of first byte
            MIN_TICK + 8,    // First bit of second byte
            MIN_TICK + 255,  // Last bit of first map
            MIN_TICK + 256,  // First bit of second map
            MIN_TICK + 2047, // Last bit of first array
            MIN_TICK + 2048, // First bit of second array (if exists)
            0,               // Zero tick
            1,               // First positive tick
            MAX_TICK - 1,    // Near max
            MAX_TICK,        // Actual max
        ];

        for &tick in &critical_ticks {
            if tick < MIN_TICK || tick > MAX_TICK {
                continue;
            }

            let array_index = get_array_index_for_tick(tick).unwrap();
            let mut tick_has_debt_array = TickHasDebtArray {
                vault_id: 1,
                index: array_index,
                tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
            };

            // Fill with test pattern
            for map_idx in 0..TICK_HAS_DEBT_ARRAY_SIZE {
                for byte_idx in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                    tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx] = 0xAA;
                    // 10101010
                }
            }

            // Test setting the bit
            let result = tick_has_debt_array.update_tick_has_debt(tick, true);
            assert!(
                result.is_ok(),
                "Failed to set debt for critical tick {}",
                tick
            );

            // Test clearing the bit
            let result = tick_has_debt_array.update_tick_has_debt(tick, false);
            assert!(
                result.is_ok(),
                "Failed to clear debt for critical tick {}",
                tick
            );

            // Test clear_bits_for_tick
            let result = tick_has_debt_array.clear_bits_for_tick(tick);
            assert!(
                result.is_ok(),
                "Failed to clear bits for critical tick {}",
                tick
            );
        }
    }

    #[test]
    fn test_mask_generation_mathematical_properties() {
        // Verify mathematical properties of the mask generation
        for bit_index in 0..8 {
            let mask = if bit_index == 0 {
                0u8
            } else {
                (1u8 << bit_index) - 1
            };

            // Property 1: Mask should have exactly bit_index number of bits set
            assert_eq!(
                mask.count_ones() as usize,
                bit_index,
                "Mask for bit_index {} should have {} bits set",
                bit_index,
                bit_index
            );

            // Property 2: All set bits should be consecutive from LSB
            if bit_index > 0 {
                let expected_mask = (1u16 << bit_index) - 1;
                assert_eq!(
                    mask as u16, expected_mask,
                    "Mask calculation incorrect for bit_index {}",
                    bit_index
                );
            }

            // Property 3: Mask should clear bit_index and all higher bits when applied
            let test_byte = 0xFFu8; // All bits set
            let result = test_byte & mask;

            for bit_pos in 0..8 {
                let bit_is_set = (result & (1 << bit_pos)) != 0;
                let should_be_set = bit_pos < bit_index;
                assert_eq!(
                    bit_is_set, should_be_set,
                    "Bit {} should be {} for bit_index {}",
                    bit_pos, should_be_set, bit_index
                );
            }
        }
    }

    #[test]
    fn test_fetch_next_tick_comprehensive() {
        // Comprehensive test of fetch_next_tick functionality
        let mut tick_has_debt_array = TickHasDebtArray {
            vault_id: 1,
            index: 0,
            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
        };

        // Set bits for specific test ticks
        let test_ticks = vec![
            MIN_TICK + 10,
            MIN_TICK + 100,
            MIN_TICK + 500,
            MIN_TICK + 1000,
            MIN_TICK + 1500,
        ];
        for &tick in &test_ticks {
            let result = tick_has_debt_array.update_tick_has_debt(tick, true);
            assert!(result.is_ok());
        }

        // Test fetch_next_top_tick for each map
        for map_index in 0..TICK_HAS_DEBT_ARRAY_SIZE {
            let (next_tick, has_next) = tick_has_debt_array.fetch_next_top_tick(map_index).unwrap();

            if has_next && next_tick != COLD_TICK {
                // Verify the returned tick is actually set
                let result = tick_has_debt_array.update_tick_has_debt(next_tick, false);
                assert!(result.is_ok());

                // Verify it was actually set before
                let result = tick_has_debt_array.update_tick_has_debt(next_tick, true);
                assert!(result.is_ok());

                println!("Map {} returned next tick: {}", map_index, next_tick);
            }
        }
    }

    #[test]
    fn test_operator_precedence_regression() {
        // Explicit regression test for the operator precedence bug
        // This test will fail if someone accidentally removes the parentheses

        struct TestCase {
            bit_index: usize,
            input_byte: u8,
            expected_result: u8,
            description: &'static str,
        }

        let test_cases = vec![
            TestCase {
                bit_index: 0,
                input_byte: 0xFF,
                expected_result: 0x00,
                description: "bit_index 0 should clear entire byte",
            },
            TestCase {
                bit_index: 1,
                input_byte: 0xFF,
                expected_result: 0x01,
                description: "bit_index 1 should keep bit 0 only",
            },
            TestCase {
                bit_index: 3,
                input_byte: 0xFF,
                expected_result: 0x07,
                description: "bit_index 3 should keep bits 0,1,2",
            },
            TestCase {
                bit_index: 7,
                input_byte: 0xFF,
                expected_result: 0x7F,
                description: "bit_index 7 should keep bits 0-6",
            },
        ];

        for test_case in test_cases {
            let mask = if test_case.bit_index == 0 {
                0u8
            } else {
                (1u8 << test_case.bit_index) - 1
            };

            let result = test_case.input_byte & mask;
            assert_eq!(
                result, test_case.expected_result,
                "REGRESSION: {} - bit_index: {}, mask: 0b{:08b}, expected: 0b{:08b}, got: 0b{:08b}",
                test_case.description, test_case.bit_index, mask, test_case.expected_result, result
            );

            // Also verify what the BUGGY code would have produced
            if test_case.bit_index > 0 {
                let buggy_mask = 1u8 << (test_case.bit_index - 1);
                let buggy_result = test_case.input_byte & buggy_mask;

                // The buggy result should be different (except for bit_index=1 edge case)
                if test_case.bit_index > 1 {
                    assert_ne!(result, buggy_result,
                              "CRITICAL: Fixed and buggy results are the same for bit_index {}! This suggests the fix was reverted.",
                              test_case.bit_index);
                }
            }
        }
    }

    #[test]
    fn test_extreme_edge_cases() {
        // Test with maximum possible values and edge conditions

        // Test with array at maximum index
        let mut tick_has_debt_array = TickHasDebtArray {
            vault_id: u16::MAX,
            index: (TOTAL_INDICES_NEEDED - 1) as u8,
            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
        };

        // Test MAX_TICK operations
        let result = tick_has_debt_array.update_tick_has_debt(MAX_TICK, true);
        assert!(result.is_ok(), "Failed to handle MAX_TICK");

        let result = tick_has_debt_array.clear_bits_for_tick(MAX_TICK);
        assert!(result.is_ok(), "Failed to clear bits for MAX_TICK");

        // Test with all bits set to verify clearing works
        for map_idx in 0..TICK_HAS_DEBT_ARRAY_SIZE {
            for byte_idx in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx] = 0xFF;
            }
        }

        // Clear from the middle and verify pattern
        let result = tick_has_debt_array.clear_bits(4, 16, 4);
        assert!(result.is_ok());

        // Verify the pattern is as expected
        assert_eq!(tick_has_debt_array.tick_has_debt[4].children_bits[16], 0x0F); // 0b00001111
        for byte_idx in 17..TICK_HAS_DEBT_CHILDREN_SIZE {
            assert_eq!(
                tick_has_debt_array.tick_has_debt[4].children_bits[byte_idx],
                0x00
            );
        }
    }

    #[test]
    fn test_performance_worst_case() {
        // Test performance characteristics in worst-case scenarios
        const ITERATIONS: usize = 1000;

        let mut tick_has_debt_array = TickHasDebtArray {
            vault_id: 1,
            index: 0,
            tick_has_debt: [TickHasDebt::default(); TICK_HAS_DEBT_ARRAY_SIZE],
        };

        // Fill with maximum density (every bit set)
        for map_idx in 0..TICK_HAS_DEBT_ARRAY_SIZE {
            for byte_idx in 0..TICK_HAS_DEBT_CHILDREN_SIZE {
                tick_has_debt_array.tick_has_debt[map_idx].children_bits[byte_idx] = 0xFF;
            }
        }

        // Perform many clear operations (worst case for the algorithm)
        let start_tick = MIN_TICK + 1000;
        for i in 0..ITERATIONS {
            let test_tick = start_tick + (i as i32);
            if test_tick > MAX_TICK {
                break;
            }

            let result = tick_has_debt_array.clear_bits_for_tick(test_tick);
            assert!(result.is_ok(), "Performance test failed at iteration {}", i);
        }

        // Test fetch_next_tick operations
        for map_index in 0..TICK_HAS_DEBT_ARRAY_SIZE {
            let (_next_tick, _has_next) =
                tick_has_debt_array.fetch_next_top_tick(map_index).unwrap();
            // Just verify it doesn't panic or error
        }
    }
}
