//! `#[derive(ChunkMarshaler)]` — emits the standard `Marshaler` delegate for
//!
//! methods when their bitmask layout does not match `#[derive(ReplicatedState)]`.
//! One concrete impl per type avoids Rust coherence overlap with the generic
//! `Marshaler` impls for wrapper types.
//!
//! Pair with a hand-written `Fragment` implementation:
//!
//! ```ignore
//! #[derive(Default, ChunkMarshaler)]
//! pub struct FooState { /* … */ }
//!
//! impl nw_network::hub::Fragment for FooState { /* hand-rolled marshal logic */ }
//! ```
//!
//! For types whose fragment implementation is itself derive-generated, use
//! `#[derive(ReplicatedState)]` instead.

use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

#[allow(clippy::unnecessary_wraps)]
pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::nw_network::serialize::marshaler::Marshaler
        for #ident #ty_generics #where_clause
        {
            fn marshal(&self, wb: &mut ::nw_network::serialize::buffer::WriteBuffer) {
                ::nw_network::hub::DynFragment::marshal_contents(self, wb);
            }

            fn unmarshal(
                rb: &mut ::nw_network::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<Self, ::nw_network::serialize::error::MarshalerError> {
                let mut value = <Self as ::core::default::Default>::default();
                ::nw_network::hub::DynFragment::unmarshal_contents(&mut value, rb)?;
                ::core::result::Result::Ok(value)
            }
        }
    })
}
