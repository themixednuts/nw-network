//! Server-context generated-message wire validation.

#[cfg(test)]
mod tests {
    use crate::TimePoint;
    use crate::generated_messages::{
        ForceMigrateActorMsg, ForceMigrateAndCrashMsg, ForcePersistMsg, ForceRespawnMsg,
        MigrationTestMsg, PulseMsg, RequestReportDirtyPersistedStatesMsg, ScriptGarbageCollectMsg,
        SetBurningMigrationTestMsg, StackConfigChangedMsg,
    };
    use crate::serialize::{CARRIER_ENDIAN, Marshal, WriteBuffer};
    fn marshal_bytes(value: &impl Marshal) -> Vec<u8> {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        wb.into_vec()
    }

    #[test]
    fn fieldless_server_context_messages_have_empty_payloads() {
        assert!(marshal_bytes(&MigrationTestMsg {}).is_empty());
        assert!(marshal_bytes(&ForceMigrateActorMsg {}).is_empty());
        assert!(marshal_bytes(&ForceRespawnMsg {}).is_empty());
        assert!(marshal_bytes(&ForcePersistMsg {}).is_empty());
        assert!(marshal_bytes(&ScriptGarbageCollectMsg {}).is_empty());
        assert!(marshal_bytes(&StackConfigChangedMsg {}).is_empty());
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
}
