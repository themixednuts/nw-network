//! Codec policies for composed value shapes.
use crate::serialize::marshaler::{Marshal, Unmarshal};

use std::marker::PhantomData;

use super::{Codec, MarshalerError, ReadBuffer, WriteBuffer};

/// A value selected by a boolean discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BooleanChoice<F, T> {
    False(F),
    True(T),
}

impl<F: Default, T> Default for BooleanChoice<F, T> {
    fn default() -> Self {
        Self::False(F::default())
    }
}

/// Encodes [`BooleanChoice`] with a boolean followed by the selected payload.
#[derive(Debug, Clone, Copy, Default)]
pub struct BooleanChoiceCodec<FM, TM>(PhantomData<fn() -> (FM, TM)>);

impl<F, T, FM: Codec<F>, TM: Codec<T>> Codec<BooleanChoice<F, T>> for BooleanChoiceCodec<FM, TM> {
    fn marshal(value: &BooleanChoice<F, T>, wb: &mut WriteBuffer) {
        match value {
            BooleanChoice::False(value) => {
                false.marshal(wb);
                FM::marshal(value, wb);
            }
            BooleanChoice::True(value) => {
                true.marshal(wb);
                TM::marshal(value, wb);
            }
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<BooleanChoice<F, T>, MarshalerError> {
        if bool::unmarshal(rb)? {
            TM::unmarshal(rb).map(BooleanChoice::True)
        } else {
            FM::unmarshal(rb).map(BooleanChoice::False)
        }
    }
}

impl<F: Marshal, T: Marshal> Marshal for BooleanChoice<F, T> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        match self {
            Self::False(value) => {
                false.marshal(wb);
                value.marshal(wb);
            }
            Self::True(value) => {
                true.marshal(wb);
                value.marshal(wb);
            }
        }
    }
}

impl<F: Unmarshal, T: Unmarshal> Unmarshal for BooleanChoice<F, T> {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        if bool::unmarshal(rb)? {
            T::unmarshal(rb).map(Self::True)
        } else {
            F::unmarshal(rb).map(Self::False)
        }
    }
}

/// Encodes an optional value through a field-local codec for its payload.
#[derive(Debug, Clone, Copy, Default)]
pub struct OptionalCodec<M>(PhantomData<fn() -> M>);

impl<T, M: Codec<T>> Codec<Option<T>> for OptionalCodec<M> {
    fn marshal(value: &Option<T>, wb: &mut WriteBuffer) {
        match value {
            Some(value) => {
                1u8.marshal(wb);
                M::marshal(value, wb);
            }
            None => 0u8.marshal(wb),
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Option<T>, MarshalerError> {
        match u8::unmarshal(rb)? {
            0 => Ok(None),
            1 => M::unmarshal(rb).map(Some),
            value => Err(MarshalerError::InvalidDiscriminant { value }),
        }
    }
}

/// Applies one field-local codec to each member of a tuple in order.
#[derive(Debug, Clone, Copy, Default)]
pub struct TupleCodec<M>(PhantomData<fn() -> M>);

/// Writes every member's non-default flag before the selected payloads.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultOmittedTupleCodec<M>(PhantomData<fn() -> M>);

macro_rules! impl_tuple_codec {
    ($(($($type:ident : $codec:ident : $index:tt),+)),+ $(,)?) => {
        $(
            impl<$($type, $codec: Codec<$type>),+> Codec<($($type,)+)>
                for TupleCodec<($($codec,)+)>
            {
                const MARSHAL_SIZE: usize = if true $(&& $codec::MARSHAL_SIZE != 0)+ {
                    0 $(+ $codec::MARSHAL_SIZE)+
                } else {
                    0
                };

                fn marshal(value: &($($type,)+), wb: &mut WriteBuffer) {
                    $($codec::marshal(&value.$index, wb);)+
                }

                fn unmarshal(rb: &mut ReadBuffer) -> Result<($($type,)+), MarshalerError> {
                    Ok(($($codec::unmarshal(rb)?,)+))
                }
            }
        )+
    };
}

impl_tuple_codec!(
    (A: MA: 0, B: MB: 1),
    (A: MA: 0, B: MB: 1, C: MC: 2),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3, E: ME: 4),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3, E: ME: 4, F: MF: 5),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3, E: ME: 4, F: MF: 5, G: MG: 6),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3, E: ME: 4, F: MF: 5, G: MG: 6, H: MH: 7),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3, E: ME: 4, F: MF: 5, G: MG: 6, H: MH: 7, I: MI: 8),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3, E: ME: 4, F: MF: 5, G: MG: 6, H: MH: 7, I: MI: 8, J: MJ: 9),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3, E: ME: 4, F: MF: 5, G: MG: 6, H: MH: 7, I: MI: 8, J: MJ: 9, K: MK: 10),
    (A: MA: 0, B: MB: 1, C: MC: 2, D: MD: 3, E: ME: 4, F: MF: 5, G: MG: 6, H: MH: 7, I: MI: 8, J: MJ: 9, K: MK: 10, L: ML: 11),
);

macro_rules! impl_default_omitted_tuple_codec {
    ($(($($type:ident : $codec:ident : $index:tt : $present:ident),+)),+ $(,)?) => {
        $(
            impl<$($type: Default + PartialEq, $codec: Codec<$type>),+>
                Codec<($($type,)+)> for DefaultOmittedTupleCodec<($($codec,)+)>
            {
                fn marshal(value: &($($type,)+), wb: &mut WriteBuffer) {
                    $(let $present = value.$index != $type::default();)+
                    $($present.marshal(wb);)+
                    $(if $present { $codec::marshal(&value.$index, wb); })+
                }

                fn unmarshal(rb: &mut ReadBuffer) -> Result<($($type,)+), MarshalerError> {
                    $(let $present = bool::unmarshal(rb)?;)+
                    Ok(($(
                        if $present { $codec::unmarshal(rb)? } else { $type::default() },
                    )+))
                }
            }
        )+
    };
}

impl_default_omitted_tuple_codec!(
    (A: MA: 0: pa, B: MB: 1: pb),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd, E: ME: 4: pe),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd, E: ME: 4: pe, F: MF: 5: pf),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd, E: ME: 4: pe, F: MF: 5: pf, G: MG: 6: pg),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd, E: ME: 4: pe, F: MF: 5: pf, G: MG: 6: pg, H: MH: 7: ph),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd, E: ME: 4: pe, F: MF: 5: pf, G: MG: 6: pg, H: MH: 7: ph, I: MI: 8: pi),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd, E: ME: 4: pe, F: MF: 5: pf, G: MG: 6: pg, H: MH: 7: ph, I: MI: 8: pi, J: MJ: 9: pj),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd, E: ME: 4: pe, F: MF: 5: pf, G: MG: 6: pg, H: MH: 7: ph, I: MI: 8: pi, J: MJ: 9: pj, K: MK: 10: pk),
    (A: MA: 0: pa, B: MB: 1: pb, C: MC: 2: pc, D: MD: 3: pd, E: ME: 4: pe, F: MF: 5: pf, G: MG: 6: pg, H: MH: 7: ph, I: MI: 8: pi, J: MJ: 9: pj, K: MK: 10: pk, L: ML: 11: pl),
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{CARRIER_ENDIAN, HalfF32Marshaler, SequenceCodec, VlqU32Marshaler};

    #[test]
    fn nested_policies_round_trip() {
        type PayloadCodec = OptionalCodec<TupleCodec<(HalfF32Marshaler, VlqU32Marshaler)>>;
        let value = Some((1.5f32, 16_384u32));
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        PayloadCodec::marshal(&value, &mut wb);
        let bytes = wb.into_vec();
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);

        let decoded: Option<(f32, u32)> = PayloadCodec::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, value);
        assert!(rb.remaining().is_empty());
    }

    #[test]
    fn nested_sequence_policy_round_trips() {
        type ElementCodec = TupleCodec<(HalfF32Marshaler, VlqU32Marshaler)>;
        type PayloadCodec = SequenceCodec<ElementCodec>;
        let value = vec![(1.5f32, 16_384u32), (-2.25, 7)];
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        PayloadCodec::marshal(&value, &mut wb);
        let bytes = wb.into_vec();
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);

        let decoded: Vec<(f32, u32)> = PayloadCodec::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, value);
        assert!(rb.remaining().is_empty());
    }

    #[test]
    fn default_omitted_tuple_writes_flags_before_payloads() {
        type Policy = DefaultOmittedTupleCodec<(
            super::super::DefaultMarshaler<u32>,
            super::super::DefaultMarshaler<u16>,
        )>;
        let value = (0u32, 7u16);
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        Policy::marshal(&value, &mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes, [0, 1, 0, 7]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        assert_eq!(Policy::unmarshal(&mut rb).unwrap(), value);
        assert!(rb.remaining().is_empty());
    }

    #[test]
    fn boolean_choice_round_trips_both_branches() {
        type Value = BooleanChoice<u16, u32>;
        for value in [Value::False(7), Value::True(0x0102_0304)] {
            let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
            value.marshal(&mut wb);
            let bytes = wb.into_vec();
            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
            assert_eq!(Value::unmarshal(&mut rb).unwrap(), value);
            assert!(rb.remaining().is_empty());
        }
    }
}
