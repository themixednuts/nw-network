use crate::hub::SequenceNumber;
use crate::serialize::container_marshal::marshal_wire_count;
use crate::serialize::{
    Marshaler, MarshalerError, ReadBuffer, ReplicatedFieldHandler, VlqU32Marshaler,
    VlqU64Marshaler, WIRE_VEC_CAP, WriteBuffer,
};

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("733DD7CC-D17E-41DD-B0EA-0FB6D8E0939F")]
#[::nw_network::type_registry(2895)]
pub struct ChatReplicatedState {
    pub state: ReplicatedFieldHandler<u32>,
    pub channel: ReplicatedFieldHandler<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatMuteEntry {
    IdOnly {
        player_id: u64,
        sequence: SequenceNumber,
    },
    WithReason {
        player_id: u64,
        sequence: SequenceNumber,
        reason: String,
    },
}

impl Default for ChatMuteEntry {
    fn default() -> Self {
        Self::IdOnly {
            player_id: 0,
            sequence: SequenceNumber::Invalid,
        }
    }
}

impl ChatMuteEntry {
    #[must_use]
    pub const fn player_id(&self) -> u64 {
        match self {
            Self::IdOnly { player_id, .. } | Self::WithReason { player_id, .. } => *player_id,
        }
    }

    #[must_use]
    pub const fn sequence(&self) -> SequenceNumber {
        match self {
            Self::IdOnly { sequence, .. } | Self::WithReason { sequence, .. } => *sequence,
        }
    }

    #[must_use]
    pub const fn has_reason(&self) -> bool {
        matches!(self, Self::WithReason { .. })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatMutes {
    pub entries: Vec<ChatMuteEntry>,
    pub base_sequence: SequenceNumber,
    pub trailing_strings: Vec<String>,
}

impl Marshaler for ChatMutes {
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.entries.len());

        if self.entries.is_empty() {
            self.base_sequence.marshal(wb);
            marshal_wire_count(wb, self.trailing_strings.len());
            for value in &self.trailing_strings {
                value.marshal(wb);
            }
            return;
        }

        let mut index = 0usize;
        let mut previous_sequence = SequenceNumber::Invalid;
        while index < self.entries.len() {
            let batch_end = (index + 8).min(self.entries.len());
            let mut bitmask = 0u8;
            for (bit, entry) in self.entries[index..batch_end].iter().enumerate() {
                if entry.has_reason() {
                    bitmask |= 1 << bit;
                }
            }

            bitmask.marshal(wb);
            for entry in &self.entries[index..batch_end] {
                VlqU64Marshaler.marshal(wb, entry.player_id());
                let sequence = entry.sequence();
                if sequence == previous_sequence {
                    SequenceNumber::ValidNonSequence.marshal(wb);
                } else {
                    sequence.marshal(wb);
                    previous_sequence = sequence;
                }

                if let ChatMuteEntry::WithReason { reason, .. } = entry {
                    reason.marshal(wb);
                }
            }
            index = batch_end;
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let mut count = VlqU32Marshaler.unmarshal(rb)? as usize;
        if count > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len: count,
                capacity: WIRE_VEC_CAP,
            });
        }
        if count == 0 {
            let base_sequence = SequenceNumber::unmarshal(rb)?;
            let string_count = VlqU32Marshaler.unmarshal(rb)? as usize;
            if string_count > WIRE_VEC_CAP {
                return Err(MarshalerError::ContainerOverflow {
                    len: string_count,
                    capacity: WIRE_VEC_CAP,
                });
            }
            let mut trailing_strings = Vec::with_capacity(string_count);
            for _ in 0..string_count {
                trailing_strings.push(String::unmarshal(rb)?);
            }

            return Ok(Self {
                entries: Vec::new(),
                base_sequence,
                trailing_strings,
            });
        }

        let mut entries = Vec::with_capacity(count);
        let mut previous_sequence = SequenceNumber::Invalid;
        while count > 0 {
            let bitmask = u8::unmarshal(rb)?;
            let batch = count.min(8);
            for bit in 0..batch {
                let player_id = VlqU64Marshaler.unmarshal(rb)?;
                let raw_sequence = SequenceNumber::unmarshal(rb)?;
                let sequence = if raw_sequence == SequenceNumber::ValidNonSequence {
                    previous_sequence
                } else {
                    previous_sequence = raw_sequence;
                    raw_sequence
                };
                let entry = if (bitmask >> bit) & 1 == 1 {
                    ChatMuteEntry::WithReason {
                        player_id,
                        sequence,
                        reason: String::unmarshal(rb)?,
                    }
                } else {
                    ChatMuteEntry::IdOnly {
                        player_id,
                        sequence,
                    }
                };
                entries.push(entry);
                count -= 1;
            }
        }

        Ok(Self {
            entries,
            base_sequence: SequenceNumber::Invalid,
            trailing_strings: Vec::new(),
        })
    }
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("01CEFF40-344D-4B55-9879-BA0D55C50312")]
#[::nw_network::type_registry(1566)]
pub struct ChatMutesReplicatedState {
    pub chat_mutes: ReplicatedFieldHandler<ChatMutes>,
}
