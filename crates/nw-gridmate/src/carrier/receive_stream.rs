//! Async stream of decoded carrier messages for one channel.
//!
//! [`ReceiveMessageStream`] is the application-facing receive end of
//! the carrier protocol: it polls already-processed messages out of
//! the per-channel inbox on a borrowed [`super::connection_state::
//! ConnectionState`]. Network I/O and DTLS decryption happen on the
//! carrier driver task elsewhere; this stream only drains the
//! channel-keyed [`super::message::MessageData`] queues the driver
//! populates.

use super::connection_state::ConnectionState;
use super::io::CarrierTransport;
use super::message::MessageData;
use super::types::MAX_CHANNELS;
use futures_util::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Async stream of received [`MessageData`] for a specific channel.
/// Yielded by [`super::connection_state::ConnectionState::receive_stream`].
pub struct ReceiveMessageStream<'a, T: CarrierTransport> {
    pub(super) connection: &'a mut ConnectionState<T>,
    pub(super) channel: usize,
}

impl<'a, T: CarrierTransport> Stream for ReceiveMessageStream<'a, T> {
    type Item = Result<MessageData, crate::GridMateError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = unsafe { self.as_mut().get_unchecked_mut() };

        // Reassembly happens inside `process_incoming_datagram` now
        // (via [`Receiver::feed`] → [`Reassembler`]). The connection's
        // `to_receive` queue already holds whatever the most recent
        // datagram made deliverable, so we just drain it.
        if this.channel < MAX_CHANNELS
            && let Some(msg) = this.connection.pop_ready_message(this.channel)
        {
            return Poll::Ready(Some(Ok(msg)));
        }

        // No ready messages — return None. Caller drives the carrier
        // driver task on the other side; once that processes more
        // datagrams the next poll yields the new messages.
        Poll::Ready(None)
    }
}
