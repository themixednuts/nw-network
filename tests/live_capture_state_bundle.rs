use nw_network::{
    hub::{FragmentTypeInfo, InterestId, ReplicatedStateBundleView, SequenceNumber},
    serialize::{CARRIER_ENDIAN, MarshalerError, ReadBuffer},
    states::{
        ALCReplicatedState, AggregateContractCountComponentReplicatedState,
        InstancedSlayerScriptReplicatedState, InteractReplicatedState,
        ProgressionComponentReplicatedState, TemporaryAffiliationReplicatedState,
    },
    types::TypeRegistryEntry,
    validate_state_fragment_type_indices,
};

#[derive(Debug, Clone, Copy)]
struct ExpectedFragment {
    interest_id: u16,
    fragment_count: usize,
    fragment_key: u32,
    type_index: u32,
    body_len: usize,
    state: ExpectedState,
}

#[derive(Debug, Clone, Copy)]
enum ExpectedState {
    Alc {
        state_id: u16,
        time_offset_milliseconds: u16,
        look_dir: [f32; 3],
        rotation: Option<[f32; 4]>,
    },
    Interact {
        has_interactors: u32,
    },
    AggregateContractCount {
        total_buy_contracts: u32,
        total_sell_contracts: u32,
    },
}

struct BundleFixture {
    name: &'static str,
    body: &'static [u8],
    seq: SequenceNumber,
    bandwidth_mode: u8,
    is_unreliable: bool,
    has_replication_control: bool,
    stop_replication_ids: &'static [InterestId],
    pause_replication_ids: &'static [InterestId],
    bundle_buffer_len: usize,
    expected_fragments: &'static [ExpectedFragment],
}

const MODEL_20260619_021014_2760: &[ExpectedFragment] = &[
    ExpectedFragment {
        interest_id: 1,
        fragment_count: 1,
        fragment_key: 16,
        type_index: 11,
        body_len: 11,
        state: ExpectedState::Alc {
            state_id: 227,
            time_offset_milliseconds: 16_240,
            look_dir: [0.329_983_17, -0.943_986_83, 0.0],
            rotation: None,
        },
    },
    ExpectedFragment {
        interest_id: 141,
        fragment_count: 1,
        fragment_key: 16,
        type_index: 11,
        body_len: 28,
        state: ExpectedState::Alc {
            state_id: 211,
            time_offset_milliseconds: 10_671,
            look_dir: [0.579_550_3, 0.699_139_83, -0.418_718_13],
            rotation: Some([0.0, 0.0, 0.418_718_16, 0.908_116_2]),
        },
    },
    ExpectedFragment {
        interest_id: 101,
        fragment_count: 1,
        fragment_key: 4,
        type_index: 2930,
        body_len: 6,
        state: ExpectedState::Interact { has_interactors: 1 },
    },
    ExpectedFragment {
        interest_id: 83,
        fragment_count: 1,
        fragment_key: 4,
        type_index: 3791,
        body_len: 10,
        state: ExpectedState::AggregateContractCount {
            total_buy_contracts: 3046,
            total_sell_contracts: 19_352,
        },
    },
];

const MODEL_20260619_021014_2761: &[ExpectedFragment] = &[
    ExpectedFragment {
        interest_id: 1,
        fragment_count: 1,
        fragment_key: 16,
        type_index: 11,
        body_len: 11,
        state: ExpectedState::Alc {
            state_id: 227,
            time_offset_milliseconds: 16_240,
            look_dir: [0.329_983_17, -0.943_986_83, 0.0],
            rotation: None,
        },
    },
    ExpectedFragment {
        interest_id: 101,
        fragment_count: 1,
        fragment_key: 4,
        type_index: 2930,
        body_len: 6,
        state: ExpectedState::Interact { has_interactors: 1 },
    },
    ExpectedFragment {
        interest_id: 83,
        fragment_count: 1,
        fragment_key: 4,
        type_index: 3791,
        body_len: 10,
        state: ExpectedState::AggregateContractCount {
            total_buy_contracts: 3046,
            total_sell_contracts: 19_352,
        },
    },
];

const BUNDLE_FIXTURES: &[BundleFixture] = &[
    BundleFixture {
        name: "20260619T021014Z-pid65972 dseq 0045",
        body: include_bytes!(
            "fixtures/live/20260619T021014Z-pid65972_state_bundle_dseq0045_body.bin"
        ),
        seq: SequenceNumber::Seq(1),
        bandwidth_mode: 1,
        is_unreliable: true,
        has_replication_control: true,
        stop_replication_ids: &[],
        pause_replication_ids: &[],
        bundle_buffer_len: 46_754,
        expected_fragments: &[],
    },
    BundleFixture {
        name: "20260619T021014Z-pid65972 ingest 2760",
        body: include_bytes!(
            "fixtures/live/20260619T021014Z-pid65972_state_bundle_00_ingest_2760.bin"
        ),
        seq: SequenceNumber::Seq(704),
        bandwidth_mode: 2,
        is_unreliable: true,
        has_replication_control: true,
        stop_replication_ids: &[InterestId::new(111)],
        pause_replication_ids: &[],
        bundle_buffer_len: 74,
        expected_fragments: MODEL_20260619_021014_2760,
    },
    BundleFixture {
        name: "20260619T021014Z-pid65972 ingest 2761",
        body: include_bytes!(
            "fixtures/live/20260619T021014Z-pid65972_state_bundle_01_ingest_2761.bin"
        ),
        seq: SequenceNumber::Seq(705),
        bandwidth_mode: 2,
        is_unreliable: true,
        has_replication_control: true,
        stop_replication_ids: &[InterestId::new(111)],
        pause_replication_ids: &[],
        bundle_buffer_len: 41,
        expected_fragments: MODEL_20260619_021014_2761,
    },
    BundleFixture {
        name: "20260619T021014Z-pid65972 state 701",
        body: include_bytes!(
            "fixtures/live/20260619T021014Z-pid65972_state_bundle_02_state0701.bin"
        ),
        seq: SequenceNumber::Seq(701),
        bandwidth_mode: 2,
        is_unreliable: true,
        has_replication_control: true,
        stop_replication_ids: &[InterestId::new(111)],
        pause_replication_ids: &[],
        bundle_buffer_len: 651,
        expected_fragments: &[],
    },
    BundleFixture {
        name: "20260619T021014Z-pid65972 state 702",
        body: include_bytes!(
            "fixtures/live/20260619T021014Z-pid65972_state_bundle_03_state0702.bin"
        ),
        seq: SequenceNumber::Seq(702),
        bandwidth_mode: 2,
        is_unreliable: true,
        has_replication_control: true,
        stop_replication_ids: &[InterestId::new(111)],
        pause_replication_ids: &[],
        bundle_buffer_len: 997,
        expected_fragments: &[],
    },
    BundleFixture {
        name: "20260619T021014Z-pid65972 state 703",
        body: include_bytes!(
            "fixtures/live/20260619T021014Z-pid65972_state_bundle_04_state0703.bin"
        ),
        seq: SequenceNumber::Seq(703),
        bandwidth_mode: 2,
        is_unreliable: true,
        has_replication_control: true,
        stop_replication_ids: &[InterestId::new(111)],
        pause_replication_ids: &[],
        bundle_buffer_len: 259,
        expected_fragments: &[],
    },
    BundleFixture {
        name: "vendor 20260528T214319Z-play-local dseq 0037",
        body: include_bytes!(
            "fixtures/vendor/20260528T214319Z-play-local_state_bundle_dseq0037_body.bin"
        ),
        seq: SequenceNumber::Seq(1),
        bandwidth_mode: 1,
        is_unreliable: true,
        has_replication_control: true,
        stop_replication_ids: &[],
        pause_replication_ids: &[],
        bundle_buffer_len: 1_273,
        expected_fragments: &[],
    },
];

#[test]
fn live_state_bundle_bodies_parse_without_transport_stack() {
    for fixture in BUNDLE_FIXTURES {
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, fixture.body);
        let view = ReplicatedStateBundleView::read_from(&mut rb)
            .unwrap_or_else(|err| panic!("{}: {err}", fixture.name));

        assert_eq!(view.seq, fixture.seq, "{}", fixture.name);
        assert_eq!(view.client_context_instance_id, 1, "{}", fixture.name);
        assert_eq!(
            view.bandwidth_mode, fixture.bandwidth_mode,
            "{}",
            fixture.name
        );
        assert_eq!(
            view.is_unreliable, fixture.is_unreliable,
            "{}",
            fixture.name
        );
        assert_eq!(
            view.has_replication_control(),
            fixture.has_replication_control,
            "{}",
            fixture.name
        );
        assert_eq!(
            view.stop_replication_ids(),
            fixture.stop_replication_ids,
            "{}",
            fixture.name
        );
        assert_eq!(
            view.pause_replication_ids(),
            fixture.pause_replication_ids,
            "{}",
            fixture.name
        );
        assert_eq!(view.bundle_buffer.len(), fixture.bundle_buffer_len);
        assert_eq!(view.total_bundle_size(), fixture.body.len());
        assert_eq!(rb.left(), 0, "{}", fixture.name);
    }
}

#[test]
fn live_state_bundle_modeled_fragments_decode() {
    for fixture in BUNDLE_FIXTURES
        .iter()
        .filter(|fixture| !fixture.expected_fragments.is_empty())
    {
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, fixture.body);
        let view = ReplicatedStateBundleView::read_from(&mut rb)
            .unwrap_or_else(|err| panic!("{}: {err}", fixture.name));
        let mut fragments = Vec::new();
        for fragment in view.fragments() {
            match fragment {
                Ok(fragment) => fragments.push(fragment),
                Err(err) => panic!("{}: state fragments: {err}", fixture.name),
            }
        }
        assert!(!fragments.is_empty(), "{}", fixture.name);
        assert_eq!(
            fragments.last().unwrap().body_range.end,
            view.bundle_buffer.len(),
            "{}",
            fixture.name
        );
        assert_eq!(
            fragments.len(),
            fixture.expected_fragments.len(),
            "{}",
            fixture.name
        );

        for (fragment, expected) in fragments.iter().zip(fixture.expected_fragments) {
            assert_eq!(
                fragment.record.interest_id.get(),
                expected.interest_id,
                "{}",
                fixture.name
            );
            assert_eq!(
                fragment.record.fragment_count, expected.fragment_count,
                "{}",
                fixture.name
            );
            assert_eq!(
                fragment.header.fragment_key.get(),
                expected.fragment_key,
                "{}",
                fixture.name
            );
            assert_eq!(
                fragment.header.type_info,
                FragmentTypeInfo::TypeIndex(expected.type_index),
                "{}",
                fixture.name
            );
            assert_eq!(fragment.body.len(), expected.body_len, "{}", fixture.name);
            assert_expected_state(fixture.name, fragment, expected.state);
        }
    }
}

#[test]
fn live_state_702_walks_instanced_slayer_script_into_progression() {
    let fixture = BUNDLE_FIXTURES
        .iter()
        .find(|fixture| fixture.seq == SequenceNumber::Seq(702))
        .expect("state 702 fixture is present");
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, fixture.body);
    let view = ReplicatedStateBundleView::read_from(&mut rb)
        .unwrap_or_else(|err| panic!("{}: {err}", fixture.name));

    let fragments = view
        .fragments()
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("{}: state fragments: {err}", fixture.name));

    assert_eq!(fragments.len(), 33, "{}", fixture.name);
    assert_eq!(
        fragments.last().unwrap().body_range.end,
        view.bundle_buffer.len(),
        "{}",
        fixture.name
    );

    let slayer_index = fragments
        .iter()
        .position(|fragment| {
            fragment.header.type_info
                == FragmentTypeInfo::TypeIndex(InstancedSlayerScriptReplicatedState::TYPE_INDEX)
        })
        .unwrap_or_else(|| panic!("{}: missing instanced slayer script state", fixture.name));
    let slayer = &fragments[slayer_index];
    assert_eq!(slayer.body_range, 0x113..0x121, "{}", fixture.name);

    let slayer = slayer
        .decode::<InstancedSlayerScriptReplicatedState>()
        .unwrap_or_else(|err| panic!("{}: instanced slayer script body: {err}", fixture.name));
    assert!(
        slayer.spawned_entity_ids_by_spawner_id.values().is_empty(),
        "{}",
        fixture.name
    );
    assert!(slayer.synced_timers.values().is_empty(), "{}", fixture.name);

    let progression = &fragments[slayer_index + 1];
    assert_eq!(progression.header.start, 0x121, "{}", fixture.name);
    assert_eq!(
        progression.header.fragment_key.get(),
        70,
        "{}",
        fixture.name
    );
    assert_eq!(
        progression.header.type_info,
        FragmentTypeInfo::TypeIndex(ProgressionComponentReplicatedState::TYPE_INDEX),
        "{}",
        fixture.name
    );
    progression
        .decode::<ProgressionComponentReplicatedState>()
        .unwrap_or_else(|err| panic!("{}: progression body: {err}", fixture.name));

    let affiliation = fragments
        .iter()
        .find(|fragment| {
            fragment.header.type_info
                == FragmentTypeInfo::TypeIndex(TemporaryAffiliationReplicatedState::TYPE_INDEX)
        })
        .unwrap_or_else(|| panic!("{}: missing temporary affiliation state", fixture.name));
    assert_eq!(affiliation.body_range, 0x338..0x340, "{}", fixture.name);
    assert_eq!(
        affiliation.header.fragment_key.get(),
        17,
        "{}",
        fixture.name
    );

    let affiliation = affiliation
        .decode::<TemporaryAffiliationReplicatedState>()
        .unwrap_or_else(|err| panic!("{}: temporary affiliation body: {err}", fixture.name));
    assert!(
        affiliation.affiliations.values().is_empty(),
        "{}",
        fixture.name
    );
    assert!(
        affiliation.affiliations.current_changes().is_empty(),
        "{}",
        fixture.name
    );

    let last = fragments.last().unwrap();
    assert_eq!(last.record.interest_id.get(), 101, "{}", fixture.name);
    assert_eq!(
        last.header.type_info,
        FragmentTypeInfo::TypeIndex(InteractReplicatedState::TYPE_INDEX),
        "{}",
        fixture.name
    );
    assert_eq!(last.body_range, 0x3df..0x3e5, "{}", fixture.name);
}

#[test]
fn full_live_and_vendor_bundles_walk_all_fragments() {
    let cases = [("20260619T021014Z-pid65972 dseq 0045", 48)];

    for (fixture_name, fragment_count) in cases {
        let fixture = BUNDLE_FIXTURES
            .iter()
            .find(|fixture| fixture.name == fixture_name)
            .unwrap_or_else(|| panic!("{fixture_name}: fixture is present"));
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, fixture.body);
        let view = ReplicatedStateBundleView::read_from(&mut rb)
            .unwrap_or_else(|err| panic!("{}: {err}", fixture.name));
        let fragments = view
            .fragments()
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|err| panic!("{}: state fragments: {err}", fixture.name));

        assert_eq!(fragments.len(), fragment_count, "{}", fixture.name);
        assert_eq!(
            fragments.last().unwrap().body_range.end,
            view.bundle_buffer.len(),
            "{}",
            fixture.name
        );
    }
}

#[test]
fn vendor_dseq0037_stops_at_non_fragment_type_index_67() {
    let fixture = BUNDLE_FIXTURES
        .iter()
        .find(|fixture| fixture.name == "vendor 20260528T214319Z-play-local dseq 0037")
        .expect("vendor fixture is present");
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, fixture.body);
    let view = ReplicatedStateBundleView::read_from(&mut rb)
        .unwrap_or_else(|err| panic!("{}: {err}", fixture.name));

    let mut fragments = view.fragments();
    for index in 0..28 {
        fragments
            .next()
            .unwrap_or_else(|| panic!("{}: missing fragment {index}", fixture.name))
            .unwrap_or_else(|err| panic!("{}: fragment {index}: {err}", fixture.name));
    }

    let err = fragments
        .next()
        .expect("vendor fixture reaches typeIndex 67")
        .unwrap_err();
    assert!(
        matches!(err, MarshalerError::UnknownTypeIndex { type_index: 67 }),
        "{}: expected non-fragment typeIndex 67, got {err}",
        fixture.name
    );
    let coverage = validate_state_fragment_type_indices([67]);
    assert_eq!(coverage.unknown_type_indices, Vec::<u32>::new());
    assert_eq!(coverage.non_replicated_state_type_indices, vec![67]);
    assert_eq!(
        coverage.unregistered_replicated_state_type_indices,
        Vec::<u32>::new()
    );
    assert_eq!(
        coverage.registered_replicated_state_type_indices,
        Vec::<u32>::new()
    );
}

fn assert_expected_state(
    name: &str,
    fragment: &nw_network::hub::StateFragmentView<'_>,
    state: ExpectedState,
) {
    match state {
        ExpectedState::Alc {
            state_id,
            time_offset_milliseconds,
            look_dir,
            rotation,
        } => {
            let alc = fragment
                .decode::<ALCReplicatedState>()
                .unwrap_or_else(|err| panic!("{name}: ALC fragment body: {err}"));
            assert_eq!(alc.state_id(), Some(state_id), "{name}");
            assert_eq!(
                alc.time_offset_milliseconds(),
                Some(time_offset_milliseconds),
                "{name}"
            );
            assert_vec3_close(name, alc.look_dir.value().copied(), look_dir);
            match rotation {
                Some(rotation) => assert_quat_close(name, alc.rotation.value().copied(), rotation),
                None => assert_eq!(alc.rotation.value(), None, "{name}"),
            }
        }
        ExpectedState::Interact { has_interactors } => {
            let interact = fragment
                .decode::<InteractReplicatedState>()
                .unwrap_or_else(|err| panic!("{name}: interact fragment body: {err}"));
            assert_eq!(interact.enabled.value(), None, "{name}");
            assert_eq!(
                interact.has_interactors.value().copied(),
                Some(has_interactors),
                "{name}"
            );
            assert!(interact.cooldown_updates.values().is_empty(), "{name}");
        }
        ExpectedState::AggregateContractCount {
            total_buy_contracts,
            total_sell_contracts,
        } => {
            let aggregate = fragment
                .decode::<AggregateContractCountComponentReplicatedState>()
                .unwrap_or_else(|err| panic!("{name}: aggregate contract-count body: {err}"));
            assert_eq!(
                aggregate.total_buy_contracts.value().copied(),
                Some(total_buy_contracts),
                "{name}"
            );
            assert_eq!(
                aggregate.total_sell_contracts.value().copied(),
                Some(total_sell_contracts),
                "{name}"
            );
            assert!(aggregate.buy_category_counts.values().is_empty(), "{name}");
            assert!(aggregate.buy_family_counts.values().is_empty(), "{name}");
            assert!(aggregate.buy_group_counts.values().is_empty(), "{name}");
            assert!(aggregate.buy_item_counts.values().is_empty(), "{name}");
            assert!(aggregate.sell_category_counts.values().is_empty(), "{name}");
            assert!(aggregate.sell_family_counts.values().is_empty(), "{name}");
            assert!(aggregate.sell_group_counts.values().is_empty(), "{name}");
            assert!(aggregate.sell_item_counts.values().is_empty(), "{name}");
        }
    }
}

fn assert_vec3_close(name: &str, actual: Option<glam::Vec3>, expected: [f32; 3]) {
    let actual = actual.unwrap_or_else(|| panic!("{name}: expected Vec3 value"));
    assert!(
        (actual.x - expected[0]).abs() < 0.000_01
            && (actual.y - expected[1]).abs() < 0.000_01
            && (actual.z - expected[2]).abs() < 0.000_01,
        "{name}: actual={actual:?} expected={expected:?}"
    );
}

fn assert_quat_close(name: &str, actual: Option<glam::Quat>, expected: [f32; 4]) {
    let actual = actual.unwrap_or_else(|| panic!("{name}: expected Quat value"));
    assert!(
        (actual.x - expected[0]).abs() < 0.000_01
            && (actual.y - expected[1]).abs() < 0.000_01
            && (actual.z - expected[2]).abs() < 0.000_01
            && (actual.w - expected[3]).abs() < 0.000_01,
        "{name}: actual={actual:?} expected={expected:?}"
    );
}
