use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Expr, ExprLit, Lit, Token, parse::Parser};

pub fn attribute(args: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let item = syn::parse2::<DeriveInput>(input)?;
    let type_index = parse_type_index_args(args)?;
    let impls = expand(&item, type_index)?;

    Ok(quote! {
        #item
        #impls
    })
}

fn expand(input: &DeriveInput, type_index: u32) -> syn::Result<TokenStream> {
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

fn parse_type_index_args(args: TokenStream) -> syn::Result<u32> {
    let args = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(args)?;
    let mut type_index = None;
    for (index, expr) in args.iter().enumerate() {
        if type_index.is_none() {
            type_index = Some(parse_arg(expr, index)?);
        }
    }
    type_index
        .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "type_index is required"))
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
