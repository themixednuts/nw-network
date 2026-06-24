//! `#[derive(Marshaler)]` entry point.
//!
//! Wraps the field/variant codegen helpers in
//! [`crate::marshal`] / [`crate::unmarshal`] and exposes a single
//! `derive(input) -> syn::Result<TokenStream>` for the proc-macro entry
//! point in [`crate::lib`]. Keeping the entry uniform with the other
//! derives means errors flow through `syn::Error` instead of darling's
//! token-emitting error type — the caller does the
//! `.unwrap_or_else(syn::Error::into_compile_error)` once at the
//! boundary and never has to think about `compile_error!` itself.

use darling::FromDeriveInput;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Type};

use crate::attrs::MarshalerOpts;
use crate::error::darling_to_syn;
use crate::{generics, marshal, unmarshal};

pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    if let Data::Union(u) = &input.data {
        return Err(syn::Error::new_spanned(
            u.union_token,
            "#[derive(Marshaler)] does not support unions",
        ));
    }

    let mut opts = MarshalerOpts::from_derive_input(input).map_err(|err| darling_to_syn(&err))?;
    opts.generics = generics::add_marshaler_bounds(&opts.generics);

    let ident = &opts.ident;
    let (impl_generics, ty_generics, where_clause) = opts.generics.split_for_impl();

    let repr = primitive_repr(input)?;
    let marshal_body = marshal::generate_marshal_body(&opts.data, repr.as_ref());
    let unmarshal_body = unmarshal::generate_unmarshal_body(ident, &opts.data, repr.as_ref());

    Ok(quote! {
        impl #impl_generics ::nw_network::serialize::marshaler::Marshaler
        for #ident #ty_generics #where_clause
        {
            fn marshal(&self, wb: &mut ::nw_network::serialize::buffer::WriteBuffer) {
                #marshal_body
            }

            fn unmarshal(
                rb: &mut ::nw_network::serialize::buffer::ReadBuffer,
            ) -> ::core::result::Result<Self, ::nw_network::serialize::error::MarshalerError> {
                #unmarshal_body
            }
        }
    })
}

fn primitive_repr(input: &DeriveInput) -> syn::Result<Option<Type>> {
    let mut repr = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            let Some(ident) = meta.path.get_ident() else {
                return Ok(());
            };
            let ty = match ident.to_string().as_str() {
                "u8" => Some(syn::parse_quote!(u8)),
                "u16" => Some(syn::parse_quote!(u16)),
                "u32" => Some(syn::parse_quote!(u32)),
                "u64" => Some(syn::parse_quote!(u64)),
                "i8" => Some(syn::parse_quote!(i8)),
                "i16" => Some(syn::parse_quote!(i16)),
                "i32" => Some(syn::parse_quote!(i32)),
                "i64" => Some(syn::parse_quote!(i64)),
                _ => None,
            };
            if ty.is_some() {
                repr = ty;
            }
            Ok(())
        })?;
    }
    Ok(repr)
}
