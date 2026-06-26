//! Generated network types and protocol glue.

pub mod states {
    #![allow(dead_code, unused_imports)]

    include!(concat!(env!("OUT_DIR"), "/nw_network/generated_states.rs"));
}

pub mod messages {
    include!(concat!(
        env!("OUT_DIR"),
        "/nw_network/generated_messages.rs"
    ));
}

mod conversions {
    include!(concat!(
        env!("OUT_DIR"),
        "/nw_network/generated_conversions.rs"
    ));
}

pub use states::*;
