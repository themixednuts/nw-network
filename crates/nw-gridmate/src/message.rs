use crate::network_schema::type_by_type_id;
use crate::serialize::{
    CARRIER_ENDIAN, Marshaler, MarshalerError, ReadBuffer, VlqU32Marshaler, WriteBuffer,
};
use crate::types::{AzRtti, TypeRegistryEntry};
use std::{any::Any, fmt::Debug, marker::PhantomData};
use uuid::Uuid;

/// UUID of the Hub `IMessage` abstract base class.
pub const IMESSAGE_BASE_UUID: Uuid = Uuid::from_u128(0x7F4B41C6_06AE_47A1_A144_5F8BA048B0EF);

/// Actor placeholder accepted by [`Message::execute`].
/// Execution belongs to higher-level systems; the transport layer only
/// preserves the hook in the public message contract.
pub trait Actor: Any + Debug + Send + Sync {}

/// Top-level Hub message payload.
///
/// A message has both a stable UUID ([`AzRtti`]) and a compact wire index
/// ([`TypeRegistryEntry`]). The compact index is what normal Hub message
/// envelopes carry; UUID envelopes are still accepted and resolved through the
/// generated network schema.
pub trait Message: AzRtti + TypeRegistryEntry + Marshaler + Debug + Send + Sync {
    /// Message envelope index.
    const TYPE_INDEX: u32 = <Self as TypeRegistryEntry>::TYPE_INDEX;

    fn params_to_string(&self) -> String {
        "...".to_owned()
    }

    /// Human-readable diagnostic string for logs and tools.
    fn message_string(&self) -> String {
        format!(
            "{}({})",
            <Self as AzRtti>::TYPE_NAME,
            self.params_to_string()
        )
    }

    /// Optional execution hook for embedders that want direct dispatch.
    fn execute(&self, _actor: &mut dyn Actor) -> bool {
        false
    }
}

impl<T> Message for T where T: AzRtti + TypeRegistryEntry + Marshaler + Debug + Send + Sync {}

/// Source-level direction markers for the Hub message wrapper.
pub mod path {
    #[derive(Debug, Clone, Copy, Default)]
    pub struct ClientToServer;

    #[derive(Debug, Clone, Copy, Default)]
    pub struct ServerToClient;
}

pub use path::{ClientToServer, ServerToClient};

/// Wire path marker. The path determines which metadata wraps the
/// [`MessageEnvelope`]; it is intentionally separate from `T` because a few
/// Hub messages, notably ping, use the same message type in both directions.
pub trait MessagePath {
    type Metadata: Debug + Clone;
    const NAME: &'static str;
}

impl MessagePath for ClientToServer {
    type Metadata = CorrelatedMetadata;
    const NAME: &'static str = "ClientToServer";
}

impl MessagePath for ServerToClient {
    type Metadata = SizedEnvelopeMetadata;
    const NAME: &'static str = "ServerToClient";
}

/// An outbound [`MessagePath`] — knows how to encode a typed message
/// into wire bytes including its path-specific outer framing.
///
/// `Context` is the per-path "what the call site supplies" type:
/// `Uuid` for [`ClientToServer`] (the correlation slot), `()` for [`ServerToClient`] (sized frame
/// has no outer header beyond the VLQ size). Implementations only
/// take what they actually consume — there's no `correlation_id`
/// param ignored at the ServerToClient impl.
pub trait EgressPath: MessagePath + Sized + 'static {
    /// Path-specific framing input supplied at the call site.
    type Context: Default + Send + 'static;

    fn marshal_framed<T: Message + Marshaler>(msg: T, context: Self::Context, wb: &mut WriteBuffer);

    fn validate_message<T: Message>() -> crate::Result<()> {
        ensure_direct_message::<T>(Self::NAME)
    }
}

impl EgressPath for ClientToServer {
    type Context = Uuid;
    fn marshal_framed<T: Message + Marshaler>(msg: T, correlation_id: Uuid, wb: &mut WriteBuffer) {
        let mut meta = MessageMetadata::<T, Self>::with_correlation_id(msg, correlation_id);
        meta.marshal(wb);
    }
}

impl EgressPath for ServerToClient {
    type Context = ();
    fn marshal_framed<T: Message + Marshaler>(msg: T, _: (), wb: &mut WriteBuffer) {
        let mut meta = MessageMetadata::<T, Self>::new(msg);
        meta.marshal(wb);
    }
}

/// An inbound [`MessagePath`] — knows how to decode a typed message
/// from wire bytes including its path-specific outer framing.
///
/// Direction markers are source-level directions, not local endpoint
/// roles: [`ClientToServer`] reads/writes the correlated C→S frame,
/// and [`ServerToClient`] reads/writes the sized S→C frame.
pub trait IngressPath: MessagePath + Sized + 'static {
    fn read_framed<T: Message + Marshaler>(
        rb: &mut ReadBuffer,
    ) -> Result<MessageMetadata<T, Self>, MarshalerError>;
}

impl IngressPath for ClientToServer {
    fn read_framed<T: Message + Marshaler>(
        rb: &mut ReadBuffer,
    ) -> Result<MessageMetadata<T, Self>, MarshalerError> {
        MessageMetadata::<T, Self>::unmarshal(rb)
    }
}

impl IngressPath for ServerToClient {
    fn read_framed<T: Message + Marshaler>(
        rb: &mut ReadBuffer,
    ) -> Result<MessageMetadata<T, Self>, MarshalerError> {
        MessageMetadata::<T, Self>::unmarshal(rb)
    }
}

/// A message that travels over wire egress path `P`. Parameterised by
/// the path rather than an associated type so a single message type
/// can implement [`Sendable`] for multiple directions — the canonical
/// case is `PingMsg`, which the server originates over
/// [`ServerToClient`] and the client echoes back over [`ClientToServer`].
///
/// Sinks fix the direction at the call site by either constraining
/// `P` to a specific path (e.g. a server-only listener that always
/// emits via [`ServerToClient`]) or by leaving `P` generic and letting
/// type inference pick from the message's implemented directions.
///
/// `#[derive(Message)]` can emit these impls from source declaration
/// direction (`#[message(client_to_server)]` / `#[message(server_to_client)]`).
/// Hand-written impls remain appropriate for bidirectional or exceptional
/// Hub messages.
pub trait Sendable<P: EgressPath>: Message + Marshaler + Send + 'static {}

/// A message that arrives over wire ingress path `P`. Mirror of
/// [`Sendable<P>`]: parameterised by the path so a single message
/// type can implement [`Receivable`] for both [`ClientToServer`] and
/// [`ServerToClient`] when the protocol uses the same shape in both
/// directions (`PingMsg`).
pub trait Receivable<P: IngressPath>: Message + Marshaler + Send + 'static {}

impl<T, P> Sendable<P> for T
where
    T: Message + Marshaler + Send + 'static,
    P: EgressPath,
{
}

impl<T, P> Receivable<P> for T
where
    T: Message + Marshaler + Send + 'static,
    P: IngressPath,
{
}

/// C->S Hub metadata: CRC/size/correlation followed by a MessageEnvelope.
///
/// Wire:
///
/// ```text
/// [crc32:u32-be]
/// [payload_size:u32-be = correlation_id(16) + MessageEnvelope.len]
/// [correlation_id:uuid]
/// [MessageEnvelope]
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorrelatedMetadata {
    pub checksum: u32,
    pub payload_size: u32,
    pub correlation_id: Uuid,
}

impl CorrelatedMetadata {
    #[inline]
    pub const fn new(correlation_id: Uuid) -> Self {
        Self {
            checksum: 0,
            payload_size: 0,
            correlation_id,
        }
    }
}

/// S->C Hub stream item metadata: VLQ envelope size followed by a
/// MessageEnvelope.
///
/// Wire:
///
/// ```text
/// [envelope_size:vlq32 = MessageEnvelope.len]
/// [MessageEnvelope]
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SizedEnvelopeMetadata {
    pub envelope_size: u32,
}

/// Message identity encoded inside a Hub message envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageTypeId {
    TypeIndex(u32),
    Uuid(Uuid),
}

impl MessageTypeId {
    #[inline]
    pub const fn type_index(self) -> Option<u32> {
        match self {
            MessageTypeId::TypeIndex(type_index) => Some(type_index),
            MessageTypeId::Uuid(_) => None,
        }
    }

    #[inline]
    pub fn resolved_type_index(self) -> Option<u32> {
        match self {
            MessageTypeId::TypeIndex(type_index) => Some(type_index),
            MessageTypeId::Uuid(uuid) => resolve_type_index(uuid),
        }
    }

    #[inline]
    fn matches<T: Message>(self) -> bool {
        match self {
            MessageTypeId::TypeIndex(type_index) => type_index == <T as Message>::TYPE_INDEX,
            MessageTypeId::Uuid(uuid) => uuid == <T as AzRtti>::TYPE_ID,
        }
    }

    /// Resolve to the numeric type index. Returns `0` when an inbound UUID is
    /// not present in the generated network schema.
    #[inline]
    pub fn display_index(self) -> u32 {
        match self {
            MessageTypeId::TypeIndex(type_index) => type_index,
            MessageTypeId::Uuid(uuid) => resolve_type_index(uuid).unwrap_or(0),
        }
    }
}

/// Borrowed wire view of one Hub message envelope.
///
/// This is intentionally not a second high-level envelope abstraction. It is
/// the dynamic-dispatch read path: receive/capture code has to read
/// `messageTypeId` before it can choose a concrete `MessageEnvelope<T>`.
#[derive(Debug, Clone, Copy)]
pub struct MessageEnvelopeView<'a> {
    pub raw: &'a [u8],
    pub outer_flags: u8,
    pub field1: Option<[u8; 8]>,
    pub field2: Option<[u8; 8]>,
    pub envelope_flags: u8,
    pub type_id: MessageTypeId,
    pub body: &'a [u8],
}

impl MessageEnvelopeView<'_> {
    #[inline]
    pub const fn type_index(&self) -> Option<u32> {
        self.type_id.type_index()
    }
}

/// Hub message envelope.
///
/// ```text
/// [outer_flags:u8]
/// [field1:u64? if outer_flags & 0x01]
/// [field2:u64? if outer_flags & 0x02]
/// [envelope_flags:u8]
/// [messageTypeId:vlq32 or UUID if 0]
/// [m_message bytes]
/// ```
///
/// Directional CRC/size/correlation and S->C VLQ stream sizing live in
/// [`MessageMetadata`], not here.
#[derive(Debug, Clone)]
pub struct MessageEnvelope<T> {
    pub message: T,
}

impl<T: Message> MessageEnvelope<T> {
    #[inline]
    pub(crate) const fn new(message: T) -> Self {
        Self { message }
    }

    pub(crate) fn marshal(&self, wb: &mut WriteBuffer) {
        0u8.marshal(wb);
        1u8.marshal(wb);
        VlqU32Marshaler.marshal(wb, <T as Message>::TYPE_INDEX);
        self.message.marshal(wb);
    }
}

/// Path-specific wrapper around a [`MessageEnvelope`].
#[derive(Debug, Clone)]
pub struct MessageMetadata<T, P: MessagePath = ClientToServer> {
    pub metadata: P::Metadata,
    pub envelope: MessageEnvelope<T>,
    _path: PhantomData<P>,
}

impl<T, P> MessageMetadata<T, P>
where
    P: MessagePath,
{
    #[inline]
    fn from_parts(metadata: P::Metadata, envelope: MessageEnvelope<T>) -> Self {
        Self {
            metadata,
            envelope,
            _path: PhantomData,
        }
    }

    #[inline]
    pub fn into_message(self) -> T {
        self.envelope.message
    }
}

impl<T: Message> MessageMetadata<T, ClientToServer> {
    pub fn new(message: T) -> Self {
        Self::with_correlation_id(message, Uuid::nil())
    }

    pub fn with_correlation_id(message: T, correlation_id: Uuid) -> Self {
        Self::from_parts(
            CorrelatedMetadata::new(correlation_id),
            MessageEnvelope::new(message),
        )
    }

    pub fn set_correlation_id(&mut self, correlation_id: Uuid) {
        self.metadata.correlation_id = correlation_id;
    }

    pub fn marshal(&mut self, wb: &mut WriteBuffer) {
        marshal_correlated_into(&mut self.metadata, &self.envelope, wb);
    }

    pub fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let (metadata, view) = read_correlated_message(rb)?;
        Ok(Self::from_parts(metadata, decode_typed_envelope(view)?))
    }
}

impl<T: Message> MessageMetadata<T, ServerToClient> {
    pub fn new(message: T) -> Self {
        Self::from_parts(
            SizedEnvelopeMetadata::default(),
            MessageEnvelope::new(message),
        )
    }

    pub fn marshal(&mut self, wb: &mut WriteBuffer) {
        marshal_sized_into(&mut self.metadata, &self.envelope, wb);
    }
    pub fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let (metadata, view) = read_sized_message(rb)?;
        Ok(Self::from_parts(metadata, decode_typed_envelope(view)?))
    }
}

/// Stream one Hub message envelope from `rb`, consuming
/// exactly `envelope_len` bytes. The returned [`MessageEnvelopeView`]
/// borrows the consumed region; `rb` is left positioned for follow-on
/// reads.
///
/// `envelope_flags == 0` (intentionally-empty record) surfaces as
/// [`MarshalerError::EmptyEnvelope`] so multi-record loops can
/// match-and-continue without conflating "no body" with malformed input.
pub fn read_message_envelope<'a>(
    rb: &mut ReadBuffer<'a>,
    envelope_len: usize,
) -> Result<MessageEnvelopeView<'a>, MarshalerError> {
    let envelope_bytes = rb.read_bytes(envelope_len)?;
    let mut local = ReadBuffer::new(CARRIER_ENDIAN, envelope_bytes);

    let outer_flags = local.read_u8()?;
    let field1 = if (outer_flags & 0x01) != 0 {
        Some(read_fixed_8(&mut local)?)
    } else {
        None
    };
    let field2 = if (outer_flags & 0x02) != 0 {
        Some(read_fixed_8(&mut local)?)
    } else {
        None
    };

    let envelope_flags = local.read_u8()?;
    if envelope_flags == 0 {
        return Err(MarshalerError::EmptyEnvelope);
    }
    if envelope_flags >= 2 {
        return Err(MarshalerError::InvalidEnvelopeFlags {
            flags: envelope_flags,
        });
    }

    let vlq_value = VlqU32Marshaler.unmarshal(&mut local)?;
    let type_id = if vlq_value == 0 {
        let uuid_bytes = local.read_bytes(16)?;
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(uuid_bytes);
        MessageTypeId::Uuid(Uuid::from_bytes(bytes))
    } else {
        MessageTypeId::TypeIndex(vlq_value)
    };

    let body = local.read_bytes(local.left())?;
    Ok(MessageEnvelopeView {
        raw: envelope_bytes,
        outer_flags,
        field1,
        field2,
        envelope_flags,
        type_id,
        body,
    })
}

/// Stream one client-to-server correlated message from `rb`: outer CRC32 / payload
/// size / correlation UUID, followed by an envelope. CRC is validated
/// against `correlation_id || envelope_bytes` before parsing.
pub fn read_correlated_message<'a>(
    rb: &mut ReadBuffer<'a>,
) -> Result<(CorrelatedMetadata, MessageEnvelopeView<'a>), MarshalerError> {
    let checksum = rb.read_u32()?;
    let payload_size = rb.read_u32()?;
    let correlation_id = Uuid::unmarshal(rb)?;

    let envelope_size = payload_size
        .checked_sub(16)
        .ok_or(MarshalerError::TruncatedPayload {
            declared: 16,
            available: payload_size as usize,
        })? as usize;
    if envelope_size > rb.left() {
        return Err(MarshalerError::TruncatedPayload {
            declared: envelope_size,
            available: rb.left(),
        });
    }

    // CRC covers `correlation_id || envelope_bytes`. Peek the slice (no
    // cursor advance) so we can validate, then stream-read the envelope.
    let envelope_bytes = &rb.remaining()[..envelope_size];
    let actual_crc = crc32_chunks(&[correlation_id.as_bytes(), envelope_bytes]);
    if actual_crc != checksum {
        return Err(MarshalerError::CrcMismatch {
            expected: checksum,
            actual: actual_crc,
        });
    }

    let view = read_message_envelope(rb, envelope_size)?;
    Ok((
        CorrelatedMetadata {
            checksum,
            payload_size,
            correlation_id,
        },
        view,
    ))
}

/// Stream one S→C sized message from `rb`: VLQ envelope size, followed
/// by an envelope. Caller is expected to gate this on `rb.is_empty()` —
/// exhausted buffers are a loop condition, not a parse outcome.
pub fn read_sized_message<'a>(
    rb: &mut ReadBuffer<'a>,
) -> Result<(SizedEnvelopeMetadata, MessageEnvelopeView<'a>), MarshalerError> {
    let envelope_size = VlqU32Marshaler.unmarshal(rb)?;
    if envelope_size == 0 || envelope_size as usize > rb.left() {
        return Err(MarshalerError::TruncatedPayload {
            declared: envelope_size as usize,
            available: rb.left(),
        });
    }

    let view = read_message_envelope(rb, envelope_size as usize)?;
    Ok((SizedEnvelopeMetadata { envelope_size }, view))
}

fn marshal_correlated_into<T: Message>(
    metadata: &mut CorrelatedMetadata,
    envelope: &MessageEnvelope<T>,
    wb: &mut WriteBuffer,
) {
    // 24-byte header: [crc32:u32][payload_size:u32][correlation_id:uuid].
    // `with_fixed_prefix` reserves 24 zero bytes, runs the envelope
    // marshal, then hands us back both the prefix region and the body
    // slice so we can compute CRC + size and patch them in place.
    let wrote = wb.with_fixed_prefix(
        24,
        |wb| {
            envelope.marshal(wb);
            true
        },
        |prefix, envelope_bytes| {
            metadata.payload_size = (16 + envelope_bytes.len()) as u32;
            metadata.checksum = crc32_chunks(&[metadata.correlation_id.as_bytes(), envelope_bytes]);
            prefix[0..4].copy_from_slice(&metadata.checksum.to_be_bytes());
            prefix[4..8].copy_from_slice(&metadata.payload_size.to_be_bytes());
            prefix[8..24].copy_from_slice(metadata.correlation_id.as_bytes());
        },
    );
    debug_assert!(wrote);
}

fn marshal_sized_into<T: Message>(
    metadata: &mut SizedEnvelopeMetadata,
    envelope: &MessageEnvelope<T>,
    wb: &mut WriteBuffer,
) {
    let mut envelope_buf = WriteBuffer::new(CARRIER_ENDIAN);
    envelope.marshal(&mut envelope_buf);
    metadata.envelope_size = envelope_buf.len() as u32;
    VlqU32Marshaler.marshal(wb, metadata.envelope_size);
    wb.write_bytes(envelope_buf.as_slice());
}

fn decode_typed_envelope<T: Message>(
    parsed: MessageEnvelopeView<'_>,
) -> Result<MessageEnvelope<T>, MarshalerError> {
    if !parsed.type_id.matches::<T>() {
        return Err(MarshalerError::MessageTypeMismatch {
            expected: <T as Message>::TYPE_INDEX,
            actual: parsed.type_id.display_index(),
        });
    }

    let mut body_rb = ReadBuffer::new(CARRIER_ENDIAN, parsed.body);
    let message = T::unmarshal(&mut body_rb)?;
    Ok(MessageEnvelope { message })
}

fn ensure_direct_message<T: Message>(path: &'static str) -> crate::Result<()> {
    let descriptor = crate::network_schema::type_by_type_index(<T as Message>::TYPE_INDEX);
    if descriptor.is_some_and(crate::network_schema::NetworkTypeDescriptor::is_direct_message) {
        return Ok(());
    }

    Err(crate::GridMateError::InvalidMessagePath {
        type_name: <T as AzRtti>::TYPE_NAME,
        type_index: <T as Message>::TYPE_INDEX,
        path,
    })
}

#[must_use]
pub fn resolve_type_index(uuid: Uuid) -> Option<u32> {
    type_by_type_id(uuid).map(|descriptor| descriptor.type_index)
}

fn crc32_chunks(chunks: &[&[u8]]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for chunk in chunks {
        for byte in *chunk {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    0xEDB8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn read_fixed_8(rb: &mut ReadBuffer) -> Result<[u8; 8], MarshalerError> {
    let bytes = rb.read_bytes(8)?;
    let mut out = [0u8; 8];
    out.copy_from_slice(bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::messages::ClientAddEntryMsg;
    use crate::{ReadBuffer, WriteBuffer};

    #[test]
    fn generated_message_round_trips_through_sized_envelope() {
        let message = ClientAddEntryMsg {
            field_0: [0xab; 16],
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        let mut metadata =
            MessageMetadata::<ClientAddEntryMsg, ServerToClient>::new(message.clone());
        metadata.marshal(&mut wb);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice());
        let decoded = MessageMetadata::<ClientAddEntryMsg, ServerToClient>::unmarshal(&mut rb)
            .expect("decode generated direct message")
            .into_message();

        assert_eq!(decoded, message);
    }

    #[cfg(any(feature = "client", feature = "server"))]
    #[test]
    fn event_decodes_generated_message_body() {
        let message = ClientAddEntryMsg {
            field_0: [0xcd; 16],
        };
        let mut body = WriteBuffer::new(CARRIER_ENDIAN);
        message.marshal(&mut body);

        let event = crate::Event::TypedReceived {
            session: crate::SessionId::new(0),
            channel: 0,
            type_index: <ClientAddEntryMsg as Message>::TYPE_INDEX,
            data: bytes::Bytes::from(body.into_vec()),
        };

        let decoded = event
            .typed_message::<ClientAddEntryMsg>()
            .expect("decode generated event body")
            .expect("typed event");

        assert_eq!(decoded, message);
    }

    #[test]
    fn generated_direct_message_is_valid_for_hub_egress() {
        ClientToServer::validate_message::<ClientAddEntryMsg>().expect("client egress direct msg");
        ServerToClient::validate_message::<ClientAddEntryMsg>().expect("server egress direct msg");
    }
}
