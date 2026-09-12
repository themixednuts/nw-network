//! Outpost Rush warboard stats message payload.
//!
//! Carries `field_1` of `Javelin::ClientMessages::WarboardComponentClientFacet_OnUpdateWarboardStats`
//! (`typeIndex` 839). The static schema records the field as
//! `composite<fixed-bytes-2, 3x length-prefixed-bytes>`; measured against 1,538
//! captured bodies that layout consumes 23 of them exactly, leaves trailing bytes
//! on 612 and fails to read 903. The layout below consumes 1,533 exactly.
//!
//! Wire grammar, all integers the crate's prefix VLQ unless stated otherwise:
//!
//! ```text
//! stats := u16 BE counter          ; roster size so far, monotonic across a match
//!          u8 local_index          ; index of the receiving client
//!          row local               ; stats of the receiving client
//!          block*                  ; one per team, read to the end of the body
//! block := u8 count
//!          count x (u8 player_index, row)
//! row   := vlq mask                ; bit n set = stat n present
//!          vlq value               ; one per set bit, ascending bit order
//! ```
//!
//! Wire bit `n` is `WarboardStatType` ordinal `n - 1`; values are running match
//! totals, not deltas.
//!
//! Known limitation: 5 of the 1,538 measured bodies (0.33 %) carry a row whose
//! mask has bit 0 set. Bit 0 has no `WarboardStatType` ordinal under the `n - 1`
//! rule and the bytes that follow do not read back as stat values, so that row
//! shape is not modelled. [`WarboardStats::unmarshal`] rejects those bodies
//! instead of guessing.

use crate::serialize::marshaler::{Marshal, Unmarshal};
use crate::serialize::{MarshalerError, ReadBuffer, VlqU64Marshaler, WriteBuffer};

/// One player's warboard stats: a presence mask plus the running match total of
/// every stat the mask selects, in ascending bit order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WarboardStatRow {
    /// Bit `n` set means `WarboardStatType` ordinal `n - 1` is present.
    pub mask: u64,
    /// One running total per set mask bit, ascending bit order.
    pub values: Vec<u64>,
}

impl WarboardStatRow {
    /// Running total carried for wire bit `bit`, `None` when the bit is clear.
    #[must_use]
    pub fn stat(&self, bit: u32) -> Option<u64> {
        if bit >= u64::BITS || self.mask >> bit & 1 == 0 {
            return None;
        }
        let rank = (self.mask & ((1u64 << bit) - 1)).count_ones() as usize;
        self.values.get(rank).copied()
    }

    /// Wire bits present in this row, ascending.
    pub fn bits(&self) -> impl Iterator<Item = u32> + '_ {
        (0..u64::BITS).filter(|bit| self.mask >> bit & 1 == 1)
    }
}

impl Marshal for WarboardStatRow {
    fn marshal(&self, wb: &mut WriteBuffer) {
        assert_eq!(
            self.values.len(),
            self.mask.count_ones() as usize,
            "row carries one value per set mask bit"
        );
        VlqU64Marshaler.marshal(wb, self.mask);
        for value in &self.values {
            VlqU64Marshaler.marshal(wb, *value);
        }
    }
}

impl Unmarshal for WarboardStatRow {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let mask = VlqU64Marshaler.unmarshal(rb)?;
        // Wire bit 0 has no `WarboardStatType` ordinal and the bytes after such
        // a row do not read back as stat values: reject rather than guess.
        if mask & 1 != 0 {
            return Err(MarshalerError::InvalidRange {
                value: 0,
                min: 1,
                max: 63,
            });
        }

        // Every value costs at least one byte, so the buffer bounds the count
        // before anything is allocated.
        let count = mask.count_ones() as usize;
        if count > rb.left() {
            return Err(MarshalerError::buffer_underrun(rb.left(), count));
        }

        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(VlqU64Marshaler.unmarshal(rb)?);
        }
        Ok(Self { mask, values })
    }
}

/// One player's row inside a team block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WarboardPlayerStats {
    /// Team-local player index.
    pub player_index: u8,
    pub row: WarboardStatRow,
}

/// One team block: a counted run of player rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WarboardStatBlock {
    pub players: Vec<WarboardPlayerStats>,
}

impl Marshal for WarboardStatBlock {
    fn marshal(&self, wb: &mut WriteBuffer) {
        let count =
            u8::try_from(self.players.len()).expect("block row count fits the u8 wire count");
        count.marshal(wb);
        for player in &self.players {
            player.player_index.marshal(wb);
            player.row.marshal(wb);
        }
    }
}

impl Unmarshal for WarboardStatBlock {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let count = usize::from(u8::unmarshal(rb)?);
        // Bound the allocation by what the buffer can still supply: each row
        // costs at least an index byte and a mask byte.
        let needed = count.saturating_mul(2);
        if needed > rb.left() {
            return Err(MarshalerError::buffer_underrun(rb.left(), needed));
        }

        let mut players = Vec::with_capacity(count);
        for _ in 0..count {
            let player_index = u8::unmarshal(rb)?;
            let row = WarboardStatRow::unmarshal(rb)?;
            players.push(WarboardPlayerStats { player_index, row });
        }
        Ok(Self { players })
    }
}

/// Warboard stats payload of the type-839 client message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WarboardStats {
    /// Roster size seen so far; monotonic non-decreasing across a match.
    pub counter: u16,
    /// Index of the receiving client.
    pub local_index: u8,
    /// Stats of the receiving client.
    pub local: WarboardStatRow,
    /// One block per team.
    pub blocks: Vec<WarboardStatBlock>,
}

impl Marshal for WarboardStats {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.counter.marshal(wb);
        self.local_index.marshal(wb);
        self.local.marshal(wb);
        for block in &self.blocks {
            block.marshal(wb);
        }
    }
}

impl Unmarshal for WarboardStats {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let counter = u16::unmarshal(rb)?;
        let local_index = u8::unmarshal(rb)?;
        let local = WarboardStatRow::unmarshal(rb)?;

        // The blocks are the last field of the message, so they run to the end
        // of the body. Every block consumes at least its count byte, so the
        // remaining bytes bound the loop.
        let mut blocks = Vec::new();
        while rb.left() > 0 {
            blocks.push(WarboardStatBlock::unmarshal(rb)?);
        }

        Ok(Self {
            counter,
            local_index,
            local,
            blocks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{CARRIER_ENDIAN, ReadBuffer, WriteBuffer};

    #[test]
    fn row_indexes_values_by_mask_bit() {
        let row = WarboardStatRow {
            mask: (1 << 1) | (1 << 8) | (1 << 38),
            values: vec![11, 22, 33],
        };

        assert_eq!(row.stat(1), Some(11));
        assert_eq!(row.stat(8), Some(22));
        assert_eq!(row.stat(38), Some(33));
        assert_eq!(row.stat(0), None);
        assert_eq!(row.stat(2), None);
        assert_eq!(row.stat(64), None);
        assert!(row.bits().eq([1, 8, 38]));
    }

    #[test]
    fn row_with_mask_bit_0_is_rejected() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        VlqU64Marshaler.marshal(&mut wb, 0b11);
        VlqU64Marshaler.marshal(&mut wb, 7);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        assert!(matches!(
            WarboardStatRow::unmarshal(&mut rb),
            Err(MarshalerError::InvalidRange { .. })
        ));
    }

    #[test]
    fn truncated_block_count_does_not_allocate() {
        let bytes = [0xffu8, 0x01];
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        assert!(matches!(
            WarboardStatBlock::unmarshal(&mut rb),
            Err(MarshalerError::BufferUnderrun { .. })
        ));
    }
}
