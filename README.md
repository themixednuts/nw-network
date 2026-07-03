# nw-network

Rust crates for the New World MMO network protocol's replication stack. The
workspace covers wire-format primitives, replicated-state machinery, state
bundles, protocol message types, generated protocol descriptors, and typed game
states. The transport crate is framework-agnostic, with Bevy-friendly wrappers
available through `nw-gridmate`'s default `client` feature.

## Workspace Layout

| Path | Purpose |
|---|---|
| `src/serialize/` | Wire primitives: endian-aware read/write buffers, marshaler traits and codec policies, VLQ integers, quantized/compressed float and vector encodings, container codecs with VLQ32 and raw-u16 length policies, replicated field handlers with default suppression and delta compression, and replicated containers with journaled change sets. |
| `src/hub/` | Replication core: sequence numbers, fragments, replicated state and fixed replicated state, filter groups, client whitelists, default bitsets, replicated state bundles, a typestate bundle builder, a borrowed bundle view, and actor movement types. |
| `src/messages/` | Registration, handshake, actor movement, and other protocol message payloads. |
| `src/states/` | Typed replicated states for gameplay systems including movement/action-list, player, combat, economy, inventory, quests, social, housing, territory, world, and presentation systems. This is a mix of handwritten and generated Rust. |
| `crates/nw-network-types` | Generated protocol types and network-schema lookup helpers. |
| `crates/nw-network-derive` | Derive macros for marshaling, fragment registration, replicated state registration, fixed-state fields, and type registries. |
| `crates/nw-gridmate` | GridMate-compatible transport layer: carrier handshake, reliability, priorities, channels, session/driver runtime, capture tooling, and Bevy integration. |

## Concepts

Replicated states are split into filter groups. Each group can carry its own
field mask and visibility policy, so a state can expose different fields to
different clients.

Fields marshal only when dirty relative to a baseline `SequenceNumber`. Values
that match their defaults are suppressed, and default bitsets let receivers
restore those values without sending full field payloads.

Per-client visibility is represented with whitelists of hashed client
identities. Empty whitelists are broadcast-visible; non-empty whitelists limit a
group to the listed clients.

State changes are packed into capped replicated state bundles per client per
tick. Bundles include sequence metadata, bandwidth/reliability flags, optional
replication-control ids, and a compact fragment buffer that can be read through
a zero-copy view.

## Quick Start

```sh
cargo build
cargo test --workspace
```

The workspace also includes Criterion benchmarks:

```sh
cargo bench --bench replicated_state
```

There are no runnable examples checked in yet.

## Status

The replication stack is intended to be wire-compatible with the live New World
protocol and is validated with capture-replay tests for carrier messages,
state-bundle parsing, and modeled state-fragment decoding. Generated protocol
types and descriptors are refreshed from the checked-in network schema.

`nw-gridmate` defaults to the `client` feature, which exposes the Bevy plugin,
message bridge, session-service resource, and connection-request component. The
optional `server` feature builds on the client feature and exposes server-side
listener types.

## License

TODO: Add a root `LICENSE` file before publishing package/license terms.
