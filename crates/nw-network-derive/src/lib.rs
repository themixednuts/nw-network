#![allow(clippy::needless_continue, clippy::too_many_lines)]

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod attrs;
mod az_rtti;
mod error;
mod fixed_replicated_state_fields;
mod fragment;
mod generics;
mod marshal;
mod marshaler;
mod replicated_state;
mod state_storage;
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

/// Implements a descriptor-mask replicated-state fragment.
///
/// Use this on structs whose fields are normal replicated fields grouped by
/// descriptor and field dirty masks. The macro injects the hidden hub state and
/// emits the fragment, marshaler, merge, metadata, and optional type-index
/// registration glue when a `#[type_registry(...)]` attribute is present.
#[proc_macro_attribute]
pub fn replicated_state(args: TokenStream, input: TokenStream) -> TokenStream {
    replicated_state::attribute(args.into(), input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives fixed-size replicated-state field traversal.
///
/// Use this only when the Rust field declaration order matches the fixed
/// registration order. Fixed replicated states predeclare the number of groups,
/// the maximum fields per group, optional client whitelist capacity, and
/// optional hub-to-hub user attributes.
#[proc_macro_derive(FixedReplicatedStateFields, attributes(fixed_state))]
pub fn derive_fixed_replicated_state_fields(input: TokenStream) -> TokenStream {
    run(input, fixed_replicated_state_fields::derive)
}

/// Injects the base storage for a custom fixed-size replicated-state fragment.
///
/// This macro only adds the fixed base field. Use it with a handwritten
/// `FixedReplicatedStateFields` implementation when field traversal needs
/// custom ordering or grouping; otherwise prefer the
/// `FixedReplicatedStateFields` derive.
#[proc_macro_attribute]
pub fn fixed_replicated_state(args: TokenStream, input: TokenStream) -> TokenStream {
    state_storage::fixed_replicated_state(args.into(), input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `AzRtti` from an AZ type UUID and optional native name.
#[proc_macro_attribute]
pub fn az_rtti(args: TokenStream, input: TokenStream) -> TokenStream {
    az_rtti::attribute(args.into(), input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements type-index registration metadata for network fragments/messages.
#[proc_macro_attribute]
pub fn type_registry(args: TokenStream, input: TokenStream) -> TokenStream {
    type_registry::attribute(args.into(), input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Fragment)]
pub fn derive_fragment(input: TokenStream) -> TokenStream {
    run(input, fragment::derive)
}
