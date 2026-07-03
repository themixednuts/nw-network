// GridSession - Session management layer
// Following GridMate/Session/Session.h and Session.cpp
//
// GridMate hierarchy:
// GridSession
//   └─ Carrier (transport layer)
//   └─ ReplicaManager (replication layer)
//   └─ GridMembers (connected players)

use crate::Result;
use crate::carrier::{
    CarrierConnected, CarrierConnecting, CarrierDesc, CarrierImpl, DataReliability,
    DisconnectReason,
};
use tracing::trace;

/// Session ID type (GridMate: SessionID).
///
/// `Deref<Target = str>` is forwarded to the inner `String`'s deref so
/// `&SessionID` substitutes for `&str` directly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Deref, derive_more::From)]
pub struct SessionID(#[deref(forward)] pub String);

impl SessionID {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for SessionID {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

/// Member ID type (GridMate: MemberID)
/// Newtype for type safety
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    derive_more::Deref,
    derive_more::From,
    derive_more::Into,
)]
pub struct MemberID(pub u32);

impl MemberID {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

/// Connection ID type (GridMate: ConnectionID from Carrier)
/// Newtype for type safety
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    derive_more::Deref,
    derive_more::From,
    derive_more::Into,
)]
pub struct ConnectionID(pub u32);

impl ConnectionID {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

/// Carrier channel ids used by application traffic.
///
/// Source `Carrier::Send`/`Receive` exposes this as an `unsigned char channel`.
/// Captures identify channel 0 as ordinary typed game traffic and channel 1 as
/// the replicated-state lane. Both application lanes carry Hub IMessage
/// envelopes; channel 1 envelopes are expected to contain
/// `ReplicatedStateBundle` bodies. Channel 3 is GridMate's internal system
/// channel, mirrored by `carrier::SYSTEM_CHANNEL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CarrierChannel {
    /// Ordinary typed game messages.
    GameData = 0,
    /// Replicated state bundle traffic.
    ReplicatedStateBundle = 1,
    /// Application channel reserved by the carrier but not yet mapped.
    Application2 = 2,
    /// Internal carrier system messages.
    System = super::carrier::SYSTEM_CHANNEL,
}

impl CarrierChannel {
    #[inline]
    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    #[inline]
    #[must_use]
    pub const fn from_u8(channel: u8) -> Option<Self> {
        match channel {
            0 => Some(Self::GameData),
            1 => Some(Self::ReplicatedStateBundle),
            2 => Some(Self::Application2),
            super::carrier::SYSTEM_CHANNEL => Some(Self::System),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn is_application(self) -> bool {
        !matches!(self, Self::System)
    }

    #[inline]
    #[must_use]
    pub const fn carries_hub_messages(self) -> bool {
        matches!(self, Self::GameData | Self::ReplicatedStateBundle)
    }

    #[inline]
    #[must_use]
    pub const fn carries_state_bundles(self) -> bool {
        matches!(self, Self::ReplicatedStateBundle)
    }
}

#[cfg(test)]
mod carrier_channel_tests {
    use super::*;

    #[test]
    fn carrier_channels_match_application_lanes() {
        assert_eq!(CarrierChannel::GameData.id(), 0);
        assert_eq!(CarrierChannel::ReplicatedStateBundle.id(), 1);
        assert_eq!(CarrierChannel::Application2.id(), 2);
        assert_eq!(
            CarrierChannel::System.id(),
            super::super::carrier::SYSTEM_CHANNEL
        );

        assert_eq!(CarrierChannel::from_u8(0), Some(CarrierChannel::GameData));
        assert_eq!(
            CarrierChannel::from_u8(1),
            Some(CarrierChannel::ReplicatedStateBundle)
        );
        assert_eq!(
            CarrierChannel::from_u8(2),
            Some(CarrierChannel::Application2)
        );
        assert_eq!(CarrierChannel::from_u8(3), Some(CarrierChannel::System));
        assert_eq!(CarrierChannel::from_u8(4), None);

        assert!(CarrierChannel::GameData.is_application());
        assert!(CarrierChannel::ReplicatedStateBundle.is_application());
        assert!(CarrierChannel::Application2.is_application());
        assert!(!CarrierChannel::System.is_application());

        assert!(CarrierChannel::GameData.carries_hub_messages());
        assert!(CarrierChannel::ReplicatedStateBundle.carries_hub_messages());
        assert!(!CarrierChannel::Application2.carries_hub_messages());
        assert!(!CarrierChannel::System.carries_hub_messages());

        assert!(!CarrierChannel::GameData.carries_state_bundles());
        assert!(CarrierChannel::ReplicatedStateBundle.carries_state_bundles());
        assert!(!CarrierChannel::Application2.carries_state_bundles());
        assert!(!CarrierChannel::System.carries_state_bundles());
    }
}

/// Session result (GridMate: GridSession::Result)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionResult {
    /// Operation succeeded
    Ok = 0,
    /// Operation failed
    Error,
}

/// Session topology (GridMate: SessionTopology)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTopology {
    /// Peer to peer
    PeerToPeer,
    /// Client/Server
    ClientServer,
}

/// Session parameter (GridMate: GridSessionParam)
#[derive(Debug, Clone)]
pub struct GridSessionParam {
    pub id: String,
    pub value: String,
}

/// GridMember - Represents a member in the session (GridMate: class GridMember)
/// Basic implementation - full member management is engine-level
pub struct GridMember {
    /// Member ID
    id: MemberID,
    /// Connection ID
    connection_id: ConnectionID,
    /// Is this member the host
    is_host: bool,
}

impl GridMember {
    /// Check if this is the host (GridMate: IsHost)
    pub fn is_host(&self) -> bool {
        self.is_host
    }

    /// Get member ID (GridMate: GetId)
    pub fn get_id(&self) -> MemberID {
        self.id
    }

    /// Get connection ID (GridMate: GetConnectionId)
    pub fn get_connection_id(&self) -> ConnectionID {
        self.connection_id
    }
}

/// Type-level state markers for GridSession
/// These encode the session state in the type system, preventing invalid operations at compile time
// Re-export shared state markers for convenience
pub use crate::state::{Connected, Connecting};

/// Idle state - no carrier, not connected (session-specific)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Idle;

/// InSession state - fully connected and validated (session-specific)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InSession;

/// Leaving state - transitioning out (session-specific)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leaving;

/// Inner state shared across all session states (enables safe state transitions)
pub(crate) struct GridSessionInner<CarrierState>
where
    CarrierState: Send + Sync,
{
    /// Session ID (GridMate: m_sessionId)
    pub(crate) session_id: SessionID,

    /// Carrier transport layer (GridMate: m_carrier)
    pub(crate) carrier: Option<CarrierImpl<CarrierState>>,

    /// Carrier descriptor (GridMate: m_carrierDesc)
    pub(crate) carrier_desc: CarrierDesc,

    /// My member (GridMate: m_myMember)
    pub(crate) my_member: Option<GridMember>,

    /// Connected members (GridMate: m_members)
    pub(crate) members: Vec<GridMember>,

    /// Session parameters (GridMate: m_state->m_params)
    pub(crate) params: Vec<GridSessionParam>,

    /// Host migration in process (GridMate: m_hostMigrationInProcess)
    pub(crate) host_migration_in_process: bool,

    /// Session topology (GridMate: m_topology)
    pub(crate) topology: SessionTopology,

    /// Number of public slots used (GridMate: m_numUsedPublicSlots)
    pub(crate) num_used_public_slots: u8,

    /// Number of private slots used (GridMate: m_numUsedPrivateSlots)
    pub(crate) num_used_private_slots: u8,

    /// Number of free public slots (GridMate: m_numFreePublicSlots)
    pub(crate) num_free_public_slots: u8,

    /// Number of free private slots (GridMate: m_numFreePrivateSlots)
    pub(crate) num_free_private_slots: u8,
}

/// GridSession - Main session management (GridMate: class GridSession)
/// Type-level state machine: Both session state and carrier state are encoded in type parameters
pub struct GridSession<State = Connecting, CarrierState = CarrierConnecting>
where
    State: Send + Sync,
    CarrierState: Send + Sync,
{
    /// Option enables moving inner out during state transitions
    inner: Option<GridSessionInner<CarrierState>>,
    _state: std::marker::PhantomData<State>,
}

impl<State, CarrierState> GridSession<State, CarrierState>
where
    State: Send + Sync,
    CarrierState: Send + Sync,
{
    /// Take the inner state for state transitions
    fn take_inner(&mut self) -> GridSessionInner<CarrierState> {
        self.inner.take().expect("GridSessionInner already taken")
    }

    /// Get a reference to the inner state
    fn inner(&self) -> &GridSessionInner<CarrierState> {
        self.inner.as_ref().expect("GridSessionInner not present")
    }

    /// Get a mutable reference to the inner state
    pub(crate) fn inner_mut(&mut self) -> &mut GridSessionInner<CarrierState> {
        self.inner.as_mut().expect("GridSessionInner not present")
    }
}

impl<State, CarrierState> GridSession<State, CarrierState>
where
    State: Send + Sync,
    CarrierState: Send + Sync,
{
    /// Get session ID
    pub fn id(&self) -> &SessionID {
        &self.inner().session_id
    }

    /// Get session ID as string slice
    pub fn id_str(&self) -> &str {
        &self.inner().session_id
    }

    /// Get carrier descriptor
    pub fn carrier_desc(&self) -> &CarrierDesc {
        &self.inner().carrier_desc
    }

    /// Get mutable carrier reference (for internal use)
    pub(crate) fn carrier_mut(&mut self) -> Option<&mut CarrierImpl<CarrierState>> {
        self.inner_mut().carrier.as_mut()
    }
}

impl GridSession<Connecting, CarrierConnecting> {
    /// Create and join a session (GridMate: GridSession constructor + Initialize)
    #[cfg(feature = "client")]
    pub async fn join(carrier_desc: CarrierDesc) -> Result<Self> {
        // GridMate: Initialize - creates carrier
        let carrier = CarrierImpl::new(carrier_desc.clone()).await?;

        let inner = GridSessionInner {
            session_id: SessionID::new(String::new()),
            carrier_desc,
            carrier: Some(carrier),
            my_member: None,
            members: Vec::new(),
            params: Vec::new(),
            host_migration_in_process: false,
            topology: SessionTopology::ClientServer,
            num_used_public_slots: 0,
            num_used_private_slots: 0,
            num_free_public_slots: 0,
            num_free_private_slots: 0,
        };

        Ok(Self {
            inner: Some(inner),
            _state: std::marker::PhantomData,
        })
    }

    /// Update session - legacy no-op for async actor
    /// For type transitions, use explicit transition methods like wait_until_connected()
    pub async fn update(&mut self) -> Result<()> {
        Ok(())
    }

    /// Check if carrier exists (for transition detection)
    pub fn has_carrier(&self) -> bool {
        self.inner().carrier.is_some()
    }

    /// Wait until carrier is connected, then transition to Connected state
    pub async fn wait_until_connected(
        mut self,
    ) -> Result<GridSession<Connected, CarrierConnected>> {
        // Take inner for transition
        let mut session_inner = self.take_inner();

        // Take carrier out, transition it, then put it back
        if let Some(carrier) = session_inner.carrier.take() {
            let carrier_connected = carrier.ready().await?;

            // Create new inner with connected carrier
            let new_inner = GridSessionInner {
                session_id: session_inner.session_id,
                carrier_desc: session_inner.carrier_desc,
                carrier: Some(carrier_connected),
                my_member: session_inner.my_member,
                members: session_inner.members,
                params: session_inner.params,
                host_migration_in_process: session_inner.host_migration_in_process,
                topology: session_inner.topology,
                num_used_public_slots: session_inner.num_used_public_slots,
                num_used_private_slots: session_inner.num_used_private_slots,
                num_free_public_slots: session_inner.num_free_public_slots,
                num_free_private_slots: session_inner.num_free_private_slots,
            };

            return Ok(GridSession {
                inner: Some(new_inner),
                _state: std::marker::PhantomData,
            });
        }

        // No carrier - this is an error state
        Err(crate::GridMateError::InvalidState(
            "No carrier to connect".into(),
        ))
    }
}

/// State-specific implementations for Connected state
impl GridSession<Connected, CarrierConnected> {
    /// Get carrier (guaranteed to be Some in Connected state)
    pub fn get_carrier(&self) -> &CarrierImpl<CarrierConnected> {
        self.inner()
            .carrier
            .as_ref()
            .expect("Carrier must exist in Connected state")
    }

    /// Get mutable carrier reference (guaranteed to be Some in Connected state)
    pub fn get_carrier_mut(&mut self) -> &mut CarrierImpl<CarrierConnected> {
        self.inner_mut()
            .carrier
            .as_mut()
            .expect("Carrier must exist in Connected state")
    }

    /// Transition to InSession state
    /// GridMate: After successful authentication, move from SS_CONNECTED to SS_IN_SESSION
    /// Consumes self and returns GridSession<InSession, CarrierConnected>
    ///
    /// This should be called by the game layer after authentication completes
    pub fn transition_to_in_session(mut self) -> GridSession<InSession, CarrierConnected> {
        trace!("Transitioning to InSession");

        // Safe state transition: take inner and move to new type
        let inner = self.take_inner();
        GridSession {
            inner: Some(inner),
            _state: std::marker::PhantomData,
        }
    }

    /// Leave session (GridMate: Leave)
    /// Consumes self and returns GridSession<Idle>
    /// Only available in Connected state
    pub async fn leave(
        mut self,
        migrate_host: bool,
    ) -> Result<GridSession<Idle, crate::carrier::CarrierDisconnected>> {
        let mut session_inner = self.take_inner();

        if let Some(carrier) = session_inner.carrier.take() {
            // GridMate: If host and migrate_host, trigger migration
            let _ = migrate_host; // Suppress unused warning

            // GridMate: Disconnect from carrier
            let _disconnecting = carrier.disconnect(DisconnectReason::UserRequested).await?;
            // Disconnecting carrier will be dropped here
        }

        // Create new inner for Idle state (no carrier)
        let new_inner: GridSessionInner<crate::carrier::CarrierDisconnected> = GridSessionInner {
            session_id: session_inner.session_id,
            carrier_desc: session_inner.carrier_desc,
            carrier: None,
            my_member: session_inner.my_member,
            members: session_inner.members,
            params: session_inner.params,
            host_migration_in_process: session_inner.host_migration_in_process,
            topology: session_inner.topology,
            num_used_public_slots: session_inner.num_used_public_slots,
            num_used_private_slots: session_inner.num_used_private_slots,
            num_free_public_slots: session_inner.num_free_public_slots,
            num_free_private_slots: session_inner.num_free_private_slots,
        };

        Ok(GridSession {
            inner: Some(new_inner),
            _state: std::marker::PhantomData,
        })
    }
}

/// State-specific implementations for InSession state
impl GridSession<InSession, CarrierConnected> {
    /// Get carrier (guaranteed to be Some in InSession state)
    pub fn get_carrier(&self) -> &CarrierImpl<CarrierConnected> {
        self.inner()
            .carrier
            .as_ref()
            .expect("Carrier must exist in InSession state")
    }

    /// Get mutable carrier reference (guaranteed to be Some in InSession state)
    pub fn get_carrier_mut(&mut self) -> &mut CarrierImpl<CarrierConnected> {
        self.inner_mut()
            .carrier
            .as_mut()
            .expect("Carrier must exist in InSession state")
    }

    /// Get members (available in InSession state)
    pub fn members(&self) -> &[GridMember] {
        &self.inner().members
    }

    /// Get member by index (available in InSession state)
    pub fn member_by_index(&self, index: usize) -> Option<&GridMember> {
        self.inner().members.get(index)
    }

    /// Get member by ID (available in InSession state)
    pub fn member_by_id(&self, id: MemberID) -> Option<&GridMember> {
        self.inner().members.iter().find(|m| m.id == id)
    }

    /// Get host member (available in InSession state)
    pub fn host(&self) -> Option<&GridMember> {
        self.inner().members.iter().find(|m| m.is_host)
    }

    /// Leave session (GridMate: Leave)
    /// Consumes self and returns GridSession<Idle>
    /// Only available in InSession state
    pub async fn leave(
        mut self,
        migrate_host: bool,
    ) -> Result<GridSession<Idle, crate::carrier::CarrierDisconnected>> {
        let mut session_inner = self.take_inner();

        if let Some(carrier) = session_inner.carrier.take() {
            // GridMate: If host and migrate_host, trigger migration
            let _ = migrate_host; // Suppress unused warning

            // GridMate: Disconnect from carrier
            let _disconnecting = carrier.disconnect(DisconnectReason::UserRequested).await?;
            // Disconnecting carrier will be dropped here
        }

        // Create new inner for Idle state (no carrier)
        let new_inner: GridSessionInner<crate::carrier::CarrierDisconnected> = GridSessionInner {
            session_id: session_inner.session_id,
            carrier_desc: session_inner.carrier_desc,
            carrier: None,
            my_member: session_inner.my_member,
            members: session_inner.members,
            params: session_inner.params,
            host_migration_in_process: session_inner.host_migration_in_process,
            topology: session_inner.topology,
            num_used_public_slots: session_inner.num_used_public_slots,
            num_used_private_slots: session_inner.num_used_private_slots,
            num_free_public_slots: session_inner.num_free_public_slots,
            num_free_private_slots: session_inner.num_free_private_slots,
        };

        Ok(GridSession {
            inner: Some(new_inner),
            _state: std::marker::PhantomData,
        })
    }

    /// Send a typed message on a channel
    /// Only available in InSession state
    ///
    /// Generic method for sending any message that implements [`Message`].
    /// Uses MessageMetadata on the client-egress path: CRC/size/correlation
    /// wrapped around a MessageEnvelope.
    ///
    /// [`Message`]: crate::message::Message
    pub async fn send_message<T>(
        &mut self,
        message: T,
        channel: u8,
        reliability: DataReliability,
        priority: usize,
    ) -> Result<()>
    where
        T: crate::message::Sendable<crate::message::ClientToServer>,
    {
        use crate::message::{ClientToServer, EgressPath, MessageMetadata};
        use crate::serialize::buffer::{CARRIER_ENDIAN, WriteBuffer};

        ClientToServer::validate_message::<T>()?;

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        let mut metadata = MessageMetadata::<T, ClientToServer>::new(message);
        metadata.marshal(&mut wb);

        // `Bytes::from(Vec)` is zero-copy (takes ownership of the
        // backing allocation).
        let payload = bytes::Bytes::from(wb.into_vec());
        self.send(channel, payload, reliability, priority).await
    }
}

impl<State, CarrierState> GridSession<State, CarrierState>
where
    State: Send + Sync,
    CarrierState: Send + Sync,
{
    // ============================================================================
    // Session Parameters (GridMate: Session parameter management)
    // ============================================================================

    /// Get number of parameters (GridMate: GetNumParams)
    pub fn get_num_params(&self) -> usize {
        self.inner().params.len()
    }

    /// Get parameter by index (GridMate: GetParam)
    pub fn get_param(&self, index: usize) -> Option<&GridSessionParam> {
        self.inner().params.get(index)
    }

    /// Set/update parameter (GridMate: SetParam)
    pub fn set_param(&mut self, param: GridSessionParam) -> bool {
        // Find existing or add new
        if let Some(existing) = self
            .inner_mut()
            .params
            .iter_mut()
            .find(|p| p.id == param.id)
        {
            *existing = param;
        } else {
            self.inner_mut().params.push(param);
        }
        true
    }

    /// Remove parameter by ID (GridMate: RemoveParam by id)
    pub fn remove_param_by_id(&mut self, param_id: &str) -> bool {
        let params = &mut self.inner_mut().params;
        if let Some(pos) = params.iter().position(|p| p.id == param_id) {
            params.remove(pos);
            true
        } else {
            false
        }
    }

    /// Remove parameter by index (GridMate: RemoveParam by index)
    pub fn remove_param_by_index(&mut self, index: usize) -> bool {
        let params = &mut self.inner_mut().params;
        if index < params.len() {
            params.remove(index);
            true
        } else {
            false
        }
    }

    // ============================================================================
    // Carrier Access (GridMate: GetCarrier, GetCarrierDesc)
    // ============================================================================

    /// Get carrier descriptor (GridMate: GetCarrierDesc)
    pub fn get_carrier_desc(&self) -> &CarrierDesc {
        &self.inner().carrier_desc
    }

    // ============================================================================
    // Message Receiving (GridMate: via GridMember or direct carrier access)
    // ============================================================================

    /// Send data to the session (available via Carrier trait)
    /// Note: Type-level state-specific send() is preferred - see impl blocks for Connected and InSession
    ///
    /// Accepts `impl Into<Bytes>` — callers with an existing `Bytes`
    /// (the typed-encode path produces `Bytes::from(Vec<u8>)`) get
    /// zero-copy Arc bump. `&[u8]` callers can `Bytes::copy_from_slice`
    /// at the boundary; we don't accept `&[u8]` directly because the
    /// async actor outlives the borrow.
    pub async fn send(
        &mut self,
        channel: u8,
        data: impl Into<bytes::Bytes>,
        reliability: DataReliability,
        priority: usize,
    ) -> Result<()> {
        if let Some(ref mut carrier) = self.inner_mut().carrier {
            carrier.send(data, reliability, priority, channel).await
        } else {
            Err(crate::GridMateError::InvalidState(
                "Carrier not initialized".to_string(),
            ))
        }
    }

    // ============================================================================
    // Member Management (GridMate: KickMember, BanMember, etc.)
    // ============================================================================

    /// Kick member from session (GridMate: KickMember)
    /// Engine-level feature - not implemented at network layer
    pub async fn kick_member(&mut self, _member_id: MemberID, _reason: u8) -> SessionResult {
        SessionResult::Error
    }

    /// Ban member from session (GridMate: BanMember)
    /// Engine-level feature - not implemented at network layer
    pub async fn ban_member(&mut self, _member_id: MemberID, _reason: u8) -> SessionResult {
        SessionResult::Error
    }

    // ============================================================================
    // Session Control (GridMate: Leave, LockSession, UnlockSession)
    // ============================================================================

    // Note: leave() is state-specific - see impl blocks for Connected and InSession

    /// Lock session (GridMate: LockSession) - Not supported yet
    pub fn lock_session(&mut self) -> SessionResult {
        SessionResult::Error
    }

    /// Unlock session (GridMate: UnlockSession) - Not supported yet
    pub fn unlock_session(&mut self) -> SessionResult {
        SessionResult::Error
    }
}

/// Implement Drop for graceful cleanup
impl<State, CarrierState> Drop for GridSession<State, CarrierState>
where
    State: Send + Sync,
    CarrierState: Send + Sync,
{
    fn drop(&mut self) {
        // GridMate: ~GridSession() calls Shutdown()
        // Only clean up if inner wasn't taken (state transition)
        if let Some(inner) = self.inner.as_mut()
            && let Some(_carrier) = inner.carrier.take()
        {
            // Drop handles cleanup via CarrierImpl::Drop
        }
    }
}
