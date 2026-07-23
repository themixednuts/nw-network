//! Source-backed actor movement payloads not yet replaced by generated types.

use crate::hub::{ActorId, ActorRef, MovementInteractionId, Timestamp};
use crate::{Marshaler, az_rtti, type_registry};

/// Queries the state of an in-flight actor migration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("44BA8334-C3AA-476E-855C-27364BF8A964")]
#[type_registry(747)]
pub struct CheckMovementStatusMsg {
    pub actor_id: ActorId,
    pub movement_interaction_timeout: Timestamp,
    pub originating_move_coordinator: ActorRef,
    pub movement_interaction_id: MovementInteractionId,
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::generated_messages::{
        AbortMovementMsg, CancelMovementMsg, CommitMovementMsg, CompleteDelayedMigrationsMsg,
        CrashMoveActorMsg, CrashMoveActorToHubMsg, DelayMigrationsMsg, MoveActorMsg,
        MoveActorToHubMsg, MoveCoordinatorInitMsg, MovementTimeoutMsg,
        ProcessDeferredMovementRequestsMsg, StartMovementCommunicationMsg,
        TimeoutMigrationsAtCommitMsg, TimeoutMigrationsMsg,
    };
    use crate::hub::{Duration, HubId, SyncedTimestamp};
    use crate::serialize::{CARRIER_ENDIAN, Marshal, WriteBuffer};

    use super::*;

    fn marshal_bytes(value: &impl Marshal) -> Vec<u8> {
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
    fn generated_actor_mover_handoff_messages_preserve_field_order() {
        let actor_id = ActorId::new(uuid!("00112233-4455-6677-8899-aabbccddeeff"));
        let hub_id = HubId::new(0x0102_0304);
        let coordinator = ActorRef::new(
            7,
            uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            uuid!("11111111-2222-3333-4444-555555555555"),
        );
        let movement_interaction_id = MovementInteractionId {
            src_hub_id: HubId::new(0x1122_3344),
            dest_hub_id: HubId::new(0x5566_7788),
            movement_sequence_id: 0x0102_0304_0506_0708,
            start_time_on_source: SyncedTimestamp::new(0x8877_6655_4433_2211),
        };

        let mut actor_hub = marshal_bytes(&actor_id);
        actor_hub.extend(marshal_bytes(&hub_id));
        assert_eq!(
            marshal_bytes(&MoveActorToHubMsg { actor_id, hub_id }),
            actor_hub
        );
        assert_eq!(
            marshal_bytes(&StartMovementCommunicationMsg { actor_id, hub_id }),
            actor_hub
        );

        let mut actor_movement = marshal_bytes(&actor_id);
        actor_movement.extend(marshal_bytes(&movement_interaction_id));
        assert_eq!(
            marshal_bytes(&CommitMovementMsg {
                actor_id,
                movement_interaction_id,
            }),
            actor_movement
        );
        assert_eq!(
            marshal_bytes(&AbortMovementMsg {
                actor_id,
                movement_interaction_id,
            }),
            actor_movement
        );

        let mut routed_movement = marshal_bytes(&actor_id);
        routed_movement.extend(marshal_bytes(&coordinator));
        routed_movement.extend(marshal_bytes(&movement_interaction_id));
        assert_eq!(
            marshal_bytes(&MovementTimeoutMsg {
                actor_id,
                remote_move_coordinator: coordinator,
                movement_interaction_id,
            }),
            routed_movement
        );
        assert_eq!(
            marshal_bytes(&CancelMovementMsg {
                actor_id,
                remote_move_coordinator: coordinator,
                movement_interaction_id,
            }),
            routed_movement
        );

        let timeout = Timestamp::new(0x1020_3040_5060_7080);
        let mut status = marshal_bytes(&actor_id);
        status.extend(marshal_bytes(&timeout));
        status.extend(marshal_bytes(&coordinator));
        status.extend(marshal_bytes(&movement_interaction_id));
        assert_eq!(
            marshal_bytes(&CheckMovementStatusMsg {
                actor_id,
                movement_interaction_timeout: timeout,
                originating_move_coordinator: coordinator,
                movement_interaction_id,
            }),
            status
        );
    }

    #[test]
    fn generated_actor_mover_fault_and_delay_messages_preserve_field_order() {
        let actor_id = ActorId::new(uuid!("00112233-4455-6677-8899-aabbccddeeff"));
        let hub_id = HubId::new(0x0102_0304);
        let delay_until = Timestamp::new(0x1020_3040_5060_7080);

        assert_eq!(
            marshal_bytes(&CompleteDelayedMigrationsMsg { delay_until }),
            marshal_bytes(&delay_until)
        );

        let mut crash_actor = marshal_bytes(&actor_id);
        crash_actor.extend(marshal_bytes(&crate::CrashTarget::Destination));
        assert_eq!(
            marshal_bytes(&CrashMoveActorMsg {
                actor_id,
                target: crate::CrashTarget::Destination,
            }),
            crash_actor
        );

        let mut crash_actor_to_hub = marshal_bytes(&actor_id);
        crash_actor_to_hub.extend(marshal_bytes(&hub_id));
        crash_actor_to_hub.extend(marshal_bytes(&crate::CrashTarget::Source));
        assert_eq!(
            marshal_bytes(&CrashMoveActorToHubMsg {
                actor_id,
                hub_id,
                target: crate::CrashTarget::Source,
            }),
            crash_actor_to_hub
        );
    }

    #[test]
    fn deferred_movement_request_message_has_empty_payload() {
        assert!(marshal_bytes(&ProcessDeferredMovementRequestsMsg {}).is_empty());
    }
}
