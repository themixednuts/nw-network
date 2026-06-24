#![allow(clippy::needless_continue, clippy::too_many_lines)]

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod attrs;
mod az_rtti;
mod chunk_marshaler;
mod error;
mod fixed_replicated_state_fields;
mod fragment;
mod generics;
mod marshal;
mod marshaler;
mod replicated_state;
mod type_registry;
mod unmarshal;

fn run<F>(input: TokenStream, f: F) -> TokenStream
where
    F: FnOnce(&DeriveInput) -> syn::Result<proc_macro2::TokenStream>,
{
    let input = parse_macro_input!(input as DeriveInput);
    f(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Marshaler, attributes(marshal))]
pub fn derive_marshaler(input: TokenStream) -> TokenStream {
    run(input, marshaler::derive)
}

#[proc_macro_derive(ChunkMarshaler)]
pub fn derive_chunk_marshaler(input: TokenStream) -> TokenStream {
    run(input, chunk_marshaler::derive)
}

#[proc_macro_derive(ReplicatedState, attributes(replicated_state))]
pub fn derive_replicated_state(input: TokenStream) -> TokenStream {
    run(input, replicated_state::derive)
}

#[proc_macro_derive(FixedReplicatedStateFields, attributes(fixed_state))]
pub fn derive_fixed_replicated_state_fields(input: TokenStream) -> TokenStream {
    run(input, fixed_replicated_state_fields::derive)
}

#[proc_macro_derive(AzRtti, attributes(az_rtti))]
pub fn derive_az_rtti(input: TokenStream) -> TokenStream {
    run(input, az_rtti::derive)
}

#[proc_macro_derive(TypeRegistry, attributes(type_registry))]
pub fn derive_type_registry(input: TokenStream) -> TokenStream {
    run(input, type_registry::derive)
}

#[proc_macro_derive(Fragment)]
pub fn derive_fragment(input: TokenStream) -> TokenStream {
    run(input, fragment::derive)
}
