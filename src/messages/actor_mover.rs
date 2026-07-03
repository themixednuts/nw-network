//! Actor movement message payloads.

use crate::{Marshaler, az_rtti, type_registry};

/// Requests a pass over movement requests that were deferred earlier.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("4C220B64-C728-4D31-9C1E-6244E8089215")]
#[type_registry(723)]
pub struct ProcessDeferredMovementRequestsMsg;

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::generated_messages::{
        DelayMigrationsMsg, MoveActorMsg, MoveCoordinatorInitMsg, TimeoutMigrationsAtCommitMsg,
        TimeoutMigrationsMsg,
    };
    use crate::hub::{ActorId, Duration};
    use crate::serialize::{CARRIER_ENDIAN, WriteBuffer};

    use super::*;

    fn marshal_bytes(value: &impl Marshaler) -> Vec<u8> {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        wb.into_vec()
    }

    #[test]
    fn generated_actor_mover_messages_use_semantic_field_types() {
        let move_actor = MoveActorMsg {
            actor_id: ActorId::new(uuid!("00112233-4455-6677-8899-aabbccddeeff")),
        };
        assert_eq!(
            marshal_bytes(&move_actor),
            [
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff
            ]
        );

        let timeout = TimeoutMigrationsMsg { duration_secs: 30 };
        assert_eq!(marshal_bytes(&timeout), [0, 0, 0, 30]);

        let timeout_at_commit = TimeoutMigrationsAtCommitMsg { duration_secs: 45 };
        assert_eq!(marshal_bytes(&timeout_at_commit), [0, 0, 0, 45]);

        let delay = DelayMigrationsMsg { duration_secs: 60 };
        assert_eq!(marshal_bytes(&delay), [0, 0, 0, 60]);

        let init = MoveCoordinatorInitMsg {
            movement_timeout: Duration::new(0x0102_0304_0506_0708),
        };
        assert_eq!(
            marshal_bytes(&init),
            [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
    }

    #[test]
    fn deferred_movement_request_message_has_empty_payload() {
        assert!(marshal_bytes(&ProcessDeferredMovementRequestsMsg).is_empty());
    }
}
