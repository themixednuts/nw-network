//! Marshaler conversions emitted for selected generated source types.

include!(concat!(
    env!("OUT_DIR"),
    "/nw_network/generated_conversions.rs"
));
