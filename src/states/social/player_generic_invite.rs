//! Generic player invite replication.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerGenericInviteParticipants;

impl crate::Marshal for PlayerGenericInviteParticipants {
    fn marshal(&self, _wb: &mut crate::WriteBuffer) {}
}

impl crate::Unmarshal for PlayerGenericInviteParticipants {
    fn unmarshal(_rb: &mut crate::ReadBuffer) -> Result<Self, crate::MarshalerError> {
        Ok(Self)
    }
}
pub use crate::generated::states::PlayerGenericInviteReplicatedState;
