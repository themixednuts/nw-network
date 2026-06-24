#![allow(clippy::struct_excessive_bools, clippy::zero_sized_map_values)]

//! Generated data types used by `nw-network`.
//!
//! The selected roots are kept intentionally small in `codegen/selection.json`.

include!(concat!(env!("OUT_DIR"), "/nw_network/src/lib.rs"));

mod ext;
