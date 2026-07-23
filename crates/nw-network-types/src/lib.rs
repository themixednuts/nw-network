#![allow(clippy::struct_excessive_bools, clippy::zero_sized_map_values)]

//! Generated data types used by `nw-network`.
//!
//! The selected roots are kept intentionally small in `codegen/selection.json`.

include!(concat!(
    env!("NW_NETWORK_TYPES_GENERATED_DIR"),
    "/src/lib.rs"
));

mod ext;
