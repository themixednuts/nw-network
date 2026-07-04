//! Server-context control message payloads.

use crate::{Marshaler, az_rtti, type_registry};

/// Requests the migration test path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("11F92E5C-122F-4FAF-A2A4-15BD4E2ED629")]
#[type_registry(2170)]
pub struct MigrationTestMsg;

/// Requests a forced actor migration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("FAF8BF74-2D81-42EE-932C-FE6F1D26A04C")]
#[type_registry(67)]
pub struct ForceMigrateActorMsg;

/// Requests a forced respawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("E0C6019C-AAE0-4C31-AAF1-C7A674C89751")]
#[type_registry(1270)]
pub struct ForceRespawnMsg;

/// Requests an immediate persistence pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("E79B0589-3011-4358-BEF5-018FF608DD7B")]
#[type_registry(2173)]
pub struct ForcePersistMsg;

/// Requests script garbage collection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("81B1655E-57B3-4636-AB32-67315A131325")]
#[type_registry(1487)]
pub struct ScriptGarbageCollectMsg;

/// Notifies clients that stack configuration changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("20D2224E-FAC8-4F92-8C80-A30889D1C269")]
#[type_registry(5536)]
pub struct StackConfigChangedMsg;

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::generated_messages::{
        AddPortrayalToClientsMsg, ForceMigrateAndCrashMsg, PulseMsg, RemovePortrayalFromClientsMsg,
        RequestReportDirtyPersistedStatesMsg, SetBurningMigrationTestMsg,
    };
    use crate::serialize::{CARRIER_ENDIAN, WriteBuffer};
    use crate::{
        ActorRef, ClientRef, GdeId, RemoteServerContextRef, RemoteServerGdeRef,
        RemoteTypelessServerFacetRef, TimePoint,
    };

    use super::*;

    fn marshal_bytes(value: &impl Marshaler) -> Vec<u8> {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        wb.into_vec()
    }

    #[test]
    fn fieldless_server_context_messages_have_empty_payloads() {
        assert!(marshal_bytes(&MigrationTestMsg).is_empty());
        assert!(marshal_bytes(&ForceMigrateActorMsg).is_empty());
        assert!(marshal_bytes(&ForceRespawnMsg).is_empty());
        assert!(marshal_bytes(&ForcePersistMsg).is_empty());
        assert!(marshal_bytes(&ScriptGarbageCollectMsg).is_empty());
        assert!(marshal_bytes(&StackConfigChangedMsg).is_empty());
    }

    #[test]
    fn generated_server_context_control_messages_preserve_field_order() {
        assert_eq!(
            marshal_bytes(&SetBurningMigrationTestMsg {
                value: true,
                period_ms: -30,
            }),
            [1, 0xff, 0xff, 0xff, 0xe2]
        );
        assert_eq!(
            marshal_bytes(&ForceMigrateAndCrashMsg { crash_source: true }),
            [1]
        );
        assert_eq!(
            marshal_bytes(&RequestReportDirtyPersistedStatesMsg {
                is_delayed_report: true,
                is_final_save_on_shutdown: false,
            }),
            [1, 0]
        );

        let current_time_point = TimePoint {
            nanoseconds_since_server_start: 0x0102_0304_0506_0708,
        };
        assert_eq!(
            marshal_bytes(&PulseMsg { current_time_point }),
            [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
    }

    #[test]
    fn generated_portrayal_messages_preserve_reference_field_order() {
        let gde_id = GdeId::new(0x0102_0304_0506_0708);
        let client_ref = ClientRef::new(ActorRef::new(
            1,
            uuid!("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            uuid!("11111111-2222-3333-4444-555555555555"),
        ));
        let ghost_client = ActorRef::new(
            2,
            uuid!("bbbbbbbb-cccc-dddd-eeee-ffffffffffff"),
            uuid!("22222222-3333-4444-5555-666666666666"),
        );
        let interest_ref = RemoteTypelessServerFacetRef {
            remote_server_gde_ref: RemoteServerGdeRef::new(
                RemoteServerContextRef::from_uuid(uuid!("33333333-4444-5555-6666-777777777777")),
                GdeId::new(9),
            ),
            target_id: 10,
        };
        let owning_actor = ActorRef::new(
            3,
            uuid!("cccccccc-dddd-eeee-ffff-000000000000"),
            uuid!("44444444-5555-6666-7777-888888888888"),
        );

        let mut expected = marshal_bytes(&gde_id);
        expected.extend(marshal_bytes(&client_ref));
        expected.extend(marshal_bytes(&ghost_client));
        expected.extend(marshal_bytes(&interest_ref));
        expected.extend(marshal_bytes(&owning_actor));

        assert_eq!(
            marshal_bytes(&AddPortrayalToClientsMsg {
                gde_id,
                client: client_ref,
                ghost_client,
                interest_ref,
                owning_actor,
            }),
            expected
        );
        assert_eq!(
            marshal_bytes(&RemovePortrayalFromClientsMsg {
                gde_id,
                client: client_ref,
                ghost_client,
                interest_ref,
                owning_actor,
            }),
            expected
        );
    }
}
