use crate::serialize::{
    Codec, ConversionMarshaler, Marshaler, MarshalerError, ReadBuffer, ReplicatedMap, VlqU64,
    WriteBuffer,
};
use crate::types::GridSides;

pub const MAX_BUILDABLE_GRID_SIDE_CHANGES: usize = 0x3fff;
type GridSideByte = ConversionMarshaler<u8, GridSides>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildableGridSideActive {
    pub my_grid_side: GridSides,
    pub their_grid_side: GridSides,
    pub active: bool,
}

impl Marshaler for BuildableGridSideActive {
    const MARSHAL_SIZE: usize =
        2 * <u8 as Marshaler>::MARSHAL_SIZE + <bool as Marshaler>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        GridSideByte::marshal(&self.my_grid_side, wb);
        GridSideByte::marshal(&self.their_grid_side, wb);
        self.active.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            my_grid_side: GridSideByte::unmarshal(rb)?,
            their_grid_side: GridSideByte::unmarshal(rb)?,
            active: bool::unmarshal(rb)?,
        })
    }
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("FFABCADB-4B64-41C2-B159-A3A6980F44D0")]
#[::nw_network::type_registry(2134)]
pub struct BuildableGridComponentReplicatedState {
    pub grid_sides_active:
        ReplicatedMap<VlqU64, BuildableGridSideActive, MAX_BUILDABLE_GRID_SIDE_CHANGES>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_side_active_uses_compact_network_shape() {
        let value = BuildableGridSideActive {
            my_grid_side: GridSides::Front,
            their_grid_side: GridSides::Left,
            active: true,
        };

        let mut wb = WriteBuffer::carrier();
        value.marshal(&mut wb);
        assert_eq!(BuildableGridSideActive::MARSHAL_SIZE, 3);
        assert_eq!(wb.as_slice(), &[1, 4, 1]);

        let mut rb = ReadBuffer::carrier(wb.as_slice());
        let decoded = BuildableGridSideActive::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(rb.left(), 0);
    }
}
