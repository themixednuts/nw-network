use uuid::Uuid;

use crate::serialize::ReplicatedFieldHandler;
use crate::types::{Crc32, TimePoint};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerGenericInviteParticipants;

impl crate::Marshaler for PlayerGenericInviteParticipants {
    fn marshal(&self, _wb: &mut crate::WriteBuffer) {}

    fn unmarshal(_rb: &mut crate::ReadBuffer) -> Result<Self, crate::MarshalerError> {
        Ok(Self)
    }
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("A02118E0-94AB-4945-B421-1FABFB0C4806")]
#[::nw_network::type_registry(3408)]
pub struct PlayerGenericInviteReplicatedState {
    pub invite_id: ReplicatedFieldHandler<Uuid>,
    #[replicated_state(group = 1)]
    pub activity_crc: ReplicatedFieldHandler<Crc32>,
    #[replicated_state(group = 1)]
    pub forward_type: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub invite_participants: ReplicatedFieldHandler<PlayerGenericInviteParticipants>,
    #[replicated_state(group = 1)]
    pub expiry_time_point: ReplicatedFieldHandler<TimePoint>,
}
