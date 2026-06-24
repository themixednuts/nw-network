use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Expr, ExprLit, Lit, Meta, Token};

pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let type_index = parse_type_index(input)?;
    if type_index == 0 {
        return Err(syn::Error::new_spanned(
            input,
            "type_registry type_index must be nonzero",
        ));
    }

    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::nw_network::types::TypeRegistryEntry
        for #ident #ty_generics #where_clause
        {
            const TYPE_INDEX: u32 = #type_index;
        }
    })
}

fn parse_type_index(input: &DeriveInput) -> syn::Result<u32> {
    let mut type_index = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("type_registry") {
            continue;
        }

        let Meta::List(list) = &attr.meta else {
            return Err(syn::Error::new_spanned(
                attr,
                "#[type_registry(...)] requires arguments",
            ));
        };
        let args = list.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)?;
        for (index, expr) in args.iter().enumerate() {
            if type_index.is_none() {
                type_index = Some(parse_arg(expr, index)?);
            }
        }
    }

    type_index.ok_or_else(|| syn::Error::new_spanned(input, "#[type_registry(...)] is required"))
}

fn parse_arg(expr: &Expr, index: usize) -> syn::Result<u32> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) if index == 0 => value.base10_parse(),
        _ => Err(syn::Error::new_spanned(
            expr,
            "unsupported type_registry argument",
        )),
    }
}
