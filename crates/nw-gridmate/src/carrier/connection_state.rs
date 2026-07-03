//! Per-connection carrier state.
//!
//! Following GridMate `Carrier.cpp`'s `ConnectionState` struct. This
//! module owns the steady-state per-peer protocol logic: outbound
//! chunking, retransmit queue, inbound reassembly, and the wire
//! framing for one carrier connection.
//!
//! Adjacent concerns live in sibling modules:
//!
//! - [`super::system_message`] — system message id constants.
//! - [`super::handshake_retry`] — connect-request retry timing.
//! - [`super::mtu`] — MTU + chunk-size arithmetic.
//! - [`super::datagram_history`] — ACK history (`DatagramHistoryList`)
//!   plus the ACK frame encode/decode helpers.

use super::carrier_desc::CarrierProtocolProfile;
use super::carrier_thread::{message_header_size, write_message_header};
use super::datagram::{DatagramData, DatagramHeader};
use super::datagram_history::{
    DatagramHistoryList, apply_ack_data_to_send_queue, read_ack_data_frame,
    sequence_number_sequential_distance, write_ack_history,
};
use super::handshake_retry;
use super::io::CarrierTransport;
use super::message::{DataReliability, MessageData};
use super::mtu;
use super::receiver::Receiver;
use super::system_message;
use super::types::{
    MAX_CHANNELS, PRIORITY_MAX, SEQUENCE_NUMBER_MAX, SYSTEM_CHANNEL, SequenceNumber,
};
use crate::serialize::{CARRIER_ENDIAN, ReadBuffer, WriteBuffer};
use bytes::Bytes;
use std::collections::VecDeque;
use std::mem::size_of;
use std::time::Instant;
use tracing::debug;

/// Per-connection carrier state. Generic over the transport `T`
/// so each call to `io.read()`/`io.write()` is monomorphised against
/// the concrete transport — no `dyn` dispatch and no enum-variant
/// match arms on the hot path. See [`super::io::CarrierTransport`].
pub struct ConnectionState<T: CarrierTransport> {
    /// Per-peer carrier I/O. Either a dedicated DTLS connection
    /// (initiator / client side) or a pair of channels into a
    /// shared multi-peer demux (responder / server side).
    pub io: T,

    /// Received datagrams history for ACK tracking
    pub received_datagrams_history: DatagramHistoryList,

    /// Next datagram sequence number to send (GridMate: starts at 1)
    pub datagram_seq_num: SequenceNumber,

    /// Last datagram we sent ACK for
    pub last_acked_datagram: SequenceNumber,

    /// Next message sequence number to send per channel (GridMate: Connection::m_sendSeqNum)
    /// Initialized to SequenceNumberMax (0xFFFF), first message wraps to 0
    pub send_seq_num: [SequenceNumber; MAX_CHANNELS],

    /// Next reliable message sequence number to send per channel (GridMate: Connection::m_sendReliableSeqNum)
    /// Initialized to SequenceNumberMax (0xFFFF)
    pub send_reliable_seq_num: [SequenceNumber; MAX_CHANNELS],

    /// Messages to send, organized by priority
    pub to_send: [VecDeque<MessageData>; PRIORITY_MAX],

    /// Stateful receive pipeline: pure decode +
    /// [`super::reassembler::Reassembler`] per-channel reassembly.
    /// `ConnectionState` holds one privately and layers dedup / ACK /
    /// retransmit logic around it; reads the latest delivered reliable
    /// sequence numbers via
    /// [`Reassembler::received_reliable_seq_num`] when other parts of
    /// the carrier (history, telemetry) need to peek.
    receiver: Receiver,

    /// Received messages ready for application, organized by channel (GridMate: Connection::m_toReceive)
    pub to_receive: [VecDeque<MessageData>; MAX_CHANNELS],

    /// Sent datagrams waiting for ACK (GridMate: ConnectionState::m_sendDataGrams)
    send_datagrams: VecDeque<DatagramData>,

    /// Last received datagram time (for timeout detection)
    pub last_received_datagram_time: Instant,

    /// Last time we sent ACKs (GridMate: ConnectionData::m_lastAckSend)
    last_ack_send_time: Instant,

    /// Whether we've received any datagram since the last send (response-based ACK trigger)
    received_since_last_send: bool,

    /// Connection create time
    pub create_time: Instant,

    /// Whether we're in the connecting phase (before SM_CONNECT_ACK received)
    /// All messages during this phase should have is_connecting=true
    pub is_connecting: bool,

    /// Handshake retry state (GridMate: PendingHandshake)
    /// Time for next retry
    pub handshake_retry_time: Instant,
    /// Number of retries so far (for exponential backoff)
    pub handshake_num_retries: u32,

    /// Outbound LZ4 compression gate — mirrors
    /// `Carrier::SendCompressed(bool)`. When `true`, post-handshake
    /// user datagrams attempt LZ4 compression (`0x81` header on
    /// success; `0x80` fallback when LZ4 doesn't shrink the payload).
    /// When `false`, every outbound datagram emits `0x80` and skips
    /// the compression attempt entirely.
    pub send_compressed: bool,

    /// Carrier handshake protocol version selected by the embedder.
    pub protocol_version: u32,
}

impl<T: CarrierTransport> ConnectionState<T> {
    /// Construct a fresh thread-side connection with the framework
    /// default carrier profile.
    pub fn new(io: T) -> Self {
        Self::with_protocol_profile(io, CarrierProtocolProfile::default())
    }

    /// Construct with an explicit `send_compressed` toggle. Mirrors
    /// `Carrier::SendCompressed(bool)`.
    pub fn with_send_compressed(io: T, send_compressed: bool) -> Self {
        let protocol = CarrierProtocolProfile {
            send_compressed,
            ..Default::default()
        };
        Self::with_protocol_profile(io, protocol)
    }

    /// Construct with an explicit carrier protocol profile.
    pub fn with_protocol_profile(io: T, protocol: CarrierProtocolProfile) -> Self {
        let now = Instant::now();

        Self {
            io,
            received_datagrams_history: DatagramHistoryList::new(),
            datagram_seq_num: SequenceNumber::from(1), // IMPORTANT: GridMate starts at 1, first datagram will be 2
            last_acked_datagram: SequenceNumber::ZERO,
            last_ack_send_time: now, // Initialize to now - connect_request sends first ACK
            received_since_last_send: false,
            // GridMate: m_sendSeqNum[all] = SequenceNumberMax (0xFFFF)
            send_seq_num: [SEQUENCE_NUMBER_MAX; MAX_CHANNELS],
            // GridMate: m_sendReliableSeqNum[all] = SequenceNumberMax (0xFFFF)
            send_reliable_seq_num: [SEQUENCE_NUMBER_MAX; MAX_CHANNELS],
            // One queue per `DataPriority` variant (System/High/Normal/Low).
            // Use `array::from_fn` rather than a literal so the queue count
            // stays driven by `PRIORITY_MAX` and renumbering the enum
            // doesn't quietly drop variants on the floor.
            to_send: std::array::from_fn(|_| VecDeque::new()),
            receiver: Receiver::new(),
            to_receive: Default::default(), // MAX_CHANNELS = 4 (ready for app)
            send_datagrams: VecDeque::new(),
            last_received_datagram_time: now,
            create_time: now,
            is_connecting: true, // Start in connecting phase
            // Handshake retry: fire immediately for initial send
            // GridMate pattern: Initial send at T=0, first retry at T+10ms
            handshake_retry_time: now, // Fire immediately
            handshake_num_retries: 0,
            send_compressed: protocol.send_compressed,
            protocol_version: protocol.version,
        }
    }

    /// `Carrier::SendCompressed() const` — current outbound compression
    /// toggle.
    pub fn send_compressed(&self) -> bool {
        self.send_compressed
    }

    /// `Carrier::SendCompressed(bool)` — toggle outbound LZ4
    /// compression. Affects only subsequent datagrams; in-flight
    /// payloads keep whatever shape they were generated with.
    pub fn set_send_compressed(&mut self, value: bool) {
        self.send_compressed = value;
    }

    /// Selected carrier handshake protocol version.
    pub fn protocol_version(&self) -> u32 {
        self.protocol_version
    }

    /// Generate SM_CONNECT_REQUEST payload for the current retry count
    /// Format: [version (4 bytes)][padding 0x01 * retry_count][msg_id 0x01]
    pub fn make_connect_request_payload(&self) -> Bytes {
        use bytes::BytesMut;

        let version_bytes = self.protocol_version.to_be_bytes();
        let data_len = 5 + self.handshake_num_retries as usize;
        let mut msg = BytesMut::with_capacity(data_len);

        msg.extend_from_slice(&version_bytes); // 4 bytes: version
        // Add padding bytes (0x01 repeated retry_count times)
        for _ in 0..self.handshake_num_retries {
            msg.extend_from_slice(&[0x01]);
        }
        msg.extend_from_slice(&[system_message::SM_CONNECT_REQUEST]); // msg_id

        msg.freeze()
    }

    /// Queue SM_CONNECT_REQUEST message for sending
    pub fn queue_connect_request(&mut self) {
        let payload = self.make_connect_request_payload();
        let msg = MessageData {
            channel: SYSTEM_CHANNEL,
            reliability: DataReliability::Unreliable,
            is_connecting: true,
            data: payload,
            ..Default::default()
        };
        // Queue at priority 0 (highest)
        self.to_send[0].push_back(msg);
    }

    /// Queue SM_CONNECT_ACK reply for the responder side.
    /// Body is `[version u32 BE][SM_CONNECT_ACK]`, mirroring
    /// `make_connect_request_payload`'s shape — `msg_id` lives at
    /// the last byte by GridMate convention.
    ///
    /// Lumberyard `Carrier.cpp:4420` explicitly sends this as
    /// `SEND_RELIABLE` — the matching `SM_CONNECT_REQUEST` is unreliable
    /// (line 3742) because the initiator retries on a timer until it
    /// sees the ACK, but the responder's ACK has to be reliable so the
    /// connection state can advance once the initiator gets it. If we
    /// send it Unreliable the launcher receives our flags=0xa0 (MF_CONNECTING |
    /// MF_DATA_CHANNEL with NO MF_RELIABLE=0x01 bit), and apparently
    /// silently refuses to progress past the handshake — observed
    /// 2026-05-15: launcher sends dgramSeq=2,3 then goes idle for the
    /// full state-10 timeout (~4 min) without ever sending Cmd_Greetings
    /// or RegistrationRequestV3, even though its in-process WriteBuffer
    /// shows the message marshaled.
    pub fn queue_connect_ack(&mut self) {
        let mut payload = Vec::with_capacity(5);
        payload.extend_from_slice(&self.protocol_version.to_be_bytes());
        payload.push(system_message::SM_CONNECT_ACK);
        let msg = MessageData {
            channel: SYSTEM_CHANNEL,
            reliability: DataReliability::Unreliable,
            is_connecting: true,
            data: bytes::Bytes::from(payload),
            ..Default::default()
        };
        self.to_send[0].push_back(msg);
    }

    /// Schedule next handshake retry with exponential backoff
    pub fn schedule_next_retry(&mut self) {
        self.handshake_num_retries += 1;
        let interval = handshake_retry::retry_interval(self.handshake_num_retries);
        self.handshake_retry_time = Instant::now() + interval;
    }

    /// Get duration until next handshake retry (None if not connecting)
    pub fn time_until_retry(&self) -> Option<std::time::Duration> {
        if !self.is_connecting {
            return None;
        }
        let now = Instant::now();
        if now >= self.handshake_retry_time {
            Some(std::time::Duration::ZERO)
        } else {
            Some(self.handshake_retry_time - now)
        }
    }

    /// Check if it's time to retry the handshake
    pub fn should_retry_handshake(&self) -> bool {
        self.is_connecting && Instant::now() >= self.handshake_retry_time
    }

    /// Process incoming datagram (GridMate: ProcessIncomingDataGram)
    /// This is called after DTLS decryption with the plaintext datagram
    ///
    /// Accepts `Bytes` to enable zero-copy slicing for uncompressed payloads.
    /// When uncompressed, the payload can be sliced directly from the input Bytes
    /// without copying, using `Bytes::slice()` which is zero-copy.
    pub fn process_incoming_datagram(&mut self, data: Bytes) -> Result<(), String> {
        // Log received datagram for debugging
        let preview: Vec<u8> = data.iter().take(16).copied().collect();
        debug!(
            "Received datagram: {} bytes, first 16: {:02x?}",
            data.len(),
            preview
        );

        // We received data - flag for response-based ACK. Done up
        // front so the `received_since_last_send` flag flips even
        // when decode or dedup later rejects the datagram (the peer
        // still acked a UDP packet our way, the carrier still wants
        // to acknowledge it).
        self.received_since_last_send = true;

        // Hand the raw bytes to the pure decoder + per-channel
        // reassembler. Lifecycle work (dedup, ACK history, time-since)
        // happens around it; envelope parsing happens above it.
        let fed = self
            .receiver
            .feed(data)
            .map_err(|e| format!("decode datagram: {e}"))?;

        let header = fed.header;
        debug!(
            "Datagram header: seq={}, compressed={}",
            header.sequence_number.get(),
            header.is_compressed
        );

        // GridMate: Check if history full and need to ACK
        if self.received_datagrams_history.is_full() {
            let first_id = self
                .received_datagrams_history
                .at(self.received_datagrams_history.begin());
            if self.last_acked_datagram < first_id {
                // TODO: WriteAckData and SendSystemMessage
            }
        }

        // GridMate: Insert into history (duplicate detection)
        if !self
            .received_datagrams_history
            .insert(header.sequence_number)
        {
            return Err("Duplicate datagram".to_string());
        }

        self.last_received_datagram_time = Instant::now();

        // Collect every message the reassembler made deliverable.
        // The `FedDatagram` borrow is on `&mut self.receiver`, so we
        // can't call `self.process_system_message(...)` (which needs
        // `&mut self`) while the iterator is live — collect into a
        // local owned Vec and drop the borrow before reacting.
        let delivered: Vec<(u8, MessageData)> = fed.into_messages().collect();
        for (channel, msg) in delivered {
            if channel == SYSTEM_CHANNEL {
                // System frames get carrier-layer handling inline —
                // ACK frames update the send window, SM_CONNECT_REQUEST
                // drives the handshake. Only main-thread system
                // messages flow on into the application queue.
                let should_add_to_queue = self.process_system_message(&msg)?;
                if !should_add_to_queue {
                    continue;
                }
            } else if channel != 0 {
                tracing::trace!(
                    "[CARRIER] Non-default channel msg: ch={} rel={:?} seq={} chunks={} size={}",
                    channel,
                    msg.reliability,
                    msg.sequence_number.get(),
                    msg.num_chunks.get(),
                    msg.data.len(),
                );
            }
            self.to_receive[channel as usize].push_back(msg);
        }

        Ok(())
    }

    /// Check if there are messages to send (Rust idiom: using Iterator::any)
    pub fn has_messages_to_send(&self) -> bool {
        self.to_send.iter().any(|queue| !queue.is_empty())
    }

    /// Get iterator over all queued messages across all priorities (Rust idiom: flat_map)
    pub fn queued_messages_iter(&self) -> impl Iterator<Item = &MessageData> {
        self.to_send.iter().flat_map(|queue| queue.iter())
    }

    /// Get mutable iterator over all queued messages (Rust idiom: iter_mut with flat_map)
    pub fn queued_messages_iter_mut(&mut self) -> impl Iterator<Item = &mut MessageData> {
        self.to_send.iter_mut().flat_map(|queue| queue.iter_mut())
    }

    /// Get iterator over sent datagrams awaiting ACK (Rust idiom: expose internal iterator)
    pub fn pending_acks_iter(&self) -> impl Iterator<Item = &DatagramData> {
        self.send_datagrams.iter()
    }

    /// Count of pending ACKs (Rust idiom: len method)
    pub fn pending_acks_count(&self) -> usize {
        self.send_datagrams.len()
    }

    /// Latest delivered reliable sequence number on `channel`, as
    /// tracked by the inner [`Reassembler`]. Mirrors the old
    /// `received_reliable_seq_num[channel]` field for code that still
    /// peeks at it (carrier diagnostics, ACK history tuning).
    pub fn received_reliable_seq_num(&self, channel: usize) -> SequenceNumber {
        self.receiver
            .reassembler()
            .received_reliable_seq_num(channel as u8)
    }

    /// Process system message (GridMate: CarrierDriver system message handling)
    /// Returns true if message should be added to receive queue (main thread messages)
    /// Returns false if message was handled here (carrier thread messages)
    fn process_system_message(&mut self, msg: &MessageData) -> Result<bool, String> {
        // GridMate: size_t messageIdSize = 1; // 1 byte
        const MESSAGE_ID_SIZE: usize = size_of::<u8>();

        if msg.data.len() < MESSAGE_ID_SIZE {
            return Err("System message too small".to_string());
        }

        // GridMate: size_t messageIdOffset = msg.m_dataSize - messageIdSize
        let message_id_offset = msg.data.len() - MESSAGE_ID_SIZE;
        let msg_id = msg.data[message_id_offset];

        // GridMate: if (msgId > SM_CT_FIRST) - carrier thread messages
        if msg_id > system_message::SM_CT_FIRST {
            // Carrier thread messages (should be unreliable)
            // These are handled here and NOT forwarded to Carrier layer
            // Carrier thread messages should be unreliable
            let _ = msg.reliability != DataReliability::Unreliable;

            match msg_id {
                system_message::SM_CT_ACKS => {
                    // GridMate: ReadAckData(connection, readBuffer)
                    let payload = &msg.data[..message_id_offset];
                    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, payload);
                    self.read_ack_data(&mut rb);
                }
                system_message::SM_CT_CONN_CONTROL => {}
                _ => {}
            }
            // Carrier thread message - handled, don't add to queue
            Ok(false)
        } else {
            // Main thread messages (SM_CONNECT_REQUEST, SM_CONNECT_ACK, etc.)
            // These need to be forwarded to Carrier layer via receive queue
            match msg_id {
                system_message::SM_CONNECT_REQUEST => {}
                system_message::SM_CONNECT_ACK => {
                    // Forward to Carrier layer for validation
                }
                system_message::SM_DISCONNECT => {}
                system_message::SM_CLOCK_SYNC => {}
                _ => {}
            }

            // Main thread message - add to queue for Carrier layer
            Ok(true)
        }
    }

    /// Read ACK data (GridMate: CarrierDriver::ReadAckData)
    fn read_ack_data(&mut self, read_buffer: &mut ReadBuffer) {
        let Some(ack_data) = read_ack_data_frame(read_buffer) else {
            return;
        };

        let nacked = apply_ack_data_to_send_queue(&mut self.send_datagrams, &ack_data);
        for seq in nacked {
            self.nack_datagram(seq);
        }
    }

    /// Write ACK data (GridMate: CarrierDriver::WriteAckData)
    fn write_ack_data(&mut self, write_buffer: &mut WriteBuffer) {
        if let Some(last_to_ack) =
            write_ack_history(&mut self.received_datagrams_history, write_buffer)
        {
            self.last_acked_datagram = last_to_ack;
        }
    }

    /// Initialize outgoing datagram (GridMate: CarrierDriver::InitOutgoingDatagram)
    /// Prepare outgoing datagram (combines init + generate)
    /// Handles ACKs if needed, then generates datagram from queued messages
    /// Returns None if there's nothing to send (no ACKs needed, no messages queued)
    /// Returns Some(Bytes) with the datagram data if there's something to send
    pub fn prepare_outgoing_datagram(
        &mut self,
        max_datagram_size: usize,
    ) -> Result<Option<Bytes>, crate::GridMateError> {
        // GridMate: if (m_trafficControl->IsSendAck(connection))
        // IsSendACKOnly: fires when (a) received data since last send, OR
        // (b) 100ms elapsed — keepalive ceiling (m_lostPacketTimeoutMS / 10)
        let now = Instant::now();
        let should_send_ack = self.received_since_last_send
            || now.duration_since(self.last_ack_send_time) >= std::time::Duration::from_millis(100);

        if should_send_ack {
            let mut ack_buffer = WriteBuffer::new(CARRIER_ENDIAN);
            self.write_ack_data(&mut ack_buffer);
            let mut payload = ack_buffer.into_vec();
            payload.push(system_message::SM_CT_ACKS);

            // Create SM_CT_ACKS message
            let mut ack_msg = MessageData::new();
            ack_msg.channel = SYSTEM_CHANNEL;
            ack_msg.reliability = DataReliability::Unreliable; // Carrier thread messages are unreliable
            ack_msg.is_connecting = self.is_connecting; // Use connection's connecting state
            ack_msg.data = bytes::Bytes::from(payload);

            // Queue the ACK message
            self.queue_message(ack_msg, 0); // PRIORITY_SYSTEM = 0

            // OnSendAck - update last send time and clear response flag
            self.last_ack_send_time = now;
            self.received_since_last_send = false;
        }

        // Check if there are messages to send (ACKs queued above, or regular messages)
        if !self.has_messages_to_send() {
            return Ok(None);
        }

        // Generate the datagram (GridMate: GenerateOutgoingDataGram)
        // GridMate: Check min size
        if max_datagram_size <= DatagramHeader::SIZE {
            return Err(crate::GridMateError::InvalidState(format!(
                "Max datagram size too small: {} <= {}",
                max_datagram_size,
                DatagramHeader::SIZE
            )));
        }

        // GridMate: dgram.m_flowControl.m_sequenceNumber = connection->m_dataGramSeqNum + 1
        let datagram_seq = self.datagram_seq_num.next();

        // GridMate: Create DatagramData for tracking sent datagram
        let mut dgram_data = DatagramData::new(datagram_seq);

        // Create buffer for message payload (will be compressed)
        let mut payload_buffer = WriteBuffer::with_capacity(CARRIER_ENDIAN, max_datagram_size);

        // Determine if we have user messages (channel != SYSTEM_CHANNEL)
        // GridMate uses 0x81 for datagrams with user messages (channel 0), 0x80 for system-only
        let has_user_messages = self
            .to_send
            .iter()
            .any(|q| q.iter().any(|msg| msg.channel != SYSTEM_CHANNEL));

        // Track context for sequential ID optimization (GridMate: OutgoingDataGramContext)
        let mut current_channel: u8 = 0;
        let mut is_written_first_seq_num = [false; MAX_CHANNELS];
        let mut is_written_first_rel_seq_num = [false; MAX_CHANNELS];
        let mut last_sequence_number = [SequenceNumber::ZERO; MAX_CHANNELS];
        let mut last_seq_reliable_number = [SequenceNumber::ZERO; MAX_CHANNELS];

        // GridMate: Iterate through priorities (highest first)
        for priority in 0..PRIORITY_MAX {
            // Pick messages from queue
            while let Some(msg) = self.to_send[priority].front() {
                // GridMate: Check if channel changed
                let is_write_channel = msg.channel != current_channel;
                if is_write_channel {
                    current_channel = msg.channel;
                }

                // GridMate: Check if messages are sequential
                let is_write_message_seq_id = if is_written_first_seq_num[current_channel as usize]
                {
                    sequence_number_sequential_distance(
                        last_sequence_number[current_channel as usize],
                        msg.sequence_number,
                    ) != 1
                } else {
                    is_written_first_seq_num[current_channel as usize] = true;
                    true // Always write first
                };

                // GridMate: Check if rel_seq needs to be written
                // Real client always writes rel_seq for every message (flags 0x88 not 0x98)
                // This keeps MF_SEQUENTIAL_REL_ID clear, matching observed client behavior
                let is_write_reliable_seq_id =
                    if is_written_first_rel_seq_num[current_channel as usize] {
                        if msg.reliability == DataReliability::Reliable {
                            sequence_number_sequential_distance(
                                last_seq_reliable_number[current_channel as usize],
                                msg.send_reliable_seq_num,
                            ) != 1
                        } else {
                            // Always write rel_seq for unreliable messages (matches real client)
                            true
                        }
                    } else {
                        is_written_first_rel_seq_num[current_channel as usize] = true;
                        true // Always write first
                    };

                // GridMate: Check if message fits
                let header_size = message_header_size(
                    msg,
                    is_write_message_seq_id,
                    is_write_reliable_seq_id,
                    is_write_channel,
                );

                if (msg.data.len() + header_size)
                    > (max_datagram_size - DatagramHeader::SIZE - payload_buffer.len())
                {
                    break; // Can't fit this message
                }

                // Pop the message and write it
                let msg = self.to_send[priority].pop_front().unwrap();

                // Write message header
                write_message_header(
                    &mut payload_buffer,
                    &msg,
                    is_write_message_seq_id,
                    is_write_reliable_seq_id,
                    is_write_channel,
                );

                // Write message data
                payload_buffer.write_bytes(&msg.data);

                // Update context
                last_sequence_number[current_channel as usize] = msg.sequence_number;
                last_seq_reliable_number[current_channel as usize] = msg.send_reliable_seq_num;

                // GridMate: Track reliable messages for retransmission
                if msg.reliability == DataReliability::Reliable {
                    dgram_data.resend_data_size += msg.data.len() as u16;
                    // Clone message data without ack_callback (not needed for resend)
                    // Note: msg.data.clone() is zero-copy - Bytes uses Arc internally, so
                    // this only increments the reference counter, not copying the actual data.
                    // The field-by-field copy is necessary for resend tracking, but the payload
                    // itself is zero-copy.
                    let mut msg_copy = MessageData::new();
                    msg_copy.reliability = msg.reliability;
                    msg_copy.channel = msg.channel;
                    msg_copy.num_chunks = msg.num_chunks;
                    msg_copy.sequence_number = msg.sequence_number;
                    msg_copy.send_reliable_seq_num = msg.send_reliable_seq_num;
                    msg_copy.is_connecting = msg.is_connecting;
                    msg_copy.data = msg.data.clone(); // Zero-copy: Arc reference increment
                    dgram_data.to_resend[priority].push(msg_copy);
                }
            }
        }

        // Check if we only have empty payload (no messages fit)
        if payload_buffer.is_empty() {
            // Don't send empty datagrams - messages are still queued but don't fit
            tracing::trace!(
                "[THREAD_CONN] No messages fit in datagram (queued messages may exceed MTU)"
            );
            return Ok(None);
        }

        // Build the final datagram with header and (optionally compressed) payload
        let payload_data = payload_buffer.into_vec();

        // GridMate: Compression is ONLY used when:
        // 1. The peer has compression enabled (`Carrier::SendCompressed(true)`)
        // 2. Connection is established (not during handshake) — CST_CONNECTING rejects compressed packets
        // 3. There are user messages (not system-only datagrams like ACKs)
        let compression_flag = if self.send_compressed && has_user_messages && !self.is_connecting {
            DatagramHeader::COMPRESSED // 0x81 for user data after connected
        } else {
            DatagramHeader::UNCOMPRESSED // 0x80 for handshake, system-only, or compression disabled
        };

        let datagram = if compression_flag == DatagramHeader::COMPRESSED {
            // Compress payload with LZ4 (raw format, no size header)
            // GridMate uses raw LZ4 - the uncompressed size is NOT prepended
            let mut compressed = Vec::new();
            lzzzz::lz4::compress_to_vec(
                &payload_data,
                &mut compressed,
                lzzzz::lz4::ACC_LEVEL_DEFAULT,
            )
            .expect("LZ4 compression failed");

            // GridMate: Only use compressed if beneficial (smaller than uncompressed)
            if compressed.len() >= payload_data.len() {
                // Fall through to uncompressed path
                let mut final_buffer =
                    Vec::with_capacity(DatagramHeader::SIZE + payload_data.len());
                final_buffer.push(DatagramHeader::UNCOMPRESSED);
                final_buffer.push(DatagramHeader::HAS_COMPRESSOR);
                final_buffer.extend_from_slice(&datagram_seq.get().to_be_bytes());
                final_buffer.extend_from_slice(&payload_data);
                Bytes::from(final_buffer)
            } else {
                // Build datagram: header + compressed payload
                let mut final_buffer = Vec::with_capacity(DatagramHeader::SIZE + compressed.len());
                final_buffer.push(compression_flag);
                final_buffer.push(DatagramHeader::HAS_COMPRESSOR);
                final_buffer.extend_from_slice(&datagram_seq.get().to_be_bytes());
                final_buffer.extend_from_slice(&compressed);
                Bytes::from(final_buffer)
            }
        } else {
            // No compression - build datagram directly
            let mut final_buffer = Vec::with_capacity(DatagramHeader::SIZE + payload_data.len());
            final_buffer.push(compression_flag);
            final_buffer.push(DatagramHeader::HAS_COMPRESSOR);
            final_buffer.extend_from_slice(&datagram_seq.get().to_be_bytes());
            final_buffer.extend_from_slice(&payload_data);
            Bytes::from(final_buffer)
        };

        // GridMate: ++connection->m_dataGramSeqNum (increment after generating)
        self.datagram_seq_num = datagram_seq;

        // GridMate: dgram.m_flowControl.m_size = dataSize
        dgram_data.flow_control.size = datagram.len() as u16;

        // GridMate: m_trafficControl->OnSend(connection, dgram.m_flowControl)
        // TODO: Implement traffic control

        // GridMate: connection->m_sendDataGrams.push_back(dgram)
        // ALWAYS push to sendDataGrams queue for ACK tracking
        self.send_datagrams.push_back(dgram_data);

        Ok(Some(datagram))
    }

    /// Queue a message to send with automatic fragmentation (GridMate: GenerateSendMessages)
    ///
    /// If the message exceeds MAX_MESSAGE_DATA_SIZE, it will be split into chunks.
    /// Fragmented messages are forced to reliable mode (GridMate requirement).
    ///
    /// GridMate algorithm:
    /// - numChunks = 1 + ((dataSize - 1) / m_maxMsgDataSizeBytes) if dataSize > max
    /// - First chunk has m_numChunks = total, subsequent chunks decrement
    /// - All chunks share sequential sequence numbers
    pub fn queue_message(&mut self, message: MessageData, priority: usize) {
        let data_len = message.data.len();

        // Check if fragmentation is needed
        if data_len <= mtu::MAX_MESSAGE_DATA_SIZE {
            // Single chunk - queue directly
            self.queue_message_chunk(message, priority, 1);
        } else {
            // Multi-chunk message - fragment it
            // GridMate: numChunks = 1 + ((dataSize - 1) / m_maxMsgDataSizeBytes)
            let num_chunks = mtu::chunks_needed(data_len);

            // Validate chunk count (GridMate: maxNumChunks = SequenceNumberHalfSpan - 1)
            if num_chunks > mtu::MAX_NUM_CHUNKS {
                tracing::trace!(
                    "[THREAD_CONN] Message too large to fragment: {} bytes requires {} chunks, max is {}",
                    data_len,
                    num_chunks,
                    mtu::MAX_NUM_CHUNKS
                );
                return;
            }

            tracing::debug!(
                "[THREAD_CONN] Fragmenting message: {} bytes into {} chunks of max {} bytes",
                data_len,
                num_chunks,
                mtu::MAX_MESSAGE_DATA_SIZE
            );

            // GridMate: fragments force reliable mode
            let reliability = DataReliability::Reliable;

            let mut remaining_data = message.data.as_ref();
            let mut chunks_remaining = num_chunks;

            while !remaining_data.is_empty() {
                // Calculate chunk size (GridMate: dataSendStep)
                let chunk_size = std::cmp::min(remaining_data.len(), mtu::MAX_MESSAGE_DATA_SIZE);
                let (chunk_data, rest) = remaining_data.split_at(chunk_size);

                // Create chunk message
                let mut chunk_msg = MessageData::new();
                chunk_msg.channel = message.channel;
                chunk_msg.reliability = reliability;
                chunk_msg.is_connecting = message.is_connecting;
                chunk_msg.data = Bytes::copy_from_slice(chunk_data);
                // num_chunks will be set by queue_message_chunk

                // Queue the chunk (num_chunks decrements for each chunk per GridMate)
                self.queue_message_chunk(chunk_msg, priority, chunks_remaining as u16);

                remaining_data = rest;
                chunks_remaining -= 1;
            }
        }
    }

    /// Internal: Queue a single message chunk with specified num_chunks
    /// Assigns sequence numbers automatically (GridMate: GenerateSendMessages inner loop)
    fn queue_message_chunk(&mut self, mut message: MessageData, priority: usize, num_chunks: u16) {
        let channel = message.channel as usize;

        // Set num_chunks (first chunk has total, subsequent decrement)
        message.num_chunks = SequenceNumber::from(num_chunks);

        // Assign sequence number (GridMate: msg.m_sequenceNumber = ++conn->m_sendSeqNum[channel])
        self.send_seq_num[channel] = self.send_seq_num[channel].next();
        message.sequence_number = self.send_seq_num[channel];

        // Assign reliable sequence number for reliable messages
        // (GridMate: msg.m_sendReliableSeqNum = conn->m_sendReliableSeqNum[channel])
        if message.reliability == DataReliability::Reliable {
            self.send_reliable_seq_num[channel] = self.send_reliable_seq_num[channel].next();
        }
        message.send_reliable_seq_num = self.send_reliable_seq_num[channel];

        // Add to appropriate priority queue
        if priority < PRIORITY_MAX {
            self.to_send[priority].push_back(message);
        } else {
            self.to_send[0].push_back(message);
        }
    }

    /// Process retransmissions for lost datagrams (GridMate: ProcessResends).
    ///
    /// Iterates unACKed datagrams in send order. If a datagram's send time
    /// exceeds `LOST_PACKET_TIMEOUT`, its reliable messages are extracted and
    /// re-queued at the **front** of the send queue (highest priority within
    /// their original priority level), then the stale datagram is dropped.
    ///
    /// Stops at the first non-expired datagram since they are ordered by time.
    ///
    /// Returns the number of datagrams that were retransmitted.
    pub fn process_resends(&mut self) -> usize {
        /// Lumberyard: m_lostPacketTimeoutMS = 1000
        const LOST_PACKET_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1000);

        let now = Instant::now();
        let mut resend_count = 0;

        // Datagrams are stored oldest-first (FIFO push_back in prepare_outgoing_datagram).
        // Pop from front while expired; stop at first non-expired.
        while let Some(front) = self.send_datagrams.front() {
            let elapsed = now.duration_since(front.flow_control.sent_time);
            if elapsed <= LOST_PACKET_TIMEOUT {
                break; // Remaining datagrams are newer — nothing more to check
            }

            // This datagram is lost. Extract it.
            let mut dgram = self.send_datagrams.pop_front().unwrap();

            if dgram.resend_data_size > 0 {
                // Re-queue reliable messages at the FRONT of each priority queue
                // so they are sent before any new application messages.
                for priority in 0..PRIORITY_MAX {
                    if !dgram.to_resend[priority].is_empty() {
                        let resend_msgs: Vec<MessageData> =
                            dgram.to_resend[priority].drain(..).collect();
                        let count = resend_msgs.len();

                        // Splice resend messages to the front
                        let queue = &mut self.to_send[priority];
                        // VecDeque doesn't have splice, so rotate: push_front in reverse order
                        for msg in resend_msgs.into_iter().rev() {
                            queue.push_front(msg);
                        }

                        tracing::trace!(
                            "[RESEND] Re-queued {} messages from datagram seq={} (priority={}, elapsed={:.0?})",
                            count,
                            dgram.flow_control.sequence_number.get(),
                            priority,
                            elapsed,
                        );
                    }
                }
            } else {
                tracing::debug!(
                    "[RESEND] Dropping expired datagram seq={} (no reliable data, elapsed={:.0?})",
                    dgram.flow_control.sequence_number.get(),
                    elapsed,
                );
            }

            resend_count += 1;
        }

        resend_count
    }

    /// Apply NACK acceleration to a datagram (GridMate: OnNAck).
    ///
    /// When we receive an ACK range that skips over an earlier datagram,
    /// that datagram is implicitly NACKed. Each NACK subtracts 1/3 of the
    /// loss timeout from the datagram's sent_time, so after 3 NACKs the
    /// datagram appears to have exceeded the timeout on the next
    /// `process_resends()` check.
    fn nack_datagram(&mut self, seq: SequenceNumber) {
        /// Lumberyard: m_lostPacketTimeoutMS / N where N = 3
        const NACK_ACCELERATION: std::time::Duration = std::time::Duration::from_millis(1000 / 3);

        for dgram in &mut self.send_datagrams {
            if dgram.flow_control.sequence_number == seq {
                // Subtract from sent_time to make it appear older.
                // saturating_sub prevents underflow.
                dgram.flow_control.sent_time = dgram
                    .flow_control
                    .sent_time
                    .checked_sub(NACK_ACCELERATION)
                    .unwrap_or(dgram.flow_control.sent_time);
                return;
            }
        }
    }

    /// Borrow a per-channel inbox as an async stream. The carrier
    /// driver task drives network I/O + DTLS elsewhere; this stream
    /// just yields whatever has already been processed into the
    /// channel queue. See
    /// [`super::receive_stream::ReceiveMessageStream`].
    pub fn receive_stream(
        &mut self,
        channel: u8,
    ) -> super::receive_stream::ReceiveMessageStream<'_, T> {
        super::receive_stream::ReceiveMessageStream {
            connection: self,
            channel: channel as usize,
        }
    }
}

impl<T: CarrierTransport> ConnectionState<T> {
    /// Pop the next ready message from a channel's inbox (the
    /// `to_receive` per-channel queue). Used by
    /// [`super::receive_stream::ReceiveMessageStream`].
    pub(crate) fn pop_ready_message(&mut self, channel: usize) -> Option<MessageData> {
        self.to_receive[channel].pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::DriverError;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    struct NullTransport;

    impl CarrierTransport for NullTransport {
        fn peer_addr(&self) -> SocketAddr {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
        }

        async fn read(&mut self) -> Result<Bytes, DriverError> {
            Err(DriverError::ConnectionClosed)
        }

        fn try_recv(&mut self) -> Result<Option<Bytes>, DriverError> {
            Ok(None)
        }

        async fn write(&mut self, data: Bytes) -> Result<usize, DriverError> {
            Ok(data.len())
        }
    }

    #[test]
    fn default_connection_uses_framework_protocol_profile() {
        let conn = ConnectionState::new(NullTransport);

        assert_eq!(conn.protocol_version(), 1);
        assert!(!conn.send_compressed());
    }

    #[test]
    fn explicit_profile_drives_connect_request_and_ack_payloads() {
        let mut conn = ConnectionState::with_protocol_profile(
            NullTransport,
            CarrierProtocolProfile::new(5, true),
        );

        assert_eq!(
            conn.make_connect_request_payload().as_ref(),
            &[0, 0, 0, 5, system_message::SM_CONNECT_REQUEST]
        );

        conn.queue_connect_ack();
        let ack = conn.to_send[0].pop_front().expect("queued ACK");
        assert_eq!(
            ack.data.as_ref(),
            &[0, 0, 0, 5, system_message::SM_CONNECT_ACK]
        );
    }
}
