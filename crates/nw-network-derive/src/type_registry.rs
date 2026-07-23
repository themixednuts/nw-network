use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Expr, ExprLit, Lit, Token, parse::Parser};

struct Options {
    type_index: u32,
    register_class: bool,
}

pub fn attribute(args: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let item = syn::parse2::<DeriveInput>(input)?;
    let options = parse_args(args)?;
    let impls = expand(&item, options)?;

    Ok(quote! {
        #item
        #impls
    })
}

fn expand(input: &DeriveInput, options: Options) -> syn::Result<TokenStream> {
    if options.type_index == 0 {
        return Err(syn::Error::new_spanned(
            input,
            "type_registry type_index must be nonzero",
        ));
    }
    if options.register_class && !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "network class registration does not support generic types",
        ));
    }

    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let type_index = options.type_index;
    let class_registration = options.register_class.then(|| {
        quote! {
            ::inventory::submit! {
                ::nw_network::serialize::NetworkClassRegistration::of::<#ident #ty_generics>()
            }
        }
    });

    Ok(quote! {
        impl #impl_generics ::nw_network::types::TypeRegistryEntry
        for #ident #ty_generics #where_clause
        {
            const TYPE_INDEX: u32 = #type_index;
        }

        #class_registration
    })
}

fn parse_args(args: TokenStream) -> syn::Result<Options> {
    let args = Punctuated::<Expr, Token![,]>::parse_terminated.parse2(args)?;
    let mut type_index = None;
    let mut register_class = false;
    for (index, expr) in args.iter().enumerate() {
        if index == 0 {
            type_index = Some(parse_type_index(expr)?);
            continue;
        }
        match expr {
            Expr::Path(path) if path.path.is_ident("class") => register_class = true,
            _ => {
                return Err(syn::Error::new_spanned(
                    expr,
                    "unsupported type_registry argument",
                ));
            }
        }
    }
    Ok(Options {
        type_index: type_index.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "type_index is required")
        })?,
        register_class,
    })
}

fn parse_type_index(expr: &Expr) -> syn::Result<u32> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) => value.base10_parse(),
        _ => Err(syn::Error::new_spanned(
            expr,
            "type_registry type_index must be an integer literal",
        )),
    }
}
