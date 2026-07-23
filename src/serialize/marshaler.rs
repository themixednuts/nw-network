//! Core marshaling traits.

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
};

/// A value that can encode itself into its network representation.
pub trait Marshal {
    /// Fixed wire size, or `0` for dynamic/unspecified.
    const MARSHAL_SIZE: usize = 0;

    /// Serialize into the provided write buffer. `WriteBuffer` is
    /// infallible, so this stays `()`.
    fn marshal(&self, wb: &mut WriteBuffer);
}

/// A value that can be decoded from its network representation.
pub trait Unmarshal: Sized {
    /// Deserialize from the provided read buffer.
    ///
    /// # Errors
    ///
    /// Returns an error when the buffer does not contain a valid value of this type.
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError>;
}

/// A value with symmetric encode and decode support.
///
/// Transport APIs use [`Marshal`] or [`Unmarshal`] directly so native
/// one-way messages do not need to expose behavior they do not implement.
/// For external field policies that encode another type with a non-default
/// representation, see [`Codec`].
pub trait Marshaler: Marshal + Unmarshal {}

impl<T: Marshal + Unmarshal> Marshaler for T {}

/// "This zero-sized type is a wire codec for some `T`" — the policy slot
/// in `ReplicatedFieldHandler<T, M>` and `ReplicatedContainer<C>`.
///
/// `Codec<T>` separates "value type that encodes itself" (which stays on
/// [`Marshaler`]) from "external policy that overrides the wire shape for
/// a field-local value.
///
pub trait Codec<T> {
    /// Fixed wire size for this policy, or `0` for dynamic/unspecified.
    ///
    /// Field-local codecs can expose fixed-size metadata even when they are
    /// not value-level [`Marshaler`] impls.
    const MARSHAL_SIZE: usize = 0;

    /// Serialize `value` using this policy's wire shape.
    fn marshal(value: &T, wb: &mut WriteBuffer);

    /// Deserialize a `T` using this policy's wire shape.
    ///
    /// # Errors
    ///
    /// Returns an error when the buffer does not contain a valid value for this policy.
    fn unmarshal(rb: &mut ReadBuffer) -> Result<T, MarshalerError>;
}

/// Default policy for a value that marshals itself.
///
/// Use this in policy slots when the field's value type already owns its wire
/// shape through [`Marshaler`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultMarshaler<T>(std::marker::PhantomData<fn() -> T>);

impl<T: Marshaler> Codec<T> for DefaultMarshaler<T> {
    const MARSHAL_SIZE: usize = <T as Marshal>::MARSHAL_SIZE;

    #[inline]
    fn marshal(value: &T, wb: &mut WriteBuffer) {
        <T as Marshal>::marshal(value, wb);
    }

    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<T, MarshalerError> {
        <T as Unmarshal>::unmarshal(rb)
    }
}

/// Fixed-size query for value-level marshalers.
pub struct IsFixedMarshaler<M>(std::marker::PhantomData<fn() -> M>);

impl<M: Marshaler> IsFixedMarshaler<M> {
    pub const MARSHAL_SIZE: usize = <M as Marshal>::MARSHAL_SIZE;
    pub const VALUE: bool = <M as Marshal>::MARSHAL_SIZE != 0;
}

/// Fixed-size query for policy codecs (`ContainerMarshaler`, `HalfF32`, etc.).
pub struct IsFixedCodec<T, M>(std::marker::PhantomData<fn() -> (T, M)>);

impl<T, M: Codec<T>> IsFixedCodec<T, M> {
    pub const MARSHAL_SIZE: usize = M::MARSHAL_SIZE;
    pub const VALUE: bool = M::MARSHAL_SIZE != 0;
}

/// Type-level query for whether `M` can encode `T` through [`Codec<T>`].
pub struct IsMarshalerForType<T, M>(std::marker::PhantomData<fn() -> (T, M)>);

impl<T, M: Codec<T>> IsMarshalerForType<T, M> {
    pub const VALUE: bool = true;
}

#[inline]
#[must_use]
pub const fn is_fixed_marshaler<M: Marshaler>() -> bool {
    <M as Marshal>::MARSHAL_SIZE != 0
}

#[inline]
#[must_use]
pub const fn fixed_marshal_size<M: Marshaler>() -> usize {
    <M as Marshal>::MARSHAL_SIZE
}

#[inline]
#[must_use]
pub const fn is_fixed_codec<T, M: Codec<T>>() -> bool {
    M::MARSHAL_SIZE != 0
}
