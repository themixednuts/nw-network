//! Replay plaintext carrier ledgers into aggregate traffic statistics.
//!
//! A ledger is a sequential capture of carrier datagrams. Each record begins
//! with a 28-byte little-endian header:
//!
//! ```text
//! bytes 0..4    magic "NWDL"
//! bytes 4..12   opaque/reserved
//! bytes 12..16  direction
//! bytes 16..20  stream
//! bytes 20..24  lane
//! bytes 24..28  payload length
//! bytes 28..    carrier datagram payload
//! ```
//!
//! Replay feeds each payload into a carrier receiver keyed by
//! `(direction, stream, lane)`, drains reassembled carrier messages, parses
//! Hub envelopes from game-data lanes, and decodes replicated-state bundle
//! fragments from replicated-state lanes. The returned [`ReplaySummary`]
//! reports traffic totals, Hub message type traffic, replicated-state fragment
//! type traffic, and recoverable parse errors observed inside individual
//! messages.
//!
//! Direction `0` uses server-to-client sized Hub messages. Direction `1` uses
//! client-to-server correlated Hub messages. Unknown directions are parsed both
//! ways and the path that yields more envelopes is selected.

use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::Path,
};

use bytes::Bytes;
use nw_network::{
    FragmentTypeInfo, ReplicatedStateBundleView,
    hub::{read_state_fragment_header, read_state_record_header},
};
use thiserror::Error;

use crate::{
    MarshalerError, ReadBuffer,
    carrier::{DecodeError, MessageData, Receiver},
    message::{MessageEnvelopeView, read_correlated_message, read_sized_message},
    network_schema,
    serialize::CARRIER_ENDIAN,
};

const LEDGER_MAGIC: &[u8; 4] = b"NWDL";
const LEDGER_HEADER_LEN: usize = 28;
const CHANNEL_GAME_DATA: u8 = 0;
const CHANNEL_REPLICATED_STATE: u8 = 1;
const REPLICATED_STATE_BUNDLE_TYPE_INDEX: u32 = 8;
const REPLICATED_STATE_BUNDLE_TYPE_NAME: &str = "Amazon::Hub::ReplicatedStateBundle";

/// Aggregate statistics produced by replaying one or more ledger captures.
#[derive(Debug, Default, Clone)]
pub struct ReplaySummary {
    /// Ledger records consumed.
    pub records: usize,
    /// Carrier datagrams fed into the receive pipeline.
    pub datagrams: usize,
    /// Carrier messages drained after per-flow reassembly.
    pub carrier_messages: usize,
    /// Carrier message counts by channel id.
    pub channels: BTreeMap<u8, usize>,

    /// Parsed non-empty Hub envelopes.
    pub hub_messages: usize,
    /// Empty Hub envelopes observed while parsing streams.
    pub hub_empty_envelopes: usize,
    /// Unknown-direction flows where both Hub paths parsed successfully.
    pub hub_ambiguous_flows: usize,
    /// Hub stream parse failures. These are recoverable and do not stop replay.
    pub hub_parse_errors: usize,
    /// Hub message traffic by wire path and message type.
    pub hub_types: BTreeMap<TypeKey, TypeTraffic>,
    /// Hub stream parse error counts by error text.
    pub hub_errors: BTreeMap<String, usize>,

    /// Carrier messages observed on the replicated-state channel.
    pub state_lane_payloads: usize,
    /// Replicated-state Hub bundle envelopes observed.
    pub state_bundles: usize,
    /// Bundle bodies decoded as replicated-state wrappers.
    pub state_wrapped_bundles: usize,
    /// Total bytes in replicated-state bundle envelope bodies.
    pub state_bundle_bytes: usize,
    /// Decoded bundle wrappers that included replication-control data.
    pub state_bundles_with_replication_control: usize,
    /// Replicated-state bundle wrapper parse failures.
    pub state_bundle_parse_errors: usize,
    /// Replicated-state fragments decoded from bundle buffers.
    pub state_fragments: usize,
    /// Errors while iterating state records or fragment headers.
    pub state_fragment_iter_errors: usize,
    /// Errors while decoding registered fragment bodies.
    pub state_fragment_decode_errors: usize,
    /// Replicated-state fragment traffic by fragment type.
    pub state_types: BTreeMap<TypeKey, TypeTraffic>,
    /// Bundle wrapper parse error counts by error text.
    pub state_bundle_errors: BTreeMap<String, usize>,
    /// State record, fragment header, and fragment body error counts by error text.
    pub state_fragment_errors: BTreeMap<String, usize>,
}

impl ReplaySummary {
    /// Merge another summary into this one.
    pub fn merge(&mut self, other: Self) {
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
        merge_type_traffic(&mut self.hub_types, other.hub_types);
        merge_type_traffic(&mut self.state_types, other.state_types);
        merge_counts(&mut self.hub_errors, other.hub_errors);
        merge_counts(&mut self.state_bundle_errors, other.state_bundle_errors);
        merge_counts(&mut self.state_fragment_errors, other.state_fragment_errors);
    }

    /// Total recoverable parse errors observed inside Hub and replicated-state messages.
    #[must_use]
    pub const fn total_parse_errors(&self) -> usize {
        self.hub_parse_errors
            + self.state_bundle_parse_errors
            + self.state_fragment_iter_errors
            + self.state_fragment_decode_errors
    }

    /// Hub message types sorted by count descending, then by [`TypeKey`].
    #[must_use]
    pub fn hub_types_sorted(&self) -> Vec<(&TypeKey, &TypeTraffic)> {
        sorted_type_rows(&self.hub_types)
    }

    /// Replicated-state fragment types sorted by count descending, then by [`TypeKey`].
    #[must_use]
    pub fn state_types_sorted(&self) -> Vec<(&TypeKey, &TypeTraffic)> {
        sorted_type_rows(&self.state_types)
    }
}

/// Stable key for grouped Hub message or replicated-state fragment traffic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeKey {
    /// Decode path that produced this type row.
    pub path: &'static str,
    /// Compact type index when the wire type could be resolved to one.
    pub type_index: Option<u32>,
    /// Type or UUID name used for display.
    pub name: String,
}

/// Traffic counters for one grouped type.
#[derive(Debug, Default, Clone)]
pub struct TypeTraffic {
    /// Number of messages or fragments observed.
    pub count: usize,
    /// Total payload bytes observed for this type.
    pub bytes: usize,
    /// Number of payloads decoded into a typed value.
    pub decoded: usize,
    /// Decode errors attributed to this type.
    pub errors: usize,
}

impl TypeTraffic {
    /// Record one payload of `bytes` bytes.
    pub fn observe(&mut self, bytes: usize) {
        self.count += 1;
        self.bytes += bytes;
    }
}

/// Hard failures that prevent replay from continuing.
///
/// Hub envelope, replicated-state bundle, and state-fragment parse failures
/// inside carrier messages are recoverable and are counted in [`ReplaySummary`]
/// instead of being returned as errors.
#[derive(Debug, Error)]
pub enum ReplayError {
    /// Ledger file I/O failed.
    #[error("read ledger: {0}")]
    Io(#[from] io::Error),

    /// A record header was incomplete.
    #[error("truncated ledger record header at offset {offset}: {remaining} bytes remain")]
    TruncatedHeader { offset: usize, remaining: usize },

    /// A record did not begin with the expected magic bytes.
    #[error("bad ledger magic at offset {offset}: {actual:02x?}")]
    BadMagic { offset: usize, actual: [u8; 4] },

    /// A record payload length overflowed the input address space.
    #[error("ledger payload length overflows at offset {offset}: payload_len={payload_len}")]
    PayloadLengthOverflow { offset: usize, payload_len: usize },

    /// A record payload extends past the end of the ledger bytes.
    #[error(
        "ledger payload at offset {offset} exceeds input length: payload_len={payload_len}, available={available}"
    )]
    PayloadOutOfRange {
        offset: usize,
        payload_len: usize,
        available: usize,
    },

    /// Carrier datagram decoding failed for a structurally valid ledger record.
    #[error("carrier decode at offset {offset}: {source}")]
    CarrierDecode {
        offset: usize,
        #[source]
        source: DecodeError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FlowKey {
    direction: u32,
    stream: u32,
    lane: u32,
}

/// Replay ledger bytes and return aggregate traffic statistics.
///
/// # Errors
///
/// Returns an error when the ledger framing is malformed or a carrier datagram
/// cannot be decoded. Message-level Hub and replicated-state parse failures are
/// counted in the returned summary when replay can continue.
pub fn replay_ledger_bytes(data: &[u8]) -> Result<ReplaySummary, ReplayError> {
    let mut stats = ReplaySummary::default();
    let mut receivers: HashMap<FlowKey, Receiver> = HashMap::new();
    let mut offset = 0usize;

    while offset < data.len() {
        let header_offset = offset;
        if data.len() - offset < LEDGER_HEADER_LEN {
            return Err(ReplayError::TruncatedHeader {
                offset,
                remaining: data.len() - offset,
            });
        }

        if &data[offset..offset + 4] != LEDGER_MAGIC {
            let mut actual = [0u8; 4];
            actual.copy_from_slice(&data[offset..offset + 4]);
            return Err(ReplayError::BadMagic { offset, actual });
        }

        let direction = read_u32_le(data, offset + 12);
        let stream = read_u32_le(data, offset + 16);
        let lane = read_u32_le(data, offset + 20);
        let payload_len = read_u32_le(data, offset + 24) as usize;
        offset += LEDGER_HEADER_LEN;

        let end = offset
            .checked_add(payload_len)
            .ok_or(ReplayError::PayloadLengthOverflow {
                offset: header_offset,
                payload_len,
            })?;
        let payload = data
            .get(offset..end)
            .ok_or_else(|| ReplayError::PayloadOutOfRange {
                offset: header_offset,
                payload_len,
                available: data.len().saturating_sub(offset),
            })?;

        let key = FlowKey {
            direction,
            stream,
            lane,
        };
        let receiver = receivers.entry(key).or_default();
        let fed = receiver
            .feed(Bytes::copy_from_slice(payload))
            .map_err(|source| ReplayError::CarrierDecode {
                offset: header_offset,
                source,
            })?;

        stats.records += 1;
        stats.datagrams += 1;
        for (channel, message) in fed.into_messages() {
            stats.carrier_messages += 1;
            *stats.channels.entry(channel).or_default() += 1;
            summarize_carrier_message(direction, channel, &message, &mut stats);
        }

        offset = end;
    }

    Ok(stats)
}

/// Read and replay a ledger file.
///
/// # Errors
///
/// Returns an error when file I/O fails, ledger framing is malformed, or a
/// carrier datagram cannot be decoded. Message-level parse failures are counted
/// in the returned summary when replay can continue.
pub fn replay_ledger_file(path: &Path) -> Result<ReplaySummary, ReplayError> {
    let data = fs::read(path)?;
    replay_ledger_bytes(&data)
}

fn summarize_carrier_message(
    direction: u32,
    channel: u8,
    message: &MessageData,
    stats: &mut ReplaySummary,
) {
    match channel {
        CHANNEL_GAME_DATA => summarize_hub_message(direction, message, stats, false),
        CHANNEL_REPLICATED_STATE => {
            stats.state_lane_payloads += 1;
            summarize_hub_message(direction, message, stats, true);
        }
        _ => {}
    }
}

fn summarize_hub_message(
    direction: u32,
    message: &MessageData,
    stats: &mut ReplaySummary,
    decode_state_bundles: bool,
) {
    let selected = if let Some(path) = hub_wire_path_for_direction(direction) {
        match parse_hub_stream(message.data.as_ref(), path) {
            Ok(stream) => stream,
            Err(error) => {
                record_hub_parse_error(error, stats);
                return;
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
                if correlated.envelopes.len() > sized.envelopes.len() {
                    correlated
                } else {
                    sized
                }
            }
            (Err(sized), Err(correlated)) => {
                record_hub_parse_error(format!("sized={sized}; correlated={correlated}"), stats);
                return;
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

        if decode_state_bundles && is_replicated_state_bundle_envelope(envelope) {
            summarize_state_bundle_body(envelope, stats);
        }
    }
}

fn summarize_state_bundle_body(envelope: &HubEnvelopeSummary<'_>, stats: &mut ReplaySummary) {
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
            *stats.state_bundle_errors.entry(error).or_default() += 1;
            return;
        }
        Err(err) => {
            stats.state_bundle_parse_errors += 1;
            *stats
                .state_bundle_errors
                .entry(err.to_string())
                .or_default() += 1;
            return;
        }
    };

    stats.state_wrapped_bundles += 1;
    if view.has_replication_control() {
        stats.state_bundles_with_replication_control += 1;
    }

    summarize_state_fragments_from_buffer(view.bundle_buffer, stats);
}

fn summarize_state_fragments_from_buffer(bundle_buffer: &[u8], stats: &mut ReplaySummary) -> usize {
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, bundle_buffer);
    let mut fragment_count = 0usize;

    while !rb.is_empty() {
        let record = match read_state_record_header(&mut rb) {
            Ok(record) => record,
            Err(err) => {
                record_state_fragment_error(err, stats);
                break;
            }
        };

        for _ in 0..record.fragment_count {
            let header = match read_state_fragment_header(&mut rb) {
                Ok(header) => header,
                Err(err) => {
                    record_state_fragment_error(err, stats);
                    return fragment_count;
                }
            };

            let body_start = rb.position();
            let fragment = match header.type_info.decode_contents(&mut rb) {
                Ok(fragment) => fragment,
                Err(err) => {
                    record_state_fragment_error(err, stats);
                    return fragment_count;
                }
            };

            let body_end = rb.position();
            let Some(body) = bundle_buffer.get(body_start..body_end) else {
                record_state_fragment_error(
                    MarshalerError::TruncatedPayload {
                        declared: body_end.saturating_sub(body_start),
                        available: bundle_buffer.len().saturating_sub(body_start),
                    },
                    stats,
                );
                return fragment_count;
            };

            fragment_count += 1;
            stats.state_fragments += 1;
            let (type_index, name) = fragment_type_name(header.type_info);
            let key = TypeKey {
                path: "state",
                type_index,
                name,
            };
            let type_stats = stats.state_types.entry(key).or_default();
            type_stats.observe(body.len());
            type_stats.decoded += 1;

            let _ = fragment;
        }
    }

    fragment_count
}

fn record_state_fragment_error(err: MarshalerError, stats: &mut ReplaySummary) {
    let is_decode_error = is_fragment_decode_error(&err);
    let error = err.to_string();
    if is_decode_error {
        stats.state_fragment_decode_errors += 1;
    } else {
        stats.state_fragment_iter_errors += 1;
    }
    *stats.state_fragment_errors.entry(error).or_default() += 1;
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

impl std::fmt::Display for HubWirePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

fn record_hub_parse_error(error: String, stats: &mut ReplaySummary) {
    stats.hub_parse_errors += 1;
    *stats.hub_errors.entry(error).or_default() += 1;
}

struct ParsedHubStream<'a> {
    path: HubWirePath,
    envelopes: Vec<HubEnvelopeSummary<'a>>,
    empty_envelopes: usize,
}

struct HubEnvelopeSummary<'a> {
    type_index: Option<u32>,
    name: String,
    body: &'a [u8],
    body_len: usize,
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
        name,
        body: envelope.body,
        body_len: envelope.body.len(),
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

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&data[offset..offset + 4]);
    u32::from_le_bytes(bytes)
}

fn merge_counts<K: Ord>(target: &mut BTreeMap<K, usize>, source: BTreeMap<K, usize>) {
    for (key, value) in source {
        *target.entry(key).or_default() += value;
    }
}

fn merge_type_traffic(
    target: &mut BTreeMap<TypeKey, TypeTraffic>,
    source: BTreeMap<TypeKey, TypeTraffic>,
) {
    for (key, value) in source {
        let target = target.entry(key).or_default();
        target.count += value.count;
        target.bytes += value.bytes;
        target.decoded += value.decoded;
        target.errors += value.errors;
    }
}

fn sorted_type_rows(types: &BTreeMap<TypeKey, TypeTraffic>) -> Vec<(&TypeKey, &TypeTraffic)> {
    let mut rows = types.iter().collect::<Vec<_>>();
    rows.sort_by(|(left_key, left), (right_key, right)| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left_key.cmp(right_key))
    });
    rows
}
