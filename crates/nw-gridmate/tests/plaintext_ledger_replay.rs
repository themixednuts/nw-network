use std::{
    collections::{BTreeMap, HashMap},
    env, fmt, fs,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
};

use bytes::Bytes;
use nw_gridmate::{
    MarshalerError, ReadBuffer,
    carrier::{MessageData, Receiver},
    message::{MessageEnvelopeView, read_correlated_message, read_sized_message},
    network_schema,
    serialize::CARRIER_ENDIAN,
};
use nw_network::{
    FragmentTypeInfo, ReplicatedStateBundleView,
    hub::{read_state_fragment_header, read_state_record_header},
};

const LEDGER_MAGIC: &[u8; 4] = b"NWDL";
const LEDGER_HEADER_LEN: usize = 28;
const CHANNEL_GAME_DATA: u8 = 0;
const CHANNEL_REPLICATED_STATE: u8 = 1;
const REPLICATED_STATE_BUNDLE_TYPE_INDEX: u32 = 8;
const REPLICATED_STATE_BUNDLE_TYPE_NAME: &str = "Amazon::Hub::ReplicatedStateBundle";

#[test]
fn replays_plaintext_ledgers_from_env() {
    let ledgers = ledger_inputs();
    if ledgers.is_empty() {
        eprintln!(
            "skipping plaintext ledger replay; set NW_GRIDMATE_REPLAY_LEDGER or \
             NW_GRIDMATE_REPLAY_LEDGER_DIR"
        );
        return;
    }

    let dump_config = ReplayDumpConfig::from_env();
    let strict = env_flag("NW_GRIDMATE_REPLAY_STRICT");

    let mut total = ReplayStats::default();
    for ledger in ledgers {
        let stats = replay_ledger(&ledger, &dump_config)
            .unwrap_or_else(|err| panic!("{}: {err}", ledger.display()));
        eprintln!(
            "{}: records={} datagrams={} carrier_messages={} channels={:?} hub={} state_bundles={} state_fragments={} state_decode_errors={}",
            ledger.display(),
            stats.records,
            stats.datagrams,
            stats.carrier_messages,
            stats.channels,
            stats.hub_messages,
            stats.state_bundles,
            stats.state_fragments,
            stats.state_fragment_decode_errors
        );
        total.merge(stats);
    }

    assert!(total.records > 0, "expected at least one ledger record");
    assert!(
        total.datagrams > 0,
        "expected at least one carrier datagram"
    );
    assert!(
        total.carrier_messages > 0,
        "expected at least one reassembled carrier message"
    );

    if strict {
        assert_eq!(
            total.hub_parse_errors, 0,
            "Hub envelope parse errors: {:?}",
            total.hub_errors
        );
        assert_eq!(
            total.state_bundle_parse_errors, 0,
            "state bundle parse errors: {:?}",
            total.state_bundle_errors
        );
        assert_eq!(
            total.state_fragment_iter_errors, 0,
            "state fragment iterator errors: {:?}",
            total.state_fragment_errors
        );
        assert_eq!(
            total.state_fragment_decode_errors, 0,
            "state fragment decode errors: {:?}",
            total.state_fragment_errors
        );
    }
}

#[derive(Debug, Clone)]
struct ReplayDumpConfig {
    summary_dir: Option<PathBuf>,
    detail_limit: usize,
    value_char_limit: usize,
    hex_byte_limit: usize,
}

impl ReplayDumpConfig {
    fn from_env() -> Self {
        let detail_limit = env_limit("NW_GRIDMATE_REPLAY_DETAIL_LIMIT", 2_000);
        Self {
            summary_dir: env::var_os("NW_GRIDMATE_REPLAY_SUMMARY_DIR").map(PathBuf::from),
            detail_limit,
            value_char_limit: env_limit("NW_GRIDMATE_REPLAY_VALUE_CHARS", 2_048),
            hex_byte_limit: env_limit("NW_GRIDMATE_REPLAY_HEX_BYTES", 64),
        }
    }
}

struct ReplayReport {
    ledger_name: String,
    summary_path: Option<PathBuf>,
    detail_path: Option<PathBuf>,
    detail: Option<File>,
    detail_limit: usize,
    value_char_limit: usize,
    hex_byte_limit: usize,
    detail_lines: usize,
    detail_truncated: bool,
}

impl ReplayReport {
    fn new(ledger: &Path, config: &ReplayDumpConfig) -> Result<Self, String> {
        let ledger_name = report_name_for_ledger(ledger);
        let Some(summary_dir) = &config.summary_dir else {
            return Ok(Self {
                ledger_name,
                summary_path: None,
                detail_path: None,
                detail: None,
                detail_limit: config.detail_limit,
                value_char_limit: config.value_char_limit,
                hex_byte_limit: config.hex_byte_limit,
                detail_lines: 0,
                detail_truncated: false,
            });
        };

        fs::create_dir_all(summary_dir)
            .map_err(|err| format!("create summary dir {}: {err}", summary_dir.display()))?;
        let summary_path = summary_dir.join(format!("{ledger_name}.summary.md"));
        let detail_path = summary_dir.join(format!("{ledger_name}.detail.log"));
        let detail = File::create(&detail_path)
            .map_err(|err| format!("create detail log {}: {err}", detail_path.display()))?;

        Ok(Self {
            ledger_name,
            summary_path: Some(summary_path),
            detail_path: Some(detail_path),
            detail: Some(detail),
            detail_limit: config.detail_limit,
            value_char_limit: config.value_char_limit,
            hex_byte_limit: config.hex_byte_limit,
            detail_lines: 0,
            detail_truncated: false,
        })
    }

    fn enabled(&self) -> bool {
        self.summary_path.is_some()
    }

    fn detail_line(&mut self, line: impl AsRef<str>) -> Result<(), String> {
        let Some(detail) = &mut self.detail else {
            return Ok(());
        };
        if self.detail_lines >= self.detail_limit {
            self.detail_truncated = true;
            return Ok(());
        }
        writeln!(detail, "{}", line.as_ref()).map_err(|err| format!("write detail line: {err}"))?;
        self.detail_lines += 1;
        Ok(())
    }

    fn value(&self, value: impl fmt::Debug) -> String {
        compact_debug(value, self.value_char_limit)
    }

    fn hex(&self, bytes: &[u8]) -> String {
        hex_prefix(bytes, self.hex_byte_limit)
    }

    fn finish(mut self, ledger: &Path, stats: &ReplayStats) -> Result<(), String> {
        let Some(summary_path) = self.summary_path.take() else {
            return Ok(());
        };
        let mut summary = File::create(&summary_path)
            .map_err(|err| format!("create summary {}: {err}", summary_path.display()))?;
        write_summary_markdown(&mut summary, ledger, stats, &self)
            .map_err(|err| format!("write summary {}: {err}", summary_path.display()))?;
        eprintln!(
            "wrote replay summary: {}{}",
            summary_path.display(),
            self.detail_path
                .as_ref()
                .map(|path| format!(" and {}", path.display()))
                .unwrap_or_default()
        );
        Ok(())
    }
}

#[derive(Debug, Default)]
struct ReplayStats {
    records: usize,
    datagrams: usize,
    carrier_messages: usize,
    channels: BTreeMap<u8, usize>,

    hub_messages: usize,
    hub_empty_envelopes: usize,
    hub_ambiguous_flows: usize,
    hub_parse_errors: usize,
    hub_types: BTreeMap<TypeKey, TrafficStats>,
    hub_errors: BTreeMap<String, usize>,

    state_lane_payloads: usize,
    state_bundles: usize,
    state_wrapped_bundles: usize,
    state_bundle_bytes: usize,
    state_bundles_with_replication_control: usize,
    state_bundle_parse_errors: usize,
    state_fragments: usize,
    state_fragment_iter_errors: usize,
    state_fragment_decode_errors: usize,
    state_types: BTreeMap<TypeKey, TrafficStats>,
    state_bundle_errors: BTreeMap<String, usize>,
    state_fragment_errors: BTreeMap<String, usize>,
}

impl ReplayStats {
    fn merge(&mut self, other: Self) {
        self.records += other.records;
        self.datagrams += other.datagrams;
        self.carrier_messages += other.carrier_messages;
        self.hub_messages += other.hub_messages;
        self.hub_empty_envelopes += other.hub_empty_envelopes;
        self.hub_ambiguous_flows += other.hub_ambiguous_flows;
        self.hub_parse_errors += other.hub_parse_errors;
        self.state_lane_payloads += other.state_lane_payloads;
        self.state_bundles += other.state_bundles;
        self.state_wrapped_bundles += other.state_wrapped_bundles;
        self.state_bundle_bytes += other.state_bundle_bytes;
        self.state_bundles_with_replication_control += other.state_bundles_with_replication_control;
        self.state_bundle_parse_errors += other.state_bundle_parse_errors;
        self.state_fragments += other.state_fragments;
        self.state_fragment_iter_errors += other.state_fragment_iter_errors;
        self.state_fragment_decode_errors += other.state_fragment_decode_errors;

        merge_counts(&mut self.channels, other.channels);
        merge_type_stats(&mut self.hub_types, other.hub_types);
        merge_type_stats(&mut self.state_types, other.state_types);
        merge_counts(&mut self.hub_errors, other.hub_errors);
        merge_counts(&mut self.state_bundle_errors, other.state_bundle_errors);
        merge_counts(&mut self.state_fragment_errors, other.state_fragment_errors);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TypeKey {
    path: &'static str,
    type_index: Option<u32>,
    name: String,
}

#[derive(Debug, Default, Clone)]
struct TrafficStats {
    count: usize,
    bytes: usize,
    decoded: usize,
    errors: usize,
}

impl TrafficStats {
    fn observe(&mut self, bytes: usize) {
        self.count += 1;
        self.bytes += bytes;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FlowKey {
    direction: u32,
    stream: u32,
    lane: u32,
}

#[derive(Debug, Clone, Copy)]
struct MessageContext {
    record_offset: usize,
    direction: u32,
    stream: u32,
    lane: u32,
    carrier_sequence: u16,
    channel: u8,
}

fn replay_ledger(path: &Path, dump_config: &ReplayDumpConfig) -> Result<ReplayStats, String> {
    let data = fs::read(path).map_err(|err| format!("read ledger: {err}"))?;
    let mut stats = ReplayStats::default();
    let mut receivers: HashMap<FlowKey, Receiver> = HashMap::new();
    let mut report = ReplayReport::new(path, dump_config)?;
    let mut offset = 0usize;

    while offset < data.len() {
        let header_offset = offset;
        if data.len() - offset < LEDGER_HEADER_LEN {
            return Err(format!(
                "trailing {} bytes after final ledger record",
                data.len() - offset
            ));
        }
        if &data[offset..offset + 4] != LEDGER_MAGIC {
            return Err(format!(
                "bad ledger magic at offset {offset}: {:02x?}",
                &data[offset..offset + 4]
            ));
        }

        let direction = read_u32_le(&data, offset + 12)?;
        let stream = read_u32_le(&data, offset + 16)?;
        let lane = read_u32_le(&data, offset + 20)?;
        let payload_len = read_u32_le(&data, offset + 24)? as usize;
        offset += LEDGER_HEADER_LEN;

        let end = offset
            .checked_add(payload_len)
            .ok_or_else(|| format!("payload length overflow at offset {header_offset}"))?;
        let payload = data
            .get(offset..end)
            .ok_or_else(|| format!("payload at offset {header_offset} exceeds file length"))?;

        let key = FlowKey {
            direction,
            stream,
            lane,
        };
        let receiver = receivers.entry(key).or_default();
        let fed = receiver
            .feed(Bytes::copy_from_slice(payload))
            .map_err(|err| format!("carrier decode at offset {header_offset}: {err}"))?;

        stats.records += 1;
        stats.datagrams += 1;
        let carrier_sequence = fed.header.sequence_number.get();
        for (channel, message) in fed.into_messages() {
            stats.carrier_messages += 1;
            *stats.channels.entry(channel).or_default() += 1;

            let ctx = MessageContext {
                record_offset: header_offset,
                direction,
                stream,
                lane,
                carrier_sequence,
                channel,
            };
            summarize_carrier_message(ctx, &message, &mut stats, &mut report)?;
        }

        offset = end;
    }

    report.finish(path, &stats)?;
    Ok(stats)
}

fn summarize_carrier_message(
    ctx: MessageContext,
    message: &MessageData,
    stats: &mut ReplayStats,
    report: &mut ReplayReport,
) -> Result<(), String> {
    match ctx.channel {
        CHANNEL_GAME_DATA => summarize_hub_message(ctx, message, stats, report, false),
        CHANNEL_REPLICATED_STATE => {
            stats.state_lane_payloads += 1;
            summarize_hub_message(ctx, message, stats, report, true)
        }
        _ => {
            if report.enabled() {
                report.detail_line(format!(
                    "kind=carrier channel={} offset={} dir={} stream={} lane={} carrier_seq={} bytes={}",
                    ctx.channel,
                    ctx.record_offset,
                    ctx.direction,
                    ctx.stream,
                    ctx.lane,
                    ctx.carrier_sequence,
                    message.data.len()
                ))?;
            }
            Ok(())
        }
    }
}

fn summarize_hub_message(
    ctx: MessageContext,
    message: &MessageData,
    stats: &mut ReplayStats,
    report: &mut ReplayReport,
    decode_state_bundles: bool,
) -> Result<(), String> {
    let selected = if let Some(path) = hub_wire_path_for_direction(ctx.direction) {
        match parse_hub_stream(message.data.as_ref(), path) {
            Ok(stream) => stream,
            Err(error) => {
                record_hub_parse_error(ctx, message.data.len(), error, stats, report)?;
                return Ok(());
            }
        }
    } else {
        let sized = parse_hub_stream(message.data.as_ref(), HubWirePath::SizedServerToClient);
        let correlated =
            parse_hub_stream(message.data.as_ref(), HubWirePath::CorrelatedClientToServer);

        match (sized, correlated) {
            (Ok(sized), Err(_)) => sized,
            (Err(_), Ok(correlated)) => correlated,
            (Ok(sized), Ok(correlated)) => {
                stats.hub_ambiguous_flows += 1;
                let selected = if correlated.envelopes.len() > sized.envelopes.len() {
                    correlated
                } else {
                    sized
                };
                report.detail_line(format!(
                    "kind=hub_ambiguous offset={} dir={} stream={} lane={} carrier_seq={} selected_path={} bytes={}",
                    ctx.record_offset,
                    ctx.direction,
                    ctx.stream,
                    ctx.lane,
                    ctx.carrier_sequence,
                    selected.path,
                    message.data.len()
                ))?;
                selected
            }
            (Err(sized), Err(correlated)) => {
                let error = format!("sized={sized}; correlated={correlated}");
                record_hub_parse_error(ctx, message.data.len(), error, stats, report)?;
                return Ok(());
            }
        }
    };

    stats.hub_empty_envelopes += selected.empty_envelopes;
    for envelope in &selected.envelopes {
        stats.hub_messages += 1;
        let key = TypeKey {
            path: selected.path.as_str(),
            type_index: envelope.type_index,
            name: envelope.name.clone(),
        };
        stats
            .hub_types
            .entry(key)
            .or_default()
            .observe(envelope.body_len);
        report.detail_line(format!(
            "kind=hub path={} offset={} dir={} stream={} lane={} carrier_seq={} type_index={} type_id={} name={} body_len={} body_prefix={}",
            selected.path,
            ctx.record_offset,
            ctx.direction,
            ctx.stream,
            ctx.lane,
            ctx.carrier_sequence,
            display_optional_u32(envelope.type_index),
            envelope.type_id,
            envelope.name,
            envelope.body_len,
            envelope.body_prefix
        ))?;

        if decode_state_bundles && is_replicated_state_bundle_envelope(envelope) {
            summarize_state_bundle_body(ctx, selected.path, envelope, stats, report)?;
        }
    }

    Ok(())
}

fn summarize_state_bundle_body(
    ctx: MessageContext,
    path: HubWirePath,
    envelope: &HubEnvelopeSummary<'_>,
    stats: &mut ReplayStats,
    report: &mut ReplayReport,
) -> Result<(), String> {
    stats.state_bundles += 1;
    stats.state_bundle_bytes += envelope.body.len();

    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, envelope.body);
    let view = match ReplicatedStateBundleView::read_from(&mut rb) {
        Ok(view) if rb.is_empty() => view,
        Ok(view) => {
            stats.state_bundle_parse_errors += 1;
            let error = format!(
                "state bundle body has {} trailing bytes after {} parsed bytes",
                rb.left(),
                view.total_bundle_size()
            );
            *stats.state_bundle_errors.entry(error.clone()).or_default() += 1;
            report.detail_line(format!(
                "kind=state_bundle_error path={} offset={} dir={} stream={} lane={} carrier_seq={} type_index={} bytes={} error={} body_prefix={}",
                path,
                ctx.record_offset,
                ctx.direction,
                ctx.stream,
                ctx.lane,
                ctx.carrier_sequence,
                display_optional_u32(envelope.type_index),
                envelope.body.len(),
                one_line(error),
                hex_prefix(envelope.body, 64)
            ))?;
            return Ok(());
        }
        Err(err) => {
            stats.state_bundle_parse_errors += 1;
            let error = err.to_string();
            *stats.state_bundle_errors.entry(error.clone()).or_default() += 1;
            report.detail_line(format!(
                "kind=state_bundle_error path={} offset={} dir={} stream={} lane={} carrier_seq={} type_index={} bytes={} error={} body_prefix={}",
                path,
                ctx.record_offset,
                ctx.direction,
                ctx.stream,
                ctx.lane,
                ctx.carrier_sequence,
                display_optional_u32(envelope.type_index),
                envelope.body.len(),
                one_line(error),
                hex_prefix(envelope.body, 64)
            ))?;
            return Ok(());
        }
    };

    stats.state_wrapped_bundles += 1;
    if view.has_replication_control() {
        stats.state_bundles_with_replication_control += 1;
    }
    report.detail_line(format!(
        "kind=state_bundle path={} offset={} dir={} stream={} lane={} carrier_seq={} type_index={} seq={:?} context={} bandwidth={} unreliable={} replication_control_ids={} bundle_bytes={} total_bytes={}",
        path,
        ctx.record_offset,
        ctx.direction,
        ctx.stream,
        ctx.lane,
        ctx.carrier_sequence,
        display_optional_u32(envelope.type_index),
        view.seq,
        view.client_context_instance_id,
        view.bandwidth_mode,
        view.is_unreliable,
        view.replication_control_count(),
        view.bundle_buffer.len(),
        view.total_bundle_size()
    ))?;

    summarize_state_fragments_from_buffer(
        ctx,
        "hub_bundle",
        format!("{:?}", view.seq),
        view.bundle_buffer,
        stats,
        report,
    )?;

    Ok(())
}

fn summarize_state_fragments_from_buffer(
    ctx: MessageContext,
    source: &'static str,
    sequence: String,
    bundle_buffer: &[u8],
    stats: &mut ReplayStats,
    report: &mut ReplayReport,
) -> Result<usize, String> {
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, bundle_buffer);
    let mut fragment_count = 0usize;

    while !rb.is_empty() {
        let mut decoded_record_fragments = Vec::new();
        let record_start = rb.position();
        let record = match read_state_record_header(&mut rb) {
            Ok(record) => record,
            Err(err) => {
                record_state_fragment_error(ctx, source, &sequence, err, stats, report, None)?;
                break;
            }
        };
        let record_header_end = rb.position();

        for ordinal in 0..record.fragment_count {
            let header_start = rb.position();
            let header = match read_state_fragment_header(&mut rb) {
                Ok(header) => header,
                Err(err) => {
                    let context = format!(
                        "record_start={} record_header_end={} interest={} fragment_ordinal={}/{} cursor={} header_window={} tail_prefix={} nearby_headers={} next_registered_headers={} resync_candidates={} hidden_fragment_candidates={}",
                        record_start,
                        record_header_end,
                        record.interest_id.get(),
                        ordinal,
                        record.fragment_count,
                        header_start,
                        hex_window(bundle_buffer, header_start, 8, 40),
                        report.hex(rb.remaining()),
                        nearby_fragment_headers(bundle_buffer, header_start),
                        next_registered_fragment_headers(bundle_buffer, header_start),
                        resync_fragment_candidates(
                            bundle_buffer,
                            header_start,
                            record.fragment_count - ordinal
                        ),
                        hidden_fragment_candidates(
                            bundle_buffer,
                            &decoded_record_fragments,
                            record.fragment_count,
                            header_start
                        )
                    );
                    record_state_fragment_error(
                        ctx,
                        source,
                        &sequence,
                        err,
                        stats,
                        report,
                        Some(context),
                    )?;
                    return Ok(fragment_count);
                }
            };

            let body_start = rb.position();
            let fragment = match header.type_info.decode_contents(&mut rb) {
                Ok(fragment) => fragment,
                Err(err) => {
                    let context = format!(
                        "record_start={} record_header_end={} interest={} fragment_ordinal={}/{} header_start={} header_end={} fragment_key={} type_info={} body_start={} header_prefix={} header_window={} body_tail={} tail_prefix={} nearby_headers={} next_registered_headers={} resync_candidates={} hidden_fragment_candidates={}",
                        record_start,
                        record_header_end,
                        record.interest_id.get(),
                        ordinal,
                        record.fragment_count,
                        header_start,
                        body_start,
                        header.fragment_key.get(),
                        display_fragment_type_info(header.type_info),
                        body_start,
                        hex_range(bundle_buffer, header_start, body_start),
                        hex_window(bundle_buffer, header_start, 8, 40),
                        report.hex(&bundle_buffer[body_start..]),
                        report.hex(rb.remaining()),
                        nearby_fragment_headers(bundle_buffer, header_start),
                        next_registered_fragment_headers(bundle_buffer, header_start),
                        resync_fragment_candidates(
                            bundle_buffer,
                            header_start,
                            record.fragment_count - ordinal
                        ),
                        hidden_fragment_candidates(
                            bundle_buffer,
                            &decoded_record_fragments,
                            record.fragment_count,
                            rb.position()
                        )
                    );
                    record_state_fragment_error(
                        ctx,
                        source,
                        &sequence,
                        err,
                        stats,
                        report,
                        Some(context),
                    )?;
                    return Ok(fragment_count);
                }
            };

            let body_end = rb.position();
            let body = rb
                .range(body_start..body_end)
                .map_err(|err| format!("state fragment body range: {err}"))?;

            fragment_count += 1;
            stats.state_fragments += 1;
            let (type_index, name) = fragment_type_name(header.type_info);
            decoded_record_fragments.push(DecodedStateFragmentSpan {
                ordinal,
                header_start,
                header_end: body_start,
                body_start,
                body_end,
                fragment_key: header.fragment_key.get(),
                type_info: header.type_info,
                name: name.clone(),
            });
            let key = TypeKey {
                path: "state",
                type_index,
                name: name.clone(),
            };
            let type_stats = stats.state_types.entry(key).or_default();
            type_stats.observe(body.len());
            type_stats.decoded += 1;
            report.detail_line(format!(
                "kind=state_fragment source={} offset={} dir={} stream={} lane={} carrier_seq={} seq={} record_start={} record_header_end={} interest={} fragment_ordinal={}/{} header_start={} header_end={} body_start={} body_end={} fragment_key={} type_index={} name={} body_len={} body_prefix={} value={}",
                source,
                ctx.record_offset,
                ctx.direction,
                ctx.stream,
                ctx.lane,
                ctx.carrier_sequence,
                sequence,
                record_start,
                record_header_end,
                record.interest_id.get(),
                ordinal,
                record.fragment_count,
                header_start,
                body_start,
                body_start,
                body_end,
                header.fragment_key.get(),
                display_optional_u32(type_index),
                name,
                body.len(),
                report.hex(body),
                report.value(fragment.as_ref())
            ))?;
        }
    }

    Ok(fragment_count)
}

#[derive(Debug, Clone)]
struct DecodedStateFragmentSpan {
    ordinal: usize,
    header_start: usize,
    header_end: usize,
    body_start: usize,
    body_end: usize,
    fragment_key: u32,
    type_info: FragmentTypeInfo,
    name: String,
}

fn record_state_fragment_error(
    ctx: MessageContext,
    source: &'static str,
    sequence: &str,
    err: MarshalerError,
    stats: &mut ReplayStats,
    report: &mut ReplayReport,
    context: Option<String>,
) -> Result<(), String> {
    let error = err.to_string();
    if is_fragment_decode_error(&err) {
        stats.state_fragment_decode_errors += 1;
    } else {
        stats.state_fragment_iter_errors += 1;
    }
    *stats
        .state_fragment_errors
        .entry(error.clone())
        .or_default() += 1;
    report.detail_line(format!(
        "kind=state_fragment_iter_error source={} offset={} dir={} stream={} lane={} carrier_seq={} seq={} error={} context={}",
        source,
        ctx.record_offset,
        ctx.direction,
        ctx.stream,
        ctx.lane,
        ctx.carrier_sequence,
        sequence,
        one_line(error),
        context.map_or_else(|| "-".to_owned(), one_line)
    ))
}

fn is_fragment_decode_error(err: &MarshalerError) -> bool {
    matches!(
        err,
        MarshalerError::UnknownTypeIndex { .. } | MarshalerError::UnknownClassUuid
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HubWirePath {
    SizedServerToClient,
    CorrelatedClientToServer,
}

impl HubWirePath {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SizedServerToClient => "server_to_client_sized",
            Self::CorrelatedClientToServer => "client_to_server_correlated",
        }
    }
}

impl fmt::Display for HubWirePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn hub_wire_path_for_direction(direction: u32) -> Option<HubWirePath> {
    match direction {
        0 => Some(HubWirePath::SizedServerToClient),
        1 => Some(HubWirePath::CorrelatedClientToServer),
        _ => None,
    }
}

fn record_hub_parse_error(
    ctx: MessageContext,
    bytes: usize,
    error: String,
    stats: &mut ReplayStats,
    report: &mut ReplayReport,
) -> Result<(), String> {
    stats.hub_parse_errors += 1;
    *stats.hub_errors.entry(error.clone()).or_default() += 1;
    report.detail_line(format!(
        "kind=hub_error offset={} dir={} stream={} lane={} carrier_seq={} bytes={} error={}",
        ctx.record_offset,
        ctx.direction,
        ctx.stream,
        ctx.lane,
        ctx.carrier_sequence,
        bytes,
        one_line(error)
    ))
}

struct ParsedHubStream<'a> {
    path: HubWirePath,
    envelopes: Vec<HubEnvelopeSummary<'a>>,
    empty_envelopes: usize,
}

struct HubEnvelopeSummary<'a> {
    type_index: Option<u32>,
    type_id: String,
    name: String,
    body: &'a [u8],
    body_len: usize,
    body_prefix: String,
}

fn parse_hub_stream<'a>(data: &'a [u8], path: HubWirePath) -> Result<ParsedHubStream<'a>, String> {
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, data);
    let mut envelopes = Vec::new();
    let mut empty_envelopes = 0usize;

    while !rb.is_empty() {
        let before = rb.position();
        let parsed = match path {
            HubWirePath::SizedServerToClient => {
                read_sized_message(&mut rb).map(|(_metadata, envelope)| envelope)
            }
            HubWirePath::CorrelatedClientToServer => {
                read_correlated_message(&mut rb).map(|(_metadata, envelope)| envelope)
            }
        };

        match parsed {
            Ok(envelope) => envelopes.push(summarize_envelope(envelope)),
            Err(MarshalerError::EmptyEnvelope) => empty_envelopes += 1,
            Err(err) => {
                return Err(format!(
                    "{path} parse at byte {} of {}: {err}",
                    before,
                    data.len()
                ));
            }
        }

        if rb.position() <= before {
            return Err(format!(
                "{path} parser did not advance at byte {} of {}",
                before,
                data.len()
            ));
        }
    }

    if envelopes.is_empty() && empty_envelopes == 0 {
        return Err(format!("{path} parsed no envelopes"));
    }

    Ok(ParsedHubStream {
        path,
        envelopes,
        empty_envelopes,
    })
}

fn summarize_envelope<'a>(envelope: MessageEnvelopeView<'a>) -> HubEnvelopeSummary<'a> {
    let type_index = envelope.type_id.resolved_type_index();
    let name = type_index
        .and_then(network_schema::name_for_type_index)
        .unwrap_or("<unknown-message>")
        .to_owned();

    HubEnvelopeSummary {
        type_index,
        type_id: format!("{:?}", envelope.type_id),
        name,
        body: envelope.body,
        body_len: envelope.body.len(),
        body_prefix: hex_prefix(envelope.body, 32),
    }
}

fn is_replicated_state_bundle_envelope(envelope: &HubEnvelopeSummary<'_>) -> bool {
    envelope.type_index == Some(REPLICATED_STATE_BUNDLE_TYPE_INDEX)
        || envelope.name == REPLICATED_STATE_BUNDLE_TYPE_NAME
}

fn fragment_type_name(type_info: FragmentTypeInfo) -> (Option<u32>, String) {
    let registration = type_info.registration().ok();
    let type_index = registration
        .map(|registration| (registration.type_index)())
        .or_else(|| type_info.type_index());
    let name = registration
        .map(|registration| (registration.name)())
        .or_else(|| {
            type_info
                .type_index()
                .and_then(network_schema::name_for_type_index)
        })
        .map(str::to_owned)
        .or_else(|| type_info.raw_uuid().map(|uuid| uuid.to_string()))
        .unwrap_or_else(|| "<unknown-state-fragment>".to_owned());
    (type_index, name)
}

fn nearby_fragment_headers(bundle_buffer: &[u8], center: usize) -> String {
    const SCAN_BEFORE: usize = 8;
    const SCAN_AFTER: usize = 24;
    const MAX_CANDIDATES: usize = 16;

    let start = center.saturating_sub(SCAN_BEFORE);
    let end = bundle_buffer
        .len()
        .min(center.saturating_add(SCAN_AFTER + 1));
    let mut candidates = Vec::new();

    for offset in start..end {
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bundle_buffer[offset..]);
        let Ok(header) = read_state_fragment_header(&mut rb) else {
            continue;
        };

        let registered = header.type_info.registration().is_ok();
        let type_index = header.type_info.type_index();
        let global_name = type_index.and_then(network_schema::name_for_type_index);
        let is_known = registered || global_name.is_some();
        if !is_known && offset != center {
            continue;
        }

        let name = global_name
            .or_else(|| {
                header
                    .type_info
                    .registration()
                    .ok()
                    .map(|registration| (registration.name)())
            })
            .map(str::to_owned)
            .unwrap_or_else(|| match header.type_info {
                FragmentTypeInfo::TypeIndex(_) => "<unknown-type-index>".to_owned(),
                FragmentTypeInfo::RawUuid(uuid) => uuid.to_string(),
            });
        let rel = offset as isize - center as isize;
        candidates.push(format!(
            "rel={rel:+} offset={offset} key={} {} header_len={} registered={} name={}",
            header.fragment_key.get(),
            display_fragment_type_info(header.type_info),
            header.header_end.saturating_sub(header.start),
            registered,
            name
        ));

        if candidates.len() == MAX_CANDIDATES {
            break;
        }
    }

    if candidates.is_empty() {
        "-".to_owned()
    } else {
        candidates.join("|")
    }
}

fn next_registered_fragment_headers(bundle_buffer: &[u8], center: usize) -> String {
    const SCAN_AFTER: usize = 4096;
    const MAX_CANDIDATES: usize = 8;

    let end = bundle_buffer
        .len()
        .min(center.saturating_add(SCAN_AFTER + 1));
    let mut candidates = Vec::new();

    for offset in center.saturating_add(1)..end {
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bundle_buffer[offset..]);
        let Ok(header) = read_state_fragment_header(&mut rb) else {
            continue;
        };
        let Ok(registration) = header.type_info.registration() else {
            continue;
        };

        let rel = offset as isize - center as isize;
        candidates.push(format!(
            "rel=+{rel} offset={offset} key={} {} header_len={} name={}",
            header.fragment_key.get(),
            display_fragment_type_info(header.type_info),
            header.header_end.saturating_sub(header.start),
            (registration.name)()
        ));

        if candidates.len() == MAX_CANDIDATES {
            break;
        }
    }

    if candidates.is_empty() {
        "-".to_owned()
    } else {
        candidates.join("|")
    }
}

fn resync_fragment_candidates(
    bundle_buffer: &[u8],
    center: usize,
    remaining_fragments: usize,
) -> String {
    const SCAN_AFTER: usize = 4096;
    const MAX_CANDIDATES: usize = 8;
    const MIN_DECODED: usize = 2;

    let end = bundle_buffer
        .len()
        .min(center.saturating_add(SCAN_AFTER + 1));
    let mut candidates = Vec::new();

    for offset in center.saturating_add(1)..end {
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bundle_buffer[offset..]);
        let mut decoded = 0usize;
        let mut first_name = None;
        let mut last_name = None;

        for _ in 0..remaining_fragments {
            let Ok(header) = read_state_fragment_header(&mut rb) else {
                break;
            };
            let Ok(registration) = header.type_info.registration() else {
                break;
            };
            if header.type_info.consume_contents(&mut rb).is_err() {
                break;
            }

            let name = (registration.name)();
            first_name.get_or_insert(name);
            last_name = Some(name);
            decoded += 1;
        }

        if decoded < MIN_DECODED {
            continue;
        }

        candidates.push((
            decoded,
            offset,
            rb.position(),
            first_name.unwrap_or("<unknown-state-fragment>"),
            last_name.unwrap_or("<unknown-state-fragment>"),
        ));
    }

    candidates.sort_by(|lhs, rhs| rhs.0.cmp(&lhs.0).then_with(|| lhs.1.cmp(&rhs.1)));
    candidates.truncate(MAX_CANDIDATES);

    if candidates.is_empty() {
        return "-".to_owned();
    }

    candidates
        .into_iter()
        .map(|(decoded, offset, end_position, first_name, last_name)| {
            let rel = offset as isize - center as isize;
            format!(
                "rel=+{rel} offset={offset} decoded={decoded} end={} first={} last={}",
                offset + end_position,
                first_name,
                last_name
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn hidden_fragment_candidates(
    bundle_buffer: &[u8],
    decoded_fragments: &[DecodedStateFragmentSpan],
    expected_fragment_count: usize,
    record_end: usize,
) -> String {
    const MAX_CANDIDATES: usize = 16;

    if !env_flag("NW_GRIDMATE_REPLAY_BOUNDARY_DIAGNOSTICS") {
        return "-".to_owned();
    }
    if record_end > bundle_buffer.len() {
        return "-".to_owned();
    }

    let mut candidates = Vec::new();

    for span in decoded_fragments.iter().rev() {
        let body_len = span.body_end.saturating_sub(span.body_start);
        if body_len < 2 {
            continue;
        }
        let expected_remaining = expected_fragment_count.saturating_sub(span.ordinal + 1);
        if expected_remaining == 0 {
            continue;
        }

        for offset in span.body_start + 1..span.body_end {
            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bundle_buffer[offset..record_end]);
            let Ok(header) = read_state_fragment_header(&mut rb) else {
                continue;
            };
            let Ok(registration) = header.type_info.registration() else {
                continue;
            };

            let first_name = (registration.name)();
            let mut decoded = 0usize;
            let mut last_name = first_name;

            if header.type_info.consume_contents(&mut rb).is_err() {
                continue;
            }
            decoded += 1;

            while decoded < expected_remaining {
                let Ok(next_header) = read_state_fragment_header(&mut rb) else {
                    break;
                };
                let Ok(next_registration) = next_header.type_info.registration() else {
                    break;
                };
                if next_header.type_info.consume_contents(&mut rb).is_err() {
                    break;
                }
                decoded += 1;
                last_name = (next_registration.name)();
            }

            if decoded != expected_remaining || !rb.is_empty() {
                continue;
            }

            candidates.push(format!(
                "owner_ord={} owner_key={} owner_type={} owner_name={} owner_header={}-{} owner_body={}-{} split_offset={} split_rel={} decoded_remaining={} hidden_key={} hidden_type={} hidden_name={} last_name={} record_end={}",
                span.ordinal,
                span.fragment_key,
                display_fragment_type_info(span.type_info),
                span.name,
                span.header_start,
                span.header_end,
                span.body_start,
                span.body_end,
                offset,
                offset - span.body_start,
                decoded,
                header.fragment_key.get(),
                display_fragment_type_info(header.type_info),
                first_name,
                last_name,
                record_end
            ));

            if candidates.len() == MAX_CANDIDATES {
                return candidates.join("|");
            }
        }
    }

    if candidates.is_empty() {
        "-".to_owned()
    } else {
        candidates.join("|")
    }
}

fn write_summary_markdown(
    out: &mut File,
    ledger: &Path,
    stats: &ReplayStats,
    report: &ReplayReport,
) -> io::Result<()> {
    writeln!(out, "# Plaintext Ledger Replay")?;
    writeln!(out)?;
    writeln!(out, "- ledger: `{}`", ledger.display())?;
    writeln!(out, "- report: `{}`", report.ledger_name)?;
    if let Some(detail_path) = &report.detail_path {
        writeln!(out, "- detail log: `{}`", detail_path.display())?;
        writeln!(
            out,
            "- detail lines: {}{}",
            report.detail_lines,
            if report.detail_truncated {
                " (truncated)"
            } else {
                ""
            }
        )?;
    }
    writeln!(out)?;

    writeln!(out, "## Carrier")?;
    writeln!(out)?;
    writeln!(out, "- records: {}", stats.records)?;
    writeln!(out, "- datagrams: {}", stats.datagrams)?;
    writeln!(
        out,
        "- reassembled carrier messages: {}",
        stats.carrier_messages
    )?;
    writeln!(out)?;
    writeln!(out, "| channel | messages |")?;
    writeln!(out, "|---:|---:|")?;
    for (channel, count) in &stats.channels {
        writeln!(out, "| {channel} | {count} |")?;
    }
    writeln!(out)?;

    writeln!(out, "## Hub Messages")?;
    writeln!(out)?;
    writeln!(out, "- parsed envelopes: {}", stats.hub_messages)?;
    writeln!(out, "- empty envelopes: {}", stats.hub_empty_envelopes)?;
    writeln!(
        out,
        "- ambiguous path selections: {}",
        stats.hub_ambiguous_flows
    )?;
    writeln!(out, "- parse errors: {}", stats.hub_parse_errors)?;
    writeln!(out)?;
    write_type_table(out, &stats.hub_types, false)?;
    write_error_table(out, "Hub parse errors", &stats.hub_errors)?;

    writeln!(out, "## Replicated State Bundles")?;
    writeln!(out)?;
    writeln!(out, "- channel-1 payloads: {}", stats.state_lane_payloads)?;
    writeln!(out, "- state bundle envelopes: {}", stats.state_bundles)?;
    writeln!(
        out,
        "- decoded bundle bodies: {}",
        stats.state_wrapped_bundles
    )?;
    writeln!(out, "- bundle bytes: {}", stats.state_bundle_bytes)?;
    writeln!(
        out,
        "- bundles with replication control: {}",
        stats.state_bundles_with_replication_control
    )?;
    writeln!(
        out,
        "- bundle parse errors: {}",
        stats.state_bundle_parse_errors
    )?;
    writeln!(out, "- fragments: {}", stats.state_fragments)?;
    writeln!(
        out,
        "- fragment iterator errors: {}",
        stats.state_fragment_iter_errors
    )?;
    writeln!(
        out,
        "- fragment decode errors: {}",
        stats.state_fragment_decode_errors
    )?;
    writeln!(out)?;
    write_type_table(out, &stats.state_types, true)?;
    write_error_table(out, "State bundle parse errors", &stats.state_bundle_errors)?;
    write_error_table(out, "State fragment errors", &stats.state_fragment_errors)?;

    Ok(())
}

fn write_type_table(
    out: &mut File,
    types: &BTreeMap<TypeKey, TrafficStats>,
    include_decode: bool,
) -> io::Result<()> {
    if include_decode {
        writeln!(
            out,
            "| path | type_index | name | count | bytes | decoded | errors |"
        )?;
        writeln!(out, "|---|---:|---|---:|---:|---:|---:|")?;
    } else {
        writeln!(out, "| path | type_index | name | count | bytes |")?;
        writeln!(out, "|---|---:|---|---:|---:|")?;
    }

    let mut rows = types.iter().collect::<Vec<_>>();
    rows.sort_by(|(left_key, left), (right_key, right)| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left_key.cmp(right_key))
    });

    for (key, value) in rows {
        if include_decode {
            writeln!(
                out,
                "| {} | {} | `{}` | {} | {} | {} | {} |",
                key.path,
                display_optional_u32(key.type_index),
                key.name,
                value.count,
                value.bytes,
                value.decoded,
                value.errors
            )?;
        } else {
            writeln!(
                out,
                "| {} | {} | `{}` | {} | {} |",
                key.path,
                display_optional_u32(key.type_index),
                key.name,
                value.count,
                value.bytes
            )?;
        }
    }
    writeln!(out)?;
    Ok(())
}

fn write_error_table(
    out: &mut File,
    title: &str,
    errors: &BTreeMap<String, usize>,
) -> io::Result<()> {
    if errors.is_empty() {
        return Ok(());
    }

    writeln!(out, "### {title}")?;
    writeln!(out)?;
    writeln!(out, "| count | error |")?;
    writeln!(out, "|---:|---|")?;
    let mut rows = errors.iter().collect::<Vec<_>>();
    rows.sort_by(|(left_error, left_count), (right_error, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_error.cmp(right_error))
    });
    for (error, count) in rows {
        writeln!(out, "| {count} | `{}` |", markdown_escape(error))?;
    }
    writeln!(out)?;
    Ok(())
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| format!("missing u32 at offset {offset}"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("slice length")))
}

fn ledger_inputs() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for var in ["NW_GRIDMATE_REPLAY_LEDGER", "NW_GRIDMATE_REPLAY_LEDGER_DIR"] {
        if let Ok(value) = env::var(var) {
            roots.extend(env::split_paths(&value));
        }
    }

    let mut ledgers = Vec::new();
    for root in roots {
        collect_ledgers(&root, &mut ledgers)
            .unwrap_or_else(|err| panic!("collect ledgers from {}: {err}", root.display()));
    }
    ledgers.sort();
    ledgers.dedup();
    ledgers
}

fn collect_ledgers(path: &Path, ledgers: &mut Vec<PathBuf>) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        ledgers.push(path.to_path_buf());
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_ledgers(&child, ledgers)?;
        } else if is_ledger_file(&child) {
            ledgers.push(child);
        }
    }

    Ok(())
}

fn is_ledger_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "ledger.bin")
        || path.extension().is_some_and(|ext| ext == "nwdl")
}

fn report_name_for_ledger(path: &Path) -> String {
    let raw = if path.file_name().is_some_and(|name| name == "ledger.bin") {
        path.parent()
            .and_then(Path::file_name)
            .unwrap_or_else(|| path.file_name().expect("ledger file name"))
    } else {
        path.file_stem()
            .or_else(|| path.file_name())
            .expect("ledger path has a name")
    };
    sanitize_file_name(&raw.to_string_lossy())
}

fn sanitize_file_name(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "ledger".to_owned()
    } else {
        out
    }
}

fn merge_counts<K: Ord>(target: &mut BTreeMap<K, usize>, source: BTreeMap<K, usize>) {
    for (key, value) in source {
        *target.entry(key).or_default() += value;
    }
}

fn merge_type_stats(
    target: &mut BTreeMap<TypeKey, TrafficStats>,
    source: BTreeMap<TypeKey, TrafficStats>,
) {
    for (key, value) in source {
        let target = target.entry(key).or_default();
        target.count += value.count;
        target.bytes += value.bytes;
        target.decoded += value.decoded;
        target.errors += value.errors;
    }
}

fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn env_limit(name: &str, default: usize) -> usize {
    match env::var(name) {
        Ok(value) if value.eq_ignore_ascii_case("all") => usize::MAX,
        Ok(value) => value.parse().unwrap_or(default),
        Err(_) => default,
    }
}

fn display_optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_owned())
}

fn display_fragment_type_info(type_info: FragmentTypeInfo) -> String {
    match type_info {
        FragmentTypeInfo::TypeIndex(type_index) => format!("type_index:{type_index}"),
        FragmentTypeInfo::RawUuid(uuid) => format!("uuid:{uuid}"),
    }
}

fn hex_prefix(bytes: &[u8], max_len: usize) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let len = bytes.len().min(max_len);
    let mut out = String::with_capacity(len * 2 + 3);
    for &byte in &bytes[..len] {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    if bytes.len() > max_len {
        out.push_str("...");
    }
    out
}

fn hex_range(bytes: &[u8], start: usize, end: usize) -> String {
    if start >= end || start >= bytes.len() {
        return String::new();
    }
    hex_prefix(&bytes[start..end.min(bytes.len())], usize::MAX)
}

fn hex_window(bytes: &[u8], center: usize, before: usize, total_len: usize) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    let start = center.saturating_sub(before);
    let end = start.saturating_add(total_len).min(bytes.len());
    format!(
        "start={start} bytes={}",
        hex_prefix(&bytes[start..end], usize::MAX)
    )
}

fn compact_debug(value: impl fmt::Debug, char_limit: usize) -> String {
    let mut value = format!("{value:?}");
    value = one_line(value);
    if value.chars().count() > char_limit {
        let mut truncated = value.chars().take(char_limit).collect::<String>();
        truncated.push_str("...");
        truncated
    } else {
        value
    }
}

fn one_line(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .replace(['\r', '\n', '\t'], " ")
        .trim()
        .to_owned()
}

fn markdown_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('`', "\\`")
}
